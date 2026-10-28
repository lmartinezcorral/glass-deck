pub mod claude_code;
pub mod clima;
pub mod codex;
pub mod historial;
#[cfg(all(test, windows))]
mod qa;
pub mod servicios;
#[cfg(windows)]
pub mod win;

use crate::acciones::procesos::{Candidato, Dueno, Nivel, Politica};
use crate::config::{AlmacenConfig, Intervalos};
use crate::ventanas::{self, catalogo, Gestor};
use historial::{puntos_por_hora, Anillo, PUNTOS_UNA_HORA};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Disks, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager};

const TIC: Duration = Duration::from_millis(250);
const INTERVALO_BATERIA_MIN_MS: u64 = 3_000;
const REVISION_ENTORNO: Duration = Duration::from_secs(2);
const REVISION_ADAPTADOR: Duration = Duration::from_secs(5);

#[derive(Serialize, Clone)]
pub struct MuestraCpu {
    pub total: f32,
    pub nucleos: Vec<f32>,
    pub nombre: String,
}

#[derive(Serialize, Clone)]
pub struct UnidadDisco {
    pub unidad: String,
    pub etiqueta: String,
    pub total: u64,
    pub libre: u64,
    pub extraible: bool,
    pub lectura_bps: f64,
    pub escritura_bps: f64,
}

#[derive(Serialize, Clone, Default)]
pub struct MuestraRed {
    pub adaptador: String,
    pub tipo: String,
    pub ip: Option<String>,
    pub bajada_bps: f64,
    pub subida_bps: f64,
}

#[derive(Serialize, Clone)]
pub struct FilaProceso {
    pub pid: u32,
    pub nombre: String,
    pub cpu: f32,
    pub ram: u64,
    pub usuario: Option<String>,
    pub nivel: Nivel,
    pub reiniciar_explorer: bool,
}

#[derive(Serialize, Clone)]
#[serde(tag = "fuente", content = "datos", rename_all = "snake_case")]
pub enum Lectura {
    Cpu(MuestraCpu),
    Ram(MemoriaSerie),
    Disco(Vec<UnidadDisco>),
    Red(MuestraRed),
    Procesos(Vec<FilaProceso>),
}

#[derive(Serialize, Clone, Copy, Default)]
pub struct MemoriaSerie {
    pub total: u64,
    pub usada: u64,
    pub en_cache: u64,
    pub libre: u64,
    pub comprometida: u64,
    pub limite_comprometida: u64,
}

pub struct Historiales {
    pub cpu: Anillo,
    pub nucleos: Vec<Anillo>,
    pub ram: Anillo,
    pub bajada: Anillo,
    pub subida: Anillo,
    pub disco: HashMap<String, (Anillo, Anillo)>,
}

impl Default for Historiales {
    fn default() -> Self {
        Self {
            cpu: Anillo::nuevo(PUNTOS_UNA_HORA),
            nucleos: Vec::new(),
            ram: Anillo::nuevo(PUNTOS_UNA_HORA),
            bajada: Anillo::nuevo(PUNTOS_UNA_HORA),
            subida: Anillo::nuevo(PUNTOS_UNA_HORA),
            disco: HashMap::new(),
        }
    }
}

#[derive(Serialize)]
pub struct SerieHistorial {
    pub nombre: String,
    pub valores: Vec<f32>,
}

impl Historiales {
    pub fn series(&self, fuente: &str, intervalos: &Intervalos) -> Vec<SerieHistorial> {
        let n = puntos_por_hora(if fuente == "disco" { intervalos.disco_ms } else { intervalos.cpu_ram_red_ms });
        let s = |nombre: &str, a: &Anillo| SerieHistorial { nombre: nombre.to_string(), valores: a.ultimos(n) };
        match fuente {
            "cpu" => {
                let mut v = vec![s("total", &self.cpu)];
                v.extend(self.nucleos.iter().enumerate().map(|(i, a)| s(&format!("n{i}"), a)));
                v
            }
            "ram" => vec![s("usada", &self.ram)],
            "red" => vec![s("bajada", &self.bajada), s("subida", &self.subida)],
            "disco" => {
                let mut claves: Vec<_> = self.disco.keys().cloned().collect();
                claves.sort();
                claves
                    .iter()
                    .flat_map(|k| {
                        let (l, e) = &self.disco[k];
                        [s(&format!("{k} lectura"), l), s(&format!("{k} escritura"), e)]
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct EstadoRecoleccion {
    pub pausado: AtomicBool,
    pub con_bateria: AtomicBool,
    pub historiales: Arc<Mutex<Historiales>>,
}

struct Recolector {
    sys: System,
    discos: Disks,
    #[cfg(windows)]
    disco_io: Option<win::ConsultaDisco>,
    #[cfg(windows)]
    memoria_listas: Option<win::ConsultaMemoria>,
    #[cfg(windows)]
    cpu_total: Option<win::ConsultaCpu>,
    #[cfg(windows)]
    adaptador: Option<win::Adaptador>,
    adaptador_revisado: Option<Instant>,
    red_previa: Option<(u32, u64, u64, Instant)>,
    duenos: HashMap<u32, (Dueno, Option<String>)>,
    nombre_cpu: String,
}

impl Recolector {
    fn nuevo() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        let nombre_cpu = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
        Self {
            sys,
            discos: Disks::new_with_refreshed_list(),
            #[cfg(windows)]
            disco_io: None,
            #[cfg(windows)]
            memoria_listas: None,
            #[cfg(windows)]
            cpu_total: win::ConsultaCpu::nueva(),
            #[cfg(windows)]
            adaptador: None,
            adaptador_revisado: None,
            red_previa: None,
            duenos: HashMap::new(),
            nombre_cpu,
        }
    }

    fn cpu(&mut self, h: &mut Historiales) -> MuestraCpu {
        self.sys.refresh_cpu_usage();
        #[cfg(windows)]
        let total = self.cpu_total.as_ref().and_then(|c| c.leer()).unwrap_or_else(|| self.sys.global_cpu_usage());
        #[cfg(not(windows))]
        let total = self.sys.global_cpu_usage();
        let nucleos: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        h.cpu.agregar(total);
        if h.nucleos.len() != nucleos.len() {
            h.nucleos = (0..nucleos.len()).map(|_| Anillo::nuevo(PUNTOS_UNA_HORA)).collect();
        }
        for (a, v) in h.nucleos.iter_mut().zip(&nucleos) {
            a.agregar(*v);
        }
        MuestraCpu { total, nucleos, nombre: self.nombre_cpu.clone() }
    }

    fn ram(&mut self, h: &mut Historiales) -> MemoriaSerie {
        #[cfg(windows)]
        let m = {
            if self.memoria_listas.is_none() {
                self.memoria_listas = win::ConsultaMemoria::nueva();
            }
            let m = win::memoria(self.memoria_listas.as_ref().and_then(|c| c.leer()));
            MemoriaSerie {
                total: m.total,
                usada: m.usada,
                en_cache: m.en_cache,
                libre: m.libre,
                comprometida: m.comprometida,
                limite_comprometida: m.limite_comprometida,
            }
        };
        #[cfg(not(windows))]
        let m = {
            self.sys.refresh_memory();
            MemoriaSerie {
                total: self.sys.total_memory(),
                usada: self.sys.used_memory(),
                libre: self.sys.available_memory(),
                ..Default::default()
            }
        };
        if m.total > 0 {
            h.ram.agregar(m.usada as f32 * 100.0 / m.total as f32);
        }
        m
    }

    fn disco(&mut self, h: &mut Historiales) -> Vec<UnidadDisco> {
        self.discos.refresh(true);
        #[cfg(windows)]
        let io = {
            if self.disco_io.is_none() {
                self.disco_io = win::ConsultaDisco::nueva();
            }
            self.disco_io.as_mut().map(|c| c.muestrear()).unwrap_or_default()
        };
        #[cfg(not(windows))]
        let io: HashMap<String, (f64, f64)> = HashMap::new();
        let mut unidades: Vec<UnidadDisco> = self
            .discos
            .list()
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| {
                let unidad = d.mount_point().to_string_lossy().trim_end_matches('\\').to_ascii_uppercase();
                let (l, e) = io.get(&unidad).copied().unwrap_or((0.0, 0.0));
                UnidadDisco {
                    etiqueta: d.name().to_string_lossy().to_string(),
                    total: d.total_space(),
                    libre: d.available_space(),
                    extraible: d.is_removable(),
                    lectura_bps: l,
                    escritura_bps: e,
                    unidad,
                }
            })
            .collect();
        unidades.sort_by(|a, b| a.extraible.cmp(&b.extraible).then(a.unidad.cmp(&b.unidad)));
        for u in &unidades {
            let par = h
                .disco
                .entry(u.unidad.clone())
                .or_insert_with(|| (Anillo::nuevo(PUNTOS_UNA_HORA), Anillo::nuevo(PUNTOS_UNA_HORA)));
            par.0.agregar((u.lectura_bps / 1_048_576.0) as f32);
            par.1.agregar((u.escritura_bps / 1_048_576.0) as f32);
        }
        unidades
    }

    #[cfg(windows)]
    fn red(&mut self, h: &mut Historiales) -> MuestraRed {
        let revisar = self.adaptador_revisado.map(|t| t.elapsed() >= REVISION_ADAPTADOR).unwrap_or(true);
        if revisar {
            self.adaptador = win::adaptador_activo();
            self.adaptador_revisado = Some(Instant::now());
        }
        let Some(a) = self.adaptador.clone() else {
            self.red_previa = None;
            return MuestraRed { adaptador: "Sin conexión".into(), ..Default::default() };
        };
        let ahora = Instant::now();
        let (mut bajada, mut subida) = (0.0, 0.0);
        if let Some((entrada, salida)) = win::bytes_adaptador(a.indice) {
            if let Some((idx, e0, s0, t0)) = self.red_previa {
                let dt = ahora.duration_since(t0).as_secs_f64();
                if idx == a.indice && dt > 0.0 {
                    bajada = entrada.saturating_sub(e0) as f64 / dt;
                    subida = salida.saturating_sub(s0) as f64 / dt;
                }
            }
            self.red_previa = Some((a.indice, entrada, salida, ahora));
        }
        h.bajada.agregar((bajada / 1024.0) as f32);
        h.subida.agregar((subida / 1024.0) as f32);
        MuestraRed { adaptador: a.nombre, tipo: a.tipo, ip: a.ip, bajada_bps: bajada, subida_bps: subida }
    }

    #[cfg(not(windows))]
    fn red(&mut self, _h: &mut Historiales) -> MuestraRed {
        MuestraRed::default()
    }

    fn procesos(&mut self, politica: &Politica) -> Vec<FilaProceso> {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let nucleos = self.sys.cpus().len().max(1) as f32;
        let vivos: HashSet<u32> = self.sys.processes().keys().map(|p| p.as_u32()).collect();
        self.duenos.retain(|pid, _| vivos.contains(pid));
        let mut filas = Vec::with_capacity(vivos.len());
        for (pid, p) in self.sys.processes() {
            let pid = pid.as_u32();
            let nombre = p.name().to_string_lossy().to_string();
            let (dueno, usuario) = self
                .duenos
                .entry(pid)
                .or_insert_with(|| {
                    #[cfg(windows)]
                    {
                        crate::acciones::procesos::sistema::dueno_de(pid)
                    }
                    #[cfg(not(windows))]
                    {
                        (Dueno::Desconocido, None)
                    }
                })
                .clone();
            let nivel = politica.clasificar(&Candidato { pid, nombre: nombre.clone(), dueno });
            filas.push(FilaProceso {
                pid,
                reiniciar_explorer: Politica::sugerir_reiniciar_explorer(&nombre),
                nombre,
                cpu: p.cpu_usage() / nucleos,
                ram: p.memory(),
                usuario,
                nivel,
            });
        }
        filas
    }
}

fn widgets_suscritos(app: &AppHandle) -> HashSet<&'static str> {
    let todos_visibles = app.state::<Gestor>().todos_visibles.load(Ordering::Relaxed);
    let cfg = app.state::<AlmacenConfig>().leer();
    suscribibles(
        todos_visibles,
        catalogo::WIDGETS
            .iter()
            .map(|w| (w.id, cfg.widget(w.id).activo, ventanas::ventana_widget(app, w.id).is_some())),
    )
}

/// Un widget desactivado o sin ventana no recibe lecturas: su recolector ni siquiera corre.
fn suscribibles(
    todos_visibles: bool,
    widgets: impl Iterator<Item = (&'static str, bool, bool)>,
) -> HashSet<&'static str> {
    if !todos_visibles {
        return HashSet::new();
    }
    widgets.filter(|(_, activo, abierto)| *activo && *abierto).map(|(id, _, _)| id).collect()
}

fn intervalo_ms(id: &str, intervalos: &Intervalos, con_bateria: bool) -> u64 {
    match id {
        "procesos" => intervalos.procesos_ms,
        "disco" => intervalos.disco_ms,
        _ if con_bateria => intervalos.cpu_ram_red_ms.max(INTERVALO_BATERIA_MIN_MS),
        _ => intervalos.cpu_ram_red_ms,
    }
}

/// Con una app a pantalla completa o la recolección en pausa no vence ninguna fuente.
fn vencidos(
    suscritos: &HashSet<&'static str>,
    proximo: &HashMap<&'static str, Instant>,
    ahora: Instant,
    pausar: bool,
) -> Vec<&'static str> {
    if pausar {
        return Vec::new();
    }
    suscritos.iter().copied().filter(|id| proximo.get(id).map(|t| ahora >= *t).unwrap_or(true)).collect()
}

fn emitir(app: &AppHandle, id: &str, lectura: Lectura) {
    let _ = app.emit_to(catalogo::etiqueta(id), "gd://datos", lectura);
}

pub fn iniciar(app: &AppHandle) {
    let app = app.clone();
    std::thread::Builder::new()
        .name("glassdeck-recolector".into())
        .spawn(move || ciclo(app))
        .ok();
}

fn ciclo(app: AppHandle) {
    let estado = app.state::<EstadoRecoleccion>();
    let mut r = Recolector::nuevo();
    let mut proximo: HashMap<&'static str, Instant> = HashMap::new();
    let mut entorno_revisado = Instant::now() - REVISION_ENTORNO;
    let mut ocupado = false;
    loop {
        std::thread::sleep(TIC);
        if entorno_revisado.elapsed() >= REVISION_ENTORNO {
            entorno_revisado = Instant::now();
            #[cfg(windows)]
            {
                ocupado = win::pantalla_completa_ocupada();
                let bateria = win::con_bateria();
                if estado.con_bateria.swap(bateria, Ordering::Relaxed) != bateria {
                    let _ = app.emit("gd://bateria", bateria);
                }
            }
        }
        let pausar = ocupado || estado.pausado.load(Ordering::Relaxed);
        let suscritos = if pausar { HashSet::new() } else { widgets_suscritos(&app) };
        let ahora = Instant::now();
        let vencidos = vencidos(&suscritos, &proximo, ahora, pausar);
        if vencidos.is_empty() {
            continue;
        }
        let cfg = app.state::<AlmacenConfig>().leer();
        let bateria = cfg.pausar_con_bateria && estado.con_bateria.load(Ordering::Relaxed);
        let intervalo = |id: &str| Duration::from_millis(intervalo_ms(id, &cfg.intervalos, bateria));
        let politica = vencidos
            .contains(&"procesos")
            .then(|| Politica::nueva(&cfg.lista_negra_usuario, std::process::id()));
        let mut h = estado.historiales.lock();
        for id in vencidos {
            proximo.insert(id, ahora + intervalo(id));
            let lectura = match id {
                "cpu" => Lectura::Cpu(r.cpu(&mut h)),
                "ram" => Lectura::Ram(r.ram(&mut h)),
                "disco" => Lectura::Disco(r.disco(&mut h)),
                "red" => Lectura::Red(r.red(&mut h)),
                "procesos" => match &politica {
                    Some(p) => Lectura::Procesos(r.procesos(p)),
                    None => continue,
                },
                _ => continue,
            };
            emitir(&app, id, lectura);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn con_bateria_cpu_ram_y_red_no_bajan_de_3_s() {
        let iv = Intervalos::default();
        assert_eq!(intervalo_ms("cpu", &iv, false), 2_000);
        assert_eq!(intervalo_ms("cpu", &iv, true), 3_000);
        assert_eq!(intervalo_ms("red", &iv, true), 3_000);
        assert_eq!(intervalo_ms("disco", &iv, true), 5_000);
        assert_eq!(intervalo_ms("procesos", &iv, true), 3_000);
        let lento = Intervalos { cpu_ram_red_ms: 5_000, ..iv };
        assert_eq!(intervalo_ms("ram", &lento, true), 5_000);
    }

    #[test]
    fn pantalla_completa_o_pausa_no_recolectan() {
        let suscritos: HashSet<&'static str> = ["cpu", "red"].into_iter().collect();
        let ahora = Instant::now();
        let mut proximo = HashMap::new();
        assert!(vencidos(&suscritos, &proximo, ahora, true).is_empty());
        assert_eq!(vencidos(&suscritos, &proximo, ahora, false).len(), 2);
        proximo.insert("cpu", ahora + Duration::from_secs(1));
        assert_eq!(vencidos(&suscritos, &proximo, ahora, false), vec!["red"]);
        assert!(vencidos(&HashSet::new(), &proximo, ahora, false).is_empty());
    }

    #[test]
    fn un_widget_desactivado_o_cerrado_no_se_recolecta() {
        let widgets = || [("cpu", true, true), ("procesos", false, true), ("disco", true, false)].into_iter();
        let s = suscribibles(true, widgets());
        assert!(s.contains("cpu"));
        assert!(!s.contains("procesos"));
        assert!(!s.contains("disco"));
        assert!(suscribibles(false, widgets()).is_empty());
        let ahora = Instant::now();
        assert_eq!(vencidos(&s, &HashMap::new(), ahora, false), vec!["cpu"]);
    }

    #[test]
    fn la_serie_de_historial_entrega_solo_la_ultima_hora() {
        let mut h = Historiales::default();
        for v in 0..PUNTOS_UNA_HORA {
            h.cpu.agregar(v as f32);
        }
        let iv = Intervalos::default();
        let total = &h.series("cpu", &iv)[0];
        assert_eq!(total.valores.len(), 1_800);
        assert_eq!(total.valores[0], 1_800.0);
    }
}

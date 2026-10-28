use super::servicios::{Fuente, Motivo};
use crate::config::{Config, PrecioModelo};
use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const DIAS_EN_MEMORIA: i64 = 62;
const DIAS_RESUMEN: i64 = 30;
const PROFUNDIDAD_MAX: usize = 6;

/// GD-CLA-03: solo campos de uso. `message.content` y cualquier texto no se declaran, así que serde
/// los salta sin copiarlos a ninguna estructura.
#[derive(Deserialize)]
struct Linea {
    #[serde(rename = "type")]
    tipo: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    cwd: Option<String>,
    message: Option<Mensaje>,
}

#[derive(Deserialize)]
struct Mensaje {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Uso>,
}

#[derive(Deserialize)]
struct Uso {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tokens {
    pub entrada: u64,
    pub salida: u64,
    pub cache_escritura: u64,
    pub cache_lectura: u64,
}

impl Tokens {
    fn sumar(&mut self, o: &Tokens) {
        self.entrada += o.entrada;
        self.salida += o.salida;
        self.cache_escritura += o.cache_escritura;
        self.cache_lectura += o.cache_lectura;
    }

    fn vacio(&self) -> bool {
        *self == Tokens::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Totales {
    pub tokens: Tokens,
    pub costo_usd: f64,
}

impl Totales {
    fn sumar(&mut self, t: &Tokens, costo: f64) {
        self.tokens.sumar(t);
        self.costo_usd += costo;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Fila {
    pub dia: String,
    pub modelo: String,
    pub proyecto: String,
    pub tokens: Tokens,
    pub costo_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dia {
    pub dia: String,
    pub tokens: Tokens,
    pub costo_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResumenClaudeCode {
    pub hoy: String,
    pub total_hoy: Totales,
    pub total_mes: Totales,
    pub dias: Vec<Dia>,
    pub filas: Vec<Fila>,
    pub sin_precio: Vec<String>,
}

struct Marca {
    posicion: u64,
    creado: Option<SystemTime>,
    proyecto: String,
}

type ClaveTotal = (NaiveDate, String, String);

#[derive(Default)]
pub struct Lector {
    archivos: HashMap<PathBuf, Marca>,
    vistos: HashSet<u64>,
    nombres: HashMap<String, String>,
    totales: BTreeMap<ClaveTotal, Tokens>,
}

pub fn dia_local<Tz: TimeZone>(marca: &str, zona: &Tz) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(marca).ok().map(|t| t.with_timezone(zona).date_naive())
}

pub fn precio_para<'a>(modelo: &str, precios: &'a [PrecioModelo]) -> Option<&'a PrecioModelo> {
    let m = modelo.to_lowercase();
    precios.iter().filter(|p| m.contains(&p.patron)).max_by_key(|p| p.patron.len())
}

pub fn costo(t: &Tokens, p: &PrecioModelo) -> f64 {
    (t.entrada as f64 * p.entrada
        + t.salida as f64 * p.salida
        + t.cache_escritura as f64 * p.cache_escritura
        + t.cache_lectura as f64 * p.cache_lectura)
        / 1_000_000.0
}

/// Nombre corto a partir del nombre de carpeta de Claude Code (la ruta con `-` en lugar de separadores).
fn nombre_de_carpeta(carpeta: &str) -> String {
    carpeta.rsplit('-').find(|s| !s.is_empty()).unwrap_or(carpeta).to_string()
}

fn huella(id: &str, peticion: &str) -> u64 {
    let mut h = DefaultHasher::new();
    id.hash(&mut h);
    peticion.hash(&mut h);
    h.finish()
}

fn jsonl_bajo(raiz: &Path) -> Vec<(PathBuf, String)> {
    let mut salida = Vec::new();
    let Ok(proyectos) = fs::read_dir(raiz) else { return salida };
    for p in proyectos.flatten() {
        if !p.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let carpeta = p.file_name().to_string_lossy().to_string();
        let mut pendientes = vec![(p.path(), 0usize)];
        while let Some((dir, nivel)) = pendientes.pop() {
            let Ok(entradas) = fs::read_dir(&dir) else { continue };
            for e in entradas.flatten() {
                let Ok(tipo) = e.file_type() else { continue };
                if tipo.is_symlink() {
                    continue;
                }
                let ruta = e.path();
                if tipo.is_dir() && nivel < PROFUNDIDAD_MAX {
                    pendientes.push((ruta, nivel + 1));
                } else if tipo.is_file() && ruta.extension().map(|x| x == "jsonl").unwrap_or(false) {
                    salida.push((ruta, carpeta.clone()));
                }
            }
        }
    }
    salida
}

impl Lector {
    pub fn actualizar<Tz: TimeZone>(&mut self, raiz: &Path, zona: &Tz, hoy: NaiveDate) {
        let archivos = jsonl_bajo(raiz);
        let vigentes: HashSet<&PathBuf> = archivos.iter().map(|(r, _)| r).collect();
        self.archivos.retain(|r, _| vigentes.contains(r));
        for (ruta, carpeta) in &archivos {
            let _ = self.leer_archivo(ruta, carpeta, zona);
        }
        let limite = hoy - chrono::Duration::days(DIAS_EN_MEMORIA);
        self.totales.retain(|(dia, _, _), _| *dia >= limite);
    }

    fn leer_archivo<Tz: TimeZone>(&mut self, ruta: &Path, carpeta: &str, zona: &Tz) -> std::io::Result<()> {
        let meta = fs::metadata(ruta)?;
        let creado = meta.created().ok();
        let marca = self
            .archivos
            .entry(ruta.to_path_buf())
            .or_insert_with(|| Marca { posicion: 0, creado, proyecto: carpeta.to_string() });
        if marca.creado != creado || meta.len() < marca.posicion {
            marca.posicion = 0;
            marca.creado = creado;
        }
        if meta.len() == marca.posicion {
            return Ok(());
        }
        let (inicio, proyecto) = (marca.posicion, marca.proyecto.clone());
        let mut f = File::open(ruta)?;
        f.seek(SeekFrom::Start(inicio))?;
        let mut lector = BufReader::with_capacity(64 * 1024, f);
        let mut linea = Vec::new();
        let mut posicion = inicio;
        loop {
            linea.clear();
            let n = lector.read_until(b'\n', &mut linea)?;
            if n == 0 || linea.last() != Some(&b'\n') {
                break;
            }
            posicion += n as u64;
            self.procesar(&linea, &proyecto, zona);
        }
        if let Some(m) = self.archivos.get_mut(ruta) {
            m.posicion = posicion;
        }
        Ok(())
    }

    fn procesar<Tz: TimeZone>(&mut self, linea: &[u8], carpeta: &str, zona: &Tz) {
        let Ok(texto) = std::str::from_utf8(linea) else { return };
        if !texto.contains("\"usage\"") {
            return;
        }
        let Ok(l) = serde_json::from_str::<Linea>(texto) else { return };
        if l.tipo.as_deref() != Some("assistant") {
            return;
        }
        let Some(m) = l.message else { return };
        let Some(u) = m.usage else { return };
        let Some(modelo) = m.model.filter(|x| !x.is_empty() && !x.starts_with('<')) else { return };
        let t = Tokens {
            entrada: u.input_tokens,
            salida: u.output_tokens,
            cache_escritura: u.cache_creation_input_tokens,
            cache_lectura: u.cache_read_input_tokens,
        };
        if t.vacio() {
            return;
        }
        let Some(dia) = l.timestamp.as_deref().and_then(|x| dia_local(x, zona)) else { return };
        if let (Some(id), Some(req)) = (&m.id, &l.request_id) {
            if !self.vistos.insert(huella(id, req)) {
                return;
            }
        }
        if !self.nombres.contains_key(carpeta) {
            if let Some(n) = l.cwd.as_deref().and_then(|c| Path::new(c).file_name()).map(|n| n.to_string_lossy().to_string()) {
                self.nombres.insert(carpeta.to_string(), n);
            }
        }
        self.totales.entry((dia, modelo, carpeta.to_string())).or_default().sumar(&t);
    }

    pub fn resumen(&self, precios: &[PrecioModelo], hoy: NaiveDate) -> ResumenClaudeCode {
        let desde = hoy - chrono::Duration::days(DIAS_RESUMEN - 1);
        let inicio_mes = hoy.with_day(1).unwrap_or(hoy);
        let mut total_hoy = Totales::default();
        let mut total_mes = Totales::default();
        let mut por_dia: BTreeMap<NaiveDate, Totales> = BTreeMap::new();
        let mut filas = Vec::new();
        let mut sin_precio: Vec<String> = Vec::new();
        for ((dia, modelo, carpeta), t) in &self.totales {
            let precio = precio_para(modelo, precios);
            if precio.is_none() && !sin_precio.contains(modelo) {
                sin_precio.push(modelo.clone());
            }
            let c = precio.map(|p| costo(t, p)).unwrap_or(0.0);
            if *dia == hoy {
                total_hoy.sumar(t, c);
            }
            if *dia >= inicio_mes && *dia <= hoy {
                total_mes.sumar(t, c);
            }
            if *dia >= desde && *dia <= hoy {
                por_dia.entry(*dia).or_default().sumar(t, c);
                filas.push(Fila {
                    dia: dia.to_string(),
                    modelo: modelo.clone(),
                    proyecto: self.nombres.get(carpeta).cloned().unwrap_or_else(|| nombre_de_carpeta(carpeta)),
                    tokens: *t,
                    costo_usd: c,
                });
            }
        }
        let dias = (0..DIAS_RESUMEN)
            .map(|i| desde + chrono::Duration::days(i))
            .map(|d| {
                let t = por_dia.remove(&d).unwrap_or_default();
                Dia { dia: d.to_string(), tokens: t.tokens, costo_usd: t.costo_usd }
            })
            .collect();
        ResumenClaudeCode { hoy: hoy.to_string(), total_hoy, total_mes, dias, filas, sin_precio }
    }
}

pub struct ClaudeCode {
    raiz: PathBuf,
    lector: Lector,
}

impl ClaudeCode {
    pub fn nuevo(inicio: &Path) -> Self {
        Self { raiz: inicio.join(".claude").join("projects"), lector: Lector::default() }
    }
}

impl Fuente for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude_code"
    }

    fn widget(&self) -> &'static str {
        "spending"
    }

    fn intervalo(&self, cfg: &Config) -> Duration {
        Duration::from_millis(cfg.intervalos.spending_ms)
    }

    fn recolectar(&mut self, cfg: &Config) -> Result<serde_json::Value, Motivo> {
        let hoy = Local::now().date_naive();
        self.lector.actualizar(&self.raiz, &Local, hoy);
        serde_json::to_value(self.lector.resumen(&cfg.precios_modelos, hoy)).map_err(|_| Motivo::Interno)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::{dir_temporal, precios_por_defecto};
    use chrono::FixedOffset;
    use std::io::Write;

    const SECRETO: &str = "texto-secreto-del-mensaje-42";

    fn zona() -> FixedOffset {
        FixedOffset::west_opt(6 * 3_600).unwrap()
    }

    fn hoy() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn linea(id: &str, req: &str, modelo: &str, ts: &str, e: u64, s: u64, ce: u64, cl: u64) -> String {
        serde_json::json!({
            "type": "assistant",
            "uuid": "u",
            "timestamp": ts,
            "requestId": req,
            "cwd": "C:\\proyectos\\glassdeck",
            "message": {
                "id": id,
                "model": modelo,
                "role": "assistant",
                "content": [{ "type": "text", "text": SECRETO }],
                "usage": {
                    "input_tokens": e,
                    "output_tokens": s,
                    "cache_creation_input_tokens": ce,
                    "cache_read_input_tokens": cl,
                    "service_tier": "standard"
                }
            }
        })
        .to_string()
            + "\n"
    }

    fn escribir(ruta: &Path, texto: &str) {
        fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        fs::write(ruta, texto).unwrap();
    }

    fn agregar(ruta: &Path, texto: &str) {
        let mut f = fs::OpenOptions::new().append(true).open(ruta).unwrap();
        f.write_all(texto.as_bytes()).unwrap();
    }

    fn suma(l: &Lector) -> Tokens {
        let mut t = Tokens::default();
        for v in l.totales.values() {
            t.sumar(v);
        }
        t
    }

    #[test]
    fn suma_por_dia_modelo_y_proyecto_sin_duplicados_ni_lineas_rotas() {
        let d = dir_temporal("cc-suma");
        let a = d.join("C--proyectos-glassdeck").join("s1.jsonl");
        let b = d.join("C--proyectos-otro").join("s2").join("subagents").join("x.jsonl");
        let mut texto = linea("m1", "r1", "claude-opus-5-5", "2026-10-28T16:00:00Z", 10, 20, 30, 40);
        texto += &linea("m1", "r1", "claude-opus-5-5", "2026-10-28T16:00:01Z", 10, 20, 30, 40);
        texto += "{ esto no es json\n";
        texto += "{\"type\":\"user\",\"message\":{\"content\":\"hola\",\"usage\":{\"input_tokens\":999}}}\n";
        texto += &linea("m0", "r0", "<synthetic>", "2026-10-28T16:00:00Z", 5, 5, 0, 0);
        texto += &linea("m2", "r2", "claude-haiku-5-5", "2026-10-28T03:00:00Z", 1, 2, 0, 0);
        escribir(&a, &texto);
        escribir(&b, &linea("m3", "r3", "claude-opus-5-5", "2026-10-28T17:00:00Z", 100, 0, 0, 0));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        let opus = l.totales[&(hoy(), "claude-opus-5-5".into(), "C--proyectos-glassdeck".into())];
        assert_eq!(opus, Tokens { entrada: 10, salida: 20, cache_escritura: 30, cache_lectura: 40 });
        let ayer = hoy().pred_opt().unwrap();
        assert!(l.totales.contains_key(&(ayer, "claude-haiku-5-5".into(), "C--proyectos-glassdeck".into())));
        assert_eq!(l.totales[&(hoy(), "claude-opus-5-5".into(), "C--proyectos-otro".into())].entrada, 100);
        assert_eq!(l.totales.len(), 3);
        assert_eq!(suma(&l), Tokens { entrada: 111, salida: 22, cache_escritura: 30, cache_lectura: 40 });
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn la_lectura_es_incremental_y_espera_lineas_completas() {
        let d = dir_temporal("cc-incremental");
        let a = d.join("p").join("s.jsonl");
        escribir(&a, &linea("m1", "r1", "claude-opus-5-5", "2026-10-28T16:00:00Z", 10, 0, 0, 0));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(suma(&l).entrada, 10);
        let siguiente = linea("m2", "r2", "claude-opus-5-5", "2026-10-28T16:01:00Z", 5, 0, 0, 0);
        let (mitad1, mitad2) = siguiente.split_at(40);
        agregar(&a, mitad1);
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(suma(&l).entrada, 10, "una línea a medio escribir no se cuenta");
        agregar(&a, mitad2);
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(suma(&l).entrada, 15);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn un_archivo_truncado_se_relee_sin_contar_doble() {
        let d = dir_temporal("cc-truncado");
        let a = d.join("p").join("s.jsonl");
        let mut texto = linea("m1", "r1", "claude-opus-5-5", "2026-10-28T16:00:00Z", 10, 0, 0, 0);
        texto += &linea("m2", "r2", "claude-opus-5-5", "2026-10-28T16:00:00Z", 20, 0, 0, 0);
        escribir(&a, &texto);
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(suma(&l).entrada, 30);
        escribir(&a, &(linea("m2", "r2", "claude-opus-5-5", "2026-10-28T16:00:00Z", 20, 0, 0, 0)));
        agregar(&a, &linea("m9", "r9", "claude-opus-5-5", "2026-10-28T16:05:00Z", 1, 0, 0, 0));
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(suma(&l).entrada, 31);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn el_texto_de_los_mensajes_no_llega_al_resultado() {
        let d = dir_temporal("cc-secreto");
        escribir(&d.join("p").join("s.jsonl"), &linea("m1", "r1", "claude-opus-5-5", "2026-10-28T16:00:00Z", 1, 1, 0, 0));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        let json = serde_json::to_string(&l.resumen(&precios_por_defecto(), hoy())).unwrap();
        assert!(!json.contains(SECRETO));
        assert!(!format!("{:?}{:?}", l.totales, l.nombres).contains(SECRETO));
        assert!(json.contains("glassdeck"), "el proyecto se nombra con la última carpeta de cwd");
        assert!(!json.contains("proyectos"), "la ruta completa de cwd no se guarda");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn el_costo_usa_la_tabla_editable_y_avisa_modelos_sin_precio() {
        let mut l = Lector::default();
        let t = Tokens { entrada: 1_000_000, salida: 1_000_000, cache_escritura: 1_000_000, cache_lectura: 1_000_000 };
        l.totales.insert((hoy(), "claude-opus-5-5".into(), "p".into()), t);
        l.totales.insert((hoy(), "modelo-raro".into(), "p".into()), t);
        let mut precios = precios_por_defecto();
        let r = l.resumen(&precios, hoy());
        assert!((r.total_hoy.costo_usd - (5.0 + 25.0 + 6.25 + 0.5)).abs() < 1e-9);
        assert_eq!(r.sin_precio, vec!["modelo-raro".to_string()]);
        precios[0].salida = 75.0;
        precios.push(PrecioModelo { patron: "opus-5".into(), entrada: 1.0, salida: 1.0, cache_escritura: 1.0, cache_lectura: 1.0 });
        let r = l.resumen(&precios, hoy());
        assert!((r.total_hoy.costo_usd - 4.0).abs() < 1e-9, "gana el patrón más largo");
        precios.pop();
        let r = l.resumen(&precios, hoy());
        assert!((r.total_hoy.costo_usd - (5.0 + 75.0 + 6.25 + 0.5)).abs() < 1e-9, "cambiar un precio recalcula");
    }

    #[test]
    fn el_resumen_trae_30_dias_y_el_mes_en_curso() {
        let mut l = Lector::default();
        let t = Tokens { entrada: 1, ..Default::default() };
        let septiembre = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        l.totales.insert((septiembre, "claude-haiku-5-5".into(), "p".into()), t);
        l.totales.insert((hoy(), "claude-haiku-5-5".into(), "p".into()), t);
        let r = l.resumen(&precios_por_defecto(), hoy());
        assert_eq!(r.dias.len(), 30);
        assert_eq!(r.dias.last().unwrap().dia, "2026-10-28");
        assert_eq!(r.total_mes.tokens.entrada, 1);
        assert_eq!(r.filas.len(), 2);
    }

    #[test]
    fn el_dia_se_cuenta_en_hora_local() {
        assert_eq!(dia_local("2026-10-28T03:00:00.000Z", &zona()), NaiveDate::from_ymd_opt(2026, 10, 27));
        assert_eq!(dia_local("2026-10-28T07:00:00Z", &zona()), NaiveDate::from_ymd_opt(2026, 10, 28));
        assert_eq!(dia_local("ayer", &zona()), None);
    }

    /// Evidencia de GD-CLA-01: `cargo test referencia -- --ignored --nocapture` imprime solo números por día.
    #[test]
    #[ignore]
    fn referencia_con_los_registros_locales() {
        let inicio = std::env::var("USERPROFILE").map(PathBuf::from).unwrap();
        let hoy = Local::now().date_naive();
        let mut l = Lector::default();
        l.actualizar(&inicio.join(".claude").join("projects"), &Local, hoy);
        let r = l.resumen(&precios_por_defecto(), hoy);
        for d in r.dias.iter().rev().take(5) {
            let t = d.tokens;
            println!("{} entrada={} salida={} cache_escritura={} cache_lectura={}", d.dia, t.entrada, t.salida, t.cache_escritura, t.cache_lectura);
        }
    }

    #[test]
    fn nombre_corto_de_la_carpeta_de_proyecto() {
        assert_eq!(nombre_de_carpeta("C--proyectos-glassdeck"), "glassdeck");
        assert_eq!(nombre_de_carpeta("solo"), "solo");
    }
}

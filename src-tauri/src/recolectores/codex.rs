use super::claude_code::{costo, dia_local, precio_para, Tokens};
use super::servicios::{Fuente, Motivo};
use crate::config::{Config, PrecioModelo};
use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const DIAS_EN_MEMORIA: i64 = 62;
const DIAS_RESUMEN: i64 = 30;
const PROFUNDIDAD_MAX: usize = 6;
const SIN_MODELO: &str = "sin modelo";
/// Codex escribe `{"timestamp":…,"type":…}`: el tipo cabe en los primeros bytes y así los mensajes se saltan
/// sin recorrer la línea completa.
const CABEZA: usize = 96;

/// GD-OAI-02: solo se declaran los campos de uso, el modelo y el id de la sesión. El texto de los mensajes
/// (`response_item`) y campos como `base_instructions` no se declaran, así que serde los salta sin copiarlos.
#[derive(Deserialize)]
struct LineaMeta {
    #[serde(rename = "type")]
    tipo: Option<String>,
    payload: Option<Meta>,
}

#[derive(Deserialize)]
struct Meta {
    id: Option<String>,
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct LineaEvento {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    tipo: Option<String>,
    payload: Option<Evento>,
}

#[derive(Deserialize)]
struct Evento {
    #[serde(rename = "type")]
    tipo: Option<String>,
    info: Option<Info>,
    thread_settings: Option<Ajustes>,
}

#[derive(Deserialize)]
struct Info {
    total_token_usage: Option<UsoCodex>,
}

#[derive(Deserialize)]
struct Ajustes {
    model: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
struct UsoCodex {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    cache_write_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

impl UsoCodex {
    fn con_desglose(&self) -> bool {
        self.input_tokens > 0 || self.output_tokens > 0
    }

    fn total(&self) -> u64 {
        if self.total_tokens > 0 {
            self.total_tokens
        } else {
            self.input_tokens + self.output_tokens
        }
    }

    /// Como en la API de OpenAI, `input_tokens` ya incluye la caché y `output_tokens` el razonamiento.
    fn como_tokens(&self) -> Tokens {
        Tokens {
            entrada: self.input_tokens.saturating_sub(self.cached_input_tokens + self.cache_write_input_tokens),
            salida: self.output_tokens,
            cache_escritura: self.cache_write_input_tokens,
            cache_lectura: self.cached_input_tokens,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Ultimo {
    marca: DateTime<FixedOffset>,
    dia: NaiveDate,
    uso: UsoCodex,
}

#[derive(Debug, Default)]
struct Sesion {
    ultimo: Option<Ultimo>,
    modelo: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct TotalCodex {
    pub tokens: u64,
    /// Solo existe si al menos una sesión trajo desglose y su modelo tiene precio (GD-OAI-03).
    pub costo_usd: Option<f64>,
}

impl TotalCodex {
    fn sumar(&mut self, tokens: u64, costo: Option<f64>) {
        self.tokens += tokens;
        if let Some(c) = costo {
            self.costo_usd = Some(self.costo_usd.unwrap_or(0.0) + c);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiaCodex {
    pub dia: String,
    pub tokens: u64,
    pub costo_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilaCodex {
    pub dia: String,
    pub modelo: String,
    pub sesiones: u32,
    pub tokens: u64,
    pub costo_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResumenCodex {
    pub hoy: String,
    pub total_hoy: TotalCodex,
    pub total_mes: TotalCodex,
    pub dias: Vec<DiaCodex>,
    pub filas: Vec<FilaCodex>,
    /// Alguna sesión de los últimos 30 días trajo entrada y salida por separado.
    pub con_desglose: bool,
    pub sin_precio: Vec<String>,
}

struct Marca {
    posicion: u64,
    creado: Option<SystemTime>,
    sesion: String,
}

#[derive(Default)]
pub struct Lector {
    archivos: HashMap<PathBuf, Marca>,
    sesiones: HashMap<String, Sesion>,
}

/// Nunca sigue enlaces ni uniones: solo archivos que viven de verdad bajo `raiz`.
fn jsonl_bajo(raiz: &Path) -> Vec<PathBuf> {
    let mut salida = Vec::new();
    if !fs::symlink_metadata(raiz).map(|m| m.is_dir()).unwrap_or(false) {
        return salida;
    }
    let mut pendientes = vec![(raiz.to_path_buf(), 0usize)];
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
                salida.push(ruta);
            }
        }
    }
    salida
}

fn contiene(texto: &[u8], patron: &[u8]) -> bool {
    texto.windows(patron.len()).any(|v| v == patron)
}

impl Lector {
    pub fn actualizar<Tz: TimeZone>(&mut self, raiz: &Path, zona: &Tz, hoy: NaiveDate) {
        let archivos = jsonl_bajo(raiz);
        let vigentes: HashSet<&PathBuf> = archivos.iter().collect();
        self.archivos.retain(|r, _| vigentes.contains(r));
        for ruta in &archivos {
            let _ = self.leer_archivo(ruta, zona);
        }
        let limite = hoy - chrono::Duration::days(DIAS_EN_MEMORIA);
        self.sesiones.retain(|_, s| s.ultimo.map(|u| u.dia >= limite).unwrap_or(true));
    }

    fn leer_archivo<Tz: TimeZone>(&mut self, ruta: &Path, zona: &Tz) -> std::io::Result<()> {
        let meta = fs::metadata(ruta)?;
        let creado = meta.created().ok();
        let provisional = ruta.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let marca = self.archivos.entry(ruta.to_path_buf()).or_insert_with(|| Marca { posicion: 0, creado, sesion: provisional });
        if marca.creado != creado || meta.len() < marca.posicion {
            marca.posicion = 0;
            marca.creado = creado;
        }
        if meta.len() == marca.posicion {
            return Ok(());
        }
        let inicio = marca.posicion;
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
            self.procesar(ruta, &linea, zona);
        }
        if let Some(m) = self.archivos.get_mut(ruta) {
            m.posicion = posicion;
        }
        Ok(())
    }

    fn procesar<Tz: TimeZone>(&mut self, ruta: &Path, linea: &[u8], zona: &Tz) {
        if contiene(&linea[..linea.len().min(CABEZA)], b"\"response_item\"") {
            return;
        }
        if contiene(linea, b"\"session_meta\"") {
            self.procesar_meta(ruta, linea);
        } else if contiene(linea, b"\"token_count\"") || contiene(linea, b"\"thread_settings_applied\"") {
            self.procesar_evento(ruta, linea, zona);
        }
    }

    fn procesar_meta(&mut self, ruta: &Path, linea: &[u8]) {
        let Ok(l) = serde_json::from_slice::<LineaMeta>(linea) else { return };
        if l.tipo.as_deref() != Some("session_meta") {
            return;
        }
        let Some(id) = l.payload.and_then(|p| p.id.or(p.session_id)).filter(|i| !i.is_empty()) else { return };
        if let Some(m) = self.archivos.get_mut(ruta) {
            m.sesion = id;
        }
    }

    fn procesar_evento<Tz: TimeZone>(&mut self, ruta: &Path, linea: &[u8], zona: &Tz) {
        let Ok(l) = serde_json::from_slice::<LineaEvento>(linea) else { return };
        if l.tipo.as_deref() != Some("event_msg") {
            return;
        }
        let Some(evento) = l.payload else { return };
        let Some(id) = self.archivos.get(ruta).map(|m| m.sesion.clone()) else { return };
        match evento.tipo.as_deref() {
            Some("thread_settings_applied") => {
                if let Some(modelo) = evento.thread_settings.and_then(|a| a.model).filter(|m| !m.is_empty()) {
                    self.sesiones.entry(id).or_default().modelo = Some(modelo);
                }
            }
            Some("token_count") => {
                let Some(uso) = evento.info.and_then(|i| i.total_token_usage) else { return };
                let Some(ts) = l.timestamp.as_deref() else { return };
                let (Ok(marca), Some(dia)) = (DateTime::parse_from_rfc3339(ts), dia_local(ts, zona)) else { return };
                let sesion = self.sesiones.entry(id).or_default();
                // El total es acumulado: gana el más reciente aunque llegue de otro archivo o se relea.
                if sesion.ultimo.map(|u| marca >= u.marca).unwrap_or(true) {
                    sesion.ultimo = Some(Ultimo { marca, dia, uso });
                }
            }
            _ => {}
        }
    }

    pub fn resumen(&self, precios: &[PrecioModelo], hoy: NaiveDate) -> ResumenCodex {
        let desde = hoy - chrono::Duration::days(DIAS_RESUMEN - 1);
        let inicio_mes = hoy.with_day(1).unwrap_or(hoy);
        let mut total_hoy = TotalCodex::default();
        let mut total_mes = TotalCodex::default();
        let mut por_dia: BTreeMap<NaiveDate, TotalCodex> = BTreeMap::new();
        let mut por_fila: BTreeMap<(NaiveDate, String), (u32, TotalCodex)> = BTreeMap::new();
        let mut con_desglose = false;
        let mut sin_precio: Vec<String> = Vec::new();
        for s in self.sesiones.values() {
            let Some(u) = s.ultimo else { continue };
            let modelo = s.modelo.clone().unwrap_or_else(|| SIN_MODELO.to_string());
            let costo_sesion = if u.uso.con_desglose() {
                let precio = precio_para(&modelo, precios);
                if precio.is_none() && u.dia >= desde && !sin_precio.contains(&modelo) {
                    sin_precio.push(modelo.clone());
                }
                precio.map(|p| costo(&u.uso.como_tokens(), p))
            } else {
                None
            };
            let tokens = u.uso.total();
            if u.dia == hoy {
                total_hoy.sumar(tokens, costo_sesion);
            }
            if u.dia >= inicio_mes && u.dia <= hoy {
                total_mes.sumar(tokens, costo_sesion);
            }
            if u.dia >= desde && u.dia <= hoy {
                con_desglose |= u.uso.con_desglose();
                por_dia.entry(u.dia).or_default().sumar(tokens, costo_sesion);
                let fila = por_fila.entry((u.dia, modelo)).or_default();
                fila.0 += 1;
                fila.1.sumar(tokens, costo_sesion);
            }
        }
        sin_precio.sort();
        let dias = (0..DIAS_RESUMEN)
            .map(|i| desde + chrono::Duration::days(i))
            .map(|d| {
                let t = por_dia.remove(&d).unwrap_or_default();
                DiaCodex { dia: d.to_string(), tokens: t.tokens, costo_usd: t.costo_usd }
            })
            .collect();
        let filas = por_fila
            .into_iter()
            .map(|((dia, modelo), (sesiones, t))| FilaCodex { dia: dia.to_string(), modelo, sesiones, tokens: t.tokens, costo_usd: t.costo_usd })
            .collect();
        ResumenCodex { hoy: hoy.to_string(), total_hoy, total_mes, dias, filas, con_desglose, sin_precio }
    }
}

/// Solo lee `~/.codex/sessions`; nunca `~/.codex/auth.json` ni otra cosa de `~/.codex`.
pub struct Codex {
    raiz: PathBuf,
    lector: Lector,
}

impl Codex {
    pub fn nuevo(inicio: &Path) -> Self {
        Self { raiz: inicio.join(".codex").join("sessions"), lector: Lector::default() }
    }
}

impl Fuente for Codex {
    fn id(&self) -> &'static str {
        "codex"
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
    use std::io::Write;

    const SECRETO: &str = "texto-secreto-del-mensaje-42";

    fn zona() -> FixedOffset {
        FixedOffset::west_opt(6 * 3_600).unwrap()
    }

    fn hoy() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
    }

    fn meta(id: &str) -> String {
        serde_json::json!({
            "timestamp": "2026-10-28T16:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "session_id": id,
                "id": id,
                "timestamp": "2026-10-28T16:00:00.000Z",
                "cwd": "C:\\proyectos\\privado",
                "originator": "Codex Desktop",
                "base_instructions": { "text": SECRETO }
            }
        })
        .to_string()
            + "\n"
    }

    fn modelo(m: &str) -> String {
        serde_json::json!({
            "timestamp": "2026-10-28T16:00:01.000Z",
            "type": "event_msg",
            "payload": { "type": "thread_settings_applied", "thread_settings": { "model": m, "cwd": "C:\\proyectos\\privado" } }
        })
        .to_string()
            + "\n"
    }

    fn conteo_desglose(ts: &str, total: u64, entrada: u64, cache: u64, salida: u64) -> String {
        let uso = serde_json::json!({
            "input_tokens": entrada,
            "cached_input_tokens": cache,
            "cache_write_input_tokens": 0,
            "output_tokens": salida,
            "reasoning_output_tokens": 0,
            "total_tokens": total
        });
        serde_json::json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": { "type": "token_count", "info": { "total_token_usage": uso, "last_token_usage": uso, "model_context_window": 1 }, "rate_limits": null }
        })
        .to_string()
            + "\n"
    }

    fn conteo(ts: &str, total: u64) -> String {
        conteo_desglose(ts, total, 0, 0, 0)
    }

    /// `json!` ordena las claves (`payload` antes que `type`) y la carga trae `"token_count"` literal: el
    /// mensaje pasa el filtro por subcadena y tiene que descartarse por su tipo.
    fn mensaje() -> String {
        serde_json::json!({
            "timestamp": "2026-10-28T16:00:02.000Z",
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": SECRETO }],
                "token_count": 1,
                "info": { "total_token_usage": { "total_tokens": 999_999 } }
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

    fn archivo(d: &Path, dia: &str, id: &str) -> PathBuf {
        d.join("2026").join("10").join(dia).join(format!("rollout-2026-10-{dia}T10-00-00-{id}.jsonl"))
    }

    fn tokens_del_dia(r: &ResumenCodex, dia: &str) -> u64 {
        r.dias.iter().find(|d| d.dia == dia).map(|d| d.tokens).unwrap_or(0)
    }

    #[test]
    fn toma_el_ultimo_token_count_por_sesion_y_suma_por_dia_y_modelo() {
        let d = dir_temporal("codex-suma");
        let mut a = meta("s1") + &modelo("gpt-5.1-codex-mini");
        a += &conteo("2026-10-28T16:00:03.000Z", 100);
        a += &mensaje();
        a += "{ esto no es json\n";
        a += &conteo("2026-10-28T16:10:00.000Z", 250);
        escribir(&archivo(&d, "28", "s1"), &a);
        escribir(&archivo(&d, "28", "s2"), &(meta("s2") + &conteo("2026-10-28T17:00:00.000Z", 40)));
        escribir(&archivo(&d, "27", "s3"), &(meta("s3") + &modelo("gpt-5.1-codex-mini") + &conteo("2026-10-28T03:00:00.000Z", 7)));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        let r = l.resumen(&precios_por_defecto(), hoy());
        assert_eq!(r.total_hoy, TotalCodex { tokens: 290, costo_usd: None }, "250 (último de s1, no 350) + 40");
        assert_eq!(tokens_del_dia(&r, "2026-10-27"), 7, "03:00 UTC es el 27 en hora local");
        assert_eq!(r.total_mes.tokens, 297);
        let filas: Vec<(&str, &str, u32, u64)> = r.filas.iter().map(|f| (f.dia.as_str(), f.modelo.as_str(), f.sesiones, f.tokens)).collect();
        assert_eq!(
            filas,
            vec![
                ("2026-10-27", "gpt-5.1-codex-mini", 1, 7),
                ("2026-10-28", "gpt-5.1-codex-mini", 1, 250),
                ("2026-10-28", "sin modelo", 1, 40),
            ]
        );
        assert!(!r.con_desglose);
        assert!(r.sin_precio.is_empty());
        assert_eq!(r.dias.len(), 30);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn el_texto_de_los_mensajes_no_llega_al_resultado() {
        let d = dir_temporal("codex-secreto");
        escribir(&archivo(&d, "28", "s1"), &(meta("s1") + &mensaje() + &conteo("2026-10-28T16:00:03.000Z", 9)));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        let json = serde_json::to_string(&l.resumen(&precios_por_defecto(), hoy())).unwrap();
        assert!(!json.contains(SECRETO));
        assert!(!json.contains("privado"), "ni el cwd de la sesión");
        assert!(!format!("{:?}", l.sesiones).contains(SECRETO));
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 9);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn un_archivo_que_crece_reemplaza_el_total_sin_duplicarlo() {
        let d = dir_temporal("codex-crece");
        let a = archivo(&d, "28", "s1");
        escribir(&a, &(meta("s1") + &conteo("2026-10-28T16:00:03.000Z", 100)));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 100);
        let siguiente = conteo("2026-10-28T16:30:00.000Z", 180);
        let (mitad1, mitad2) = siguiente.split_at(50);
        agregar(&a, mitad1);
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 100, "una línea a medio escribir no se cuenta");
        agregar(&a, mitad2);
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 180);
        escribir(&a, &(meta("s1") + &conteo("2026-10-28T16:30:00.000Z", 180)));
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 180, "releer un archivo truncado no cuenta doble");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn una_sesion_que_sigue_otro_dia_se_mueve_al_dia_de_su_ultimo_conteo() {
        let d = dir_temporal("codex-otro-dia");
        let a = archivo(&d, "27", "s1");
        escribir(&a, &(meta("s1") + &conteo("2026-10-27T20:00:00.000Z", 50)));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(tokens_del_dia(&l.resumen(&[], hoy()), "2026-10-27"), 50);
        agregar(&a, &conteo("2026-10-28T18:00:00.000Z", 80));
        l.actualizar(&d, &zona(), hoy());
        let r = l.resumen(&[], hoy());
        assert_eq!(tokens_del_dia(&r, "2026-10-27"), 0);
        assert_eq!(tokens_del_dia(&r, "2026-10-28"), 80);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn la_misma_sesion_en_dos_archivos_cuenta_una_vez() {
        let d = dir_temporal("codex-dos-archivos");
        escribir(&archivo(&d, "28", "a"), &(meta("s1") + &conteo("2026-10-28T16:00:00.000Z", 60)));
        escribir(&archivo(&d, "28", "b"), &(meta("s1") + &conteo("2026-10-28T17:00:00.000Z", 90)));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 90);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn con_desglose_usa_la_tabla_de_precios_y_sin_precio_no_inventa_costo() {
        let d = dir_temporal("codex-desglose");
        let a = meta("s1") + &modelo("gpt-prueba") + &conteo_desglose("2026-10-28T16:00:00.000Z", 3_000_000, 2_000_000, 1_000_000, 1_000_000);
        escribir(&archivo(&d, "28", "s1"), &a);
        escribir(&archivo(&d, "28", "s2"), &(meta("s2") + &conteo("2026-10-28T16:00:00.000Z", 5)));
        let mut l = Lector::default();
        l.actualizar(&d, &zona(), hoy());
        let r = l.resumen(&precios_por_defecto(), hoy());
        assert!(r.con_desglose);
        assert_eq!(r.total_hoy.costo_usd, None, "los precios por defecto no traen modelos de OpenAI");
        assert_eq!(r.sin_precio, vec!["gpt-prueba".to_string()]);
        let precios = vec![PrecioModelo { patron: "gpt-prueba".into(), entrada: 1.0, salida: 10.0, cache_escritura: 0.0, cache_lectura: 0.1 }];
        let r = l.resumen(&precios, hoy());
        assert!((r.total_hoy.costo_usd.unwrap() - (1.0 + 10.0 + 0.1)).abs() < 1e-9, "1 M entrada sin caché, 1 M salida, 1 M caché");
        assert_eq!(r.total_hoy.tokens, 3_000_005);
        assert!(r.sin_precio.is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn sin_carpeta_de_sesiones_no_hay_datos_ni_error() {
        let d = dir_temporal("codex-vacio");
        let mut l = Lector::default();
        l.actualizar(&d.join("no-existe"), &zona(), hoy());
        let r = l.resumen(&[], hoy());
        assert_eq!(r.total_mes.tokens, 0);
        assert_eq!(r.dias.len(), 30);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(windows)]
    #[test]
    fn no_sigue_uniones_fuera_de_la_carpeta() {
        let d = dir_temporal("codex-union");
        let raiz = d.join("sessions");
        let fuera = d.join("fuera");
        escribir(&fuera.join("x.jsonl"), &(meta("s9") + &conteo("2026-10-28T16:00:00.000Z", 999)));
        escribir(&archivo(&raiz, "28", "s1"), &(meta("s1") + &conteo("2026-10-28T16:00:00.000Z", 1)));
        let ok = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(raiz.join("2026").join("union"))
            .arg(&fuera)
            .stdout(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "mklink /J no necesita permisos de administrador");
        let mut l = Lector::default();
        l.actualizar(&raiz, &zona(), hoy());
        assert_eq!(l.resumen(&[], hoy()).total_hoy.tokens, 1);
        let _ = fs::remove_dir(raiz.join("2026").join("union"));
        let _ = fs::remove_dir_all(&d);
    }

    /// Evidencia de GD-OAI-01: `cargo test qa_codex -- --ignored --nocapture` compara el lector con una suma
    /// independiente (último `token_count` de cada archivo) e imprime solo números por día.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn qa_codex_contra_una_suma_independiente() {
        let raiz = std::env::var("USERPROFILE").map(PathBuf::from).unwrap().join(".codex").join("sessions");
        if !raiz.is_dir() {
            println!("sin ~/.codex/sessions");
            return;
        }
        let hoy = Local::now().date_naive();
        let mut l = Lector::default();
        l.actualizar(&raiz, &Local, hoy);
        let r = l.resumen(&[], hoy);
        let mut independiente: BTreeMap<String, u64> = BTreeMap::new();
        let mut archivos = 0;
        for ruta in jsonl_bajo(&raiz) {
            archivos += 1;
            let texto = fs::read(&ruta).unwrap();
            let ultimo = texto
                .split(|b| *b == b'\n')
                .filter(|x| contiene(x, b"\"token_count\""))
                .filter_map(|x| serde_json::from_slice::<serde_json::Value>(x).ok())
                .filter(|v| v["type"] == "event_msg" && v["payload"]["type"] == "token_count")
                .filter_map(|v| Some((v["timestamp"].as_str()?.to_string(), v["payload"]["info"]["total_token_usage"]["total_tokens"].as_u64()?)))
                .max_by(|a, b| a.0.cmp(&b.0));
            if let Some((ts, total)) = ultimo {
                let dia = dia_local(&ts, &Local).unwrap().to_string();
                *independiente.entry(dia).or_default() += total;
            }
        }
        println!("archivos={archivos} sesiones={}", l.sesiones.len());
        for d in r.dias.iter().filter(|d| d.tokens > 0 || independiente.contains_key(&d.dia)) {
            println!("{} lector={} independiente={}", d.dia, d.tokens, independiente.get(&d.dia).copied().unwrap_or(0));
        }
        for d in &r.dias {
            assert_eq!(d.tokens, independiente.get(&d.dia).copied().unwrap_or(0), "día {}", d.dia);
        }
    }
}

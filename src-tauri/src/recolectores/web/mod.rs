//! Fuentes web de gasto y uso (bloques 13 y 17). Cada servicio vive en su propio adaptador con todas sus
//! direcciones y nombres de campos (GD-CUR-04); aquí solo queda lo común: fuentes, marca de conexión y estado de las
//! cuentas. Las sesiones de Cursor, claude.ai y ChatGPT viven solo en su perfil aislado de WebView2 y se leen ahí:
//! Glassdeck no lee, copia ni guarda cookies (GD-CUR-02, GD-NFR-SEC-01, R-10).

pub mod anthropic_consola;
pub mod chatgpt;
pub mod claude_ai;
pub mod cursor;
pub mod openai_api;

use super::servicios::{EstadoFuente, EstadoServicios, Fuente, Motivo};
use crate::avisos::Uso;
use crate::conexion::{Peticion, Respuesta, TransporteWeb};
use crate::config::{AlmacenEstado, Config};
use crate::seguridad::boveda::{self, Clave};
use chrono::{DateTime, Datelike, Local, SecondsFormat, TimeZone, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Únicas direcciones a las que `conexion` permite un POST: consultas del panel de Cursor (GD-CUR-03).
pub const POSTS_DE_LECTURA: &[&str] = cursor::POSTS_LECTURA;

/// El dock y Customize recargan el estado de las cuentas al recibirlo.
pub const EVENTO_CUENTAS: &str = "gd://cuentas";

/// Una sola ronda web a la vez: como mucho un WebView oculto vivo (presupuesto de RAM, GD-NFR-PERF-02). Desconectar
/// también la toma para borrar el perfil cuando ninguna lectura lo usa.
pub(crate) static RONDA_WEB: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServicioWeb {
    Cursor,
    ClaudeAi,
    Chatgpt,
}

impl ServicioWeb {
    pub const TODOS: [ServicioWeb; 3] = [ServicioWeb::Cursor, ServicioWeb::ClaudeAi, ServicioWeb::Chatgpt];

    /// También es el id de su fuente en `servicios`.
    pub fn id(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => "cursor",
            ServicioWeb::ClaudeAi => "claude_ai",
            ServicioWeb::Chatgpt => "chatgpt",
        }
    }

    pub fn nombre(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => "Cursor",
            ServicioWeb::ClaudeAi => "Claude.ai",
            ServicioWeb::Chatgpt => "ChatGPT",
        }
    }

    pub fn url_login(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => cursor::URL_LOGIN,
            ServicioWeb::ClaudeAi => claude_ai::URL_LOGIN,
            ServicioWeb::Chatgpt => chatgpt::URL_LOGIN,
        }
    }

    /// Origen del WebView oculto de lectura y de los sondeos del login.
    pub fn origen(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => cursor::ORIGEN,
            ServicioWeb::ClaudeAi => claude_ai::ORIGEN,
            ServicioWeb::Chatgpt => chatgpt::ORIGEN,
        }
    }

    /// Expresión de JavaScript que corre en la ventana de login y resuelve a `true` o `false`: si la sesión ya sirve.
    pub fn guion_sesion(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => cursor::GUION_SESION,
            ServicioWeb::ClaudeAi => claude_ai::GUION_SESION,
            ServicioWeb::Chatgpt => chatgpt::GUION_SESION,
        }
    }

    /// Credencial donde las versiones anteriores a SPEC 1.6 copiaban su cookie; solo se borra al migrar.
    pub fn credencial_antigua(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => boveda::SESIONES_WEB_ANTIGUAS[0],
            ServicioWeb::ClaudeAi => boveda::SESIONES_WEB_ANTIGUAS[1],
            ServicioWeb::Chatgpt => boveda::SESIONES_WEB_ANTIGUAS[2],
        }
    }
}

/// La sesión la pone el WebView con el perfil del servicio; Glassdeck solo pide JSON.
pub(crate) fn json_de(p: Peticion) -> Peticion {
    p.encabezado("accept", "application/json")
}

pub(crate) fn pedir(t: &dyn TransporteWeb, p: Peticion) -> Result<Value, Motivo> {
    t.enviar(&p).map(|r: Respuesta| r.cuerpo).map_err(Motivo::from)
}

/// Sesión vencida y Cloudflare cortan la lectura; los demás errores solo dejan sin ese dato.
pub(crate) fn corta(m: Motivo) -> bool {
    matches!(m, Motivo::SesionExpirada | Motivo::Cloudflare)
}

pub(crate) fn campo<'a>(v: &'a Value, ruta: &[&str]) -> Option<&'a Value> {
    ruta.iter().try_fold(v, |actual, k| actual.get(*k)).filter(|x| !x.is_null())
}

/// Número o texto numérico.
pub(crate) fn numero(v: &Value, ruta: &[&str]) -> Option<f64> {
    let x = campo(v, ruta)?;
    x.as_f64().or_else(|| x.as_str().and_then(|s| s.trim().parse::<f64>().ok())).filter(|n| n.is_finite())
}

pub(crate) fn texto(v: &Value, ruta: &[&str]) -> Option<String> {
    campo(v, ruta)?.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

pub(crate) fn iso(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Fecha en RFC 3339, en segundos o milisegundos desde 1970 (como número o texto); se normaliza a UTC.
pub(crate) fn fecha(v: &Value, ruta: &[&str]) -> Option<String> {
    let x = campo(v, ruta)?;
    let desde_numero = |n: f64| {
        let ms = if n > 1e12 { n } else { n * 1000.0 };
        DateTime::from_timestamp_millis(ms as i64).map(iso)
    };
    if let Some(n) = x.as_f64() {
        return desde_numero(n);
    }
    let s = x.as_str()?.trim();
    if let Ok(n) = s.parse::<f64>() {
        return desde_numero(n);
    }
    DateTime::parse_from_rfc3339(s).ok().map(|d| iso(d.with_timezone(&Utc)))
}

/// Identifica la ventana de límite para el motor de avisos: la hora de reinicio al minuto.
pub(crate) fn ciclo_de(reinicio: &Option<String>) -> Option<String> {
    reinicio.as_ref().map(|r| r.chars().take(16).collect())
}

/// Porcentaje usado de una ventana de límite (5 h, semanal) y cuándo se reinicia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VentanaUso {
    pub porcentaje: f64,
    pub reinicio: Option<String>,
}

pub fn inicio_mes_local(ahora: DateTime<Local>) -> (DateTime<Utc>, String) {
    let inicio = ahora
        .date_naive()
        .with_day(1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .and_then(|d| Local.from_local_datetime(&d).earliest())
        .unwrap_or(ahora);
    (inicio.with_timezone(&Utc), format!("{:04}-{:02}", ahora.year(), ahora.month()))
}

pub type Conectada = Arc<dyn Fn() -> bool + Send + Sync>;

pub struct FuenteWeb {
    servicio: ServicioWeb,
    transporte: Arc<dyn TransporteWeb>,
    conectada: Conectada,
}

impl FuenteWeb {
    pub fn nueva(servicio: ServicioWeb, transporte: Arc<dyn TransporteWeb>, conectada: Conectada) -> Self {
        Self { servicio, transporte, conectada }
    }
}

impl Fuente for FuenteWeb {
    fn id(&self) -> &'static str {
        self.servicio.id()
    }

    fn widget(&self) -> &'static str {
        "spending"
    }

    fn intervalo(&self, cfg: &Config) -> Duration {
        Duration::from_millis(cfg.intervalos.spending_ms)
    }

    /// Sin marca de conexión no se toca la red: el dato queda en `null` y la cuenta en gris. La marca se revisa ya con
    /// el turno tomado: si se desconectó mientras esperaba, no se vuelve a abrir el perfil.
    fn recolectar(&mut self, _cfg: &Config) -> Result<Value, Motivo> {
        let _turno = RONDA_WEB.lock();
        if !(self.conectada)() {
            return Ok(Value::Null);
        }
        let t = &*self.transporte;
        let r = match self.servicio {
            ServicioWeb::Cursor => cursor::leer(t, Utc::now()),
            ServicioWeb::ClaudeAi => claude_ai::leer(t),
            ServicioWeb::Chatgpt => chatgpt::leer(t, Utc::now()),
        };
        t.terminar();
        r
    }

    fn usos(&self, datos: &Value, cfg: &Config) -> Vec<Uso> {
        match self.servicio {
            ServicioWeb::Cursor => cursor::usos(datos, cfg),
            ServicioWeb::ClaudeAi => claude_ai::usos(datos),
            ServicioWeb::Chatgpt => chatgpt::usos(datos),
        }
    }
}

/// APIs oficiales de costos con clave de administrador (GD-CLA-05, GD-OAI-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiCostos {
    Anthropic,
    OpenAi,
}

impl ApiCostos {
    pub fn id(self) -> &'static str {
        match self {
            ApiCostos::Anthropic => "anthropic_consola",
            ApiCostos::OpenAi => "openai_api",
        }
    }

    pub fn clave(self) -> Clave {
        match self {
            ApiCostos::Anthropic => Clave::AnthropicAdmin,
            ApiCostos::OpenAi => Clave::OpenaiAdmin,
        }
    }
}

/// Costo real del mes en curso según la API oficial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostoMes {
    pub mes: String,
    pub costo_usd: f64,
}

pub struct FuenteCostos {
    api: ApiCostos,
    transporte: Arc<dyn TransporteWeb>,
}

impl FuenteCostos {
    pub fn nueva(api: ApiCostos, transporte: Arc<dyn TransporteWeb>) -> Self {
        Self { api, transporte }
    }
}

impl Fuente for FuenteCostos {
    fn id(&self) -> &'static str {
        self.api.id()
    }

    fn widget(&self) -> &'static str {
        "spending"
    }

    fn intervalo(&self, cfg: &Config) -> Duration {
        Duration::from_millis(cfg.intervalos.spending_ms)
    }

    fn recolectar(&mut self, _cfg: &Config) -> Result<Value, Motivo> {
        let Some(clave) = boveda::leer(self.api.clave().nombre()).map_err(|_| Motivo::Interno)? else {
            return Ok(Value::Null);
        };
        let (inicio, mes) = inicio_mes_local(Local::now());
        match self.api {
            ApiCostos::Anthropic => anthropic_consola::leer(&*self.transporte, &clave, inicio, &mes),
            ApiCostos::OpenAi => openai_api::leer(&*self.transporte, &clave, inicio, &mes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoConexion {
    Conectada,
    Reconectar,
    SinConectar,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EstadoCuentaWeb {
    pub servicio: ServicioWeb,
    pub estado: EstadoConexion,
    pub motivo: Option<&'static str>,
    pub conectando: bool,
}

/// Solo booleanos: el valor de una clave nunca sale del núcleo (GD-NFR-SEC-01).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClavesOpcionales {
    pub anthropic_admin: bool,
    pub openai_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EstadoCuentas {
    pub web: Vec<EstadoCuentaWeb>,
    pub claves: ClavesOpcionales,
}

/// GD-DOCK-02: ámbar con sesión vencida o Cloudflare en la última lectura; los demás errores no piden reconectar.
pub fn estado_cuenta(presente: bool, ultimo: Option<&EstadoFuente>) -> (EstadoConexion, Option<&'static str>) {
    if !presente {
        return (EstadoConexion::SinConectar, None);
    }
    match ultimo.and_then(|e| e.error) {
        Some(m) if corta(m) => (EstadoConexion::Reconectar, Some(m.texto())),
        _ => (EstadoConexion::Conectada, None),
    }
}

pub fn estado_cuentas(app: &AppHandle) -> EstadoCuentas {
    let servicios = app.state::<EstadoServicios>();
    let marcas = app.state::<AlmacenEstado>();
    let web = ServicioWeb::TODOS
        .iter()
        .map(|s| {
            let presente = marcas.sesion_web(s.id());
            let (estado, motivo) = estado_cuenta(presente, servicios.ultimo(s.id()).as_ref());
            let conectando = app.get_webview_window(&crate::ventanas::login::etiqueta(*s)).is_some();
            EstadoCuentaWeb { servicio: *s, estado, motivo, conectando }
        })
        .collect();
    let claves = ClavesOpcionales {
        anthropic_admin: boveda::existe(Clave::AnthropicAdmin.nombre()),
        openai_admin: boveda::existe(Clave::OpenaiAdmin.nombre()),
    };
    EstadoCuentas { web, claves }
}

/// Cursor, claude.ai y ChatGPT leen solo con el WebView oculto de su perfil aislado; las APIs de costos, por reqwest
/// con su clave de la bóveda.
pub fn fuentes(app: &AppHandle) -> Vec<Box<dyn Fuente>> {
    let mut v: Vec<Box<dyn Fuente>> = ServicioWeb::TODOS
        .iter()
        .map(|s| {
            let oculto = Arc::new(crate::ventanas::login::NavegadorOculto::nuevo(app.clone(), *s));
            let marcas = app.clone();
            let id = s.id();
            let conectada: Conectada = Arc::new(move || marcas.state::<AlmacenEstado>().sesion_web(id));
            Box::new(FuenteWeb::nueva(*s, oculto, conectada)) as Box<dyn Fuente>
        })
        .collect();
    let directo: Arc<dyn TransporteWeb> = Arc::new(crate::conexion::Internet);
    v.extend([ApiCostos::Anthropic, ApiCostos::OpenAi].map(|a| Box::new(FuenteCostos::nueva(a, directo.clone())) as Box<dyn Fuente>));
    v
}

/// SPEC 1.6 (R-10): las versiones anteriores copiaban la cookie de sesión a la bóveda. Se borra sin leerla y, si el
/// perfil aislado del servicio existe, queda «por verificar»: marcado como conectado hasta que la siguiente lectura
/// decida verde o ámbar. Devuelve los servicios que quedaron por verificar.
pub fn migrar_sesiones_antiguas(
    servicios: &[ServicioWeb],
    perfil_existe: impl Fn(ServicioWeb) -> bool,
    marcas: &AlmacenEstado,
) -> Vec<ServicioWeb> {
    let mut por_verificar = Vec::new();
    for s in servicios {
        match boveda::borrar_sesion_antigua(s.credencial_antigua()) {
            Ok(true) if perfil_existe(*s) => {
                if let Err(e) = marcas.marcar_sesion_web(s.id(), true) {
                    log::warn!("{e}");
                }
                log::info!("Conexión: {} queda por verificar con su perfil aislado", s.nombre());
                por_verificar.push(*s);
            }
            Ok(_) => {}
            Err(e) => log::warn!("Conexión: no se pudo borrar la sesión antigua de {}: {e}", s.nombre()),
        }
    }
    por_verificar
}

pub fn migrar_al_arrancar(app: &AppHandle) {
    let existe = |s: ServicioWeb| crate::ventanas::login::dir_perfil(app, s).is_some_and(|d| d.is_dir());
    migrar_sesiones_antiguas(&ServicioWeb::TODOS, existe, &app.state::<AlmacenEstado>());
}

/// Fuente que lee con cada credencial: al guardarla o borrarla se relee de inmediato.
pub fn fuente_de_clave(c: Clave) -> Option<&'static str> {
    [ApiCostos::Anthropic, ApiCostos::OpenAi].into_iter().find(|a| a.clave() == c).map(|a| a.id())
}

pub fn credencial_cambio(app: &AppHandle, c: Clave) {
    if let Some(f) = fuente_de_clave(c) {
        app.state::<EstadoServicios>().forzar(f);
    }
    let _ = app.emit(EVENTO_CUENTAS, ());
}

#[cfg(test)]
pub(crate) mod prueba {
    use crate::conexion::{validar_peticion, ErrorRed, Metodo, Peticion, Respuesta, TransporteWeb};
    use parking_lot::Mutex;
    use serde_json::Value;

    pub struct Pedida {
        pub metodo: Metodo,
        pub url: String,
        pub encabezados: Vec<(&'static str, String)>,
        pub cuerpo: Option<Value>,
    }

    /// Respuestas grabadas sintéticas por «MÉTODO dirección-sin-consulta» (o «PAGINA origen» para las lecturas
    /// compuestas); valida dominio y lista de POST como el real. Si una dirección tiene varias respuestas se entregan
    /// en orden (paginación); la última se repite.
    pub struct Grabado {
        respuestas: Mutex<Vec<(String, Result<Value, ErrorRed>)>>,
        pub pedidas: Mutex<Vec<Pedida>>,
        pub guiones: Mutex<Vec<&'static str>>,
        pub terminadas: Mutex<usize>,
    }

    impl Grabado {
        pub fn nuevo(respuestas: Vec<(&str, Result<Value, ErrorRed>)>) -> Self {
            Self {
                respuestas: Mutex::new(respuestas.into_iter().map(|(k, v)| (k.to_string(), v)).collect()),
                pedidas: Mutex::new(Vec::new()),
                guiones: Mutex::new(Vec::new()),
                terminadas: Mutex::new(0),
            }
        }

        pub fn urls(&self) -> Vec<String> {
            self.pedidas.lock().iter().map(|p| p.url.clone()).collect()
        }

        fn responder(&self, clave: &str) -> Result<Respuesta, ErrorRed> {
            let mut lista = self.respuestas.lock();
            let indices: Vec<usize> = lista.iter().enumerate().filter(|(_, (k, _))| *k == clave).map(|(i, _)| i).collect();
            let r = match indices.as_slice() {
                [] => Err(ErrorRed::Http(404)),
                [i] => lista[*i].1.clone(),
                [i, ..] => lista.remove(*i).1,
            };
            r.map(Respuesta::de_prueba)
        }
    }

    impl TransporteWeb for Grabado {
        fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed> {
            let u = validar_peticion(p)?;
            self.pedidas.lock().push(Pedida { metodo: p.metodo, url: u.to_string(), encabezados: p.encabezados.clone(), cuerpo: p.cuerpo.clone() });
            let metodo = if p.metodo == Metodo::Get { "GET" } else { "POST" };
            let mut sin_consulta = u.clone();
            sin_consulta.set_query(None);
            self.responder(&format!("{metodo} {sin_consulta}"))
        }

        fn en_pagina(&self, origen: &'static str, guion: &'static str) -> Result<Respuesta, ErrorRed> {
            crate::conexion::validar_url(origen, &[])?;
            self.guiones.lock().push(guion);
            self.responder(&format!("PAGINA {origen}"))
        }

        fn terminar(&self) {
            *self.terminadas.lock() += 1;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::prueba::Grabado;
    use super::*;
    use crate::conexion::ErrorRed;
    use crate::seguridad::boveda::mock;
    use serde_json::json;

    fn siempre(conectada: bool) -> Conectada {
        Arc::new(move || conectada)
    }

    #[test]
    fn fechas_en_segundos_milisegundos_o_texto_se_normalizan() {
        let v = json!({ "s": 1_761_696_000, "ms": "1761696000000", "iso": "2025-10-29T00:00:00.000+00:00", "malo": "ayer" });
        for k in ["s", "ms", "iso"] {
            assert_eq!(fecha(&v, &[k]).as_deref(), Some("2025-10-29T00:00:00Z"), "{k}");
        }
        assert_eq!(fecha(&v, &["malo"]), None);
        assert_eq!(fecha(&v, &["falta"]), None);
        assert_eq!(numero(&json!({ "a": "12.5", "b": null }), &["a"]), Some(12.5));
        assert_eq!(numero(&json!({ "b": null }), &["b"]), None);
    }

    #[test]
    fn el_inicio_de_mes_es_el_dia_1_a_medianoche_local() {
        let ahora = Local.with_ymd_and_hms(2026, 10, 28, 15, 10, 0).unwrap();
        let (inicio, mes) = inicio_mes_local(ahora);
        assert_eq!(mes, "2026-10");
        assert_eq!(inicio.with_timezone(&Local), Local.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn el_estado_de_la_cuenta_pasa_a_ambar_solo_con_sesion_vencida_o_cloudflare() {
        let con = |m: Option<Motivo>| EstadoFuente { error: m, ..Default::default() };
        assert_eq!(estado_cuenta(false, Some(&con(Some(Motivo::SesionExpirada)))), (EstadoConexion::SinConectar, None));
        assert_eq!(estado_cuenta(true, None), (EstadoConexion::Conectada, None));
        assert_eq!(estado_cuenta(true, Some(&con(Some(Motivo::SesionExpirada)))), (EstadoConexion::Reconectar, Some("Sesión expirada")));
        assert_eq!(
            estado_cuenta(true, Some(&con(Some(Motivo::Cloudflare)))),
            (EstadoConexion::Reconectar, Some("Cloudflare bloqueó la lectura"))
        );
        assert_eq!(estado_cuenta(true, Some(&con(Some(Motivo::FormatoCambio)))), (EstadoConexion::Conectada, None));
        assert_eq!(estado_cuenta(true, Some(&con(Some(Motivo::SinConexion)))), (EstadoConexion::Conectada, None));
    }

    #[test]
    fn sin_marca_de_conexion_la_fuente_no_toca_la_red() {
        let t = Arc::new(Grabado::nuevo(vec![]));
        let mut f = FuenteWeb::nueva(ServicioWeb::ClaudeAi, t.clone(), siempre(false));
        assert_eq!(f.recolectar(&Config::default()), Ok(Value::Null));
        assert!(t.urls().is_empty() && t.guiones.lock().is_empty());
        assert_eq!(*t.terminadas.lock(), 0, "ni siquiera se abre el WebView");
        assert!(f.usos(&Value::Null, &Config::default()).is_empty());
        let mut costos = FuenteCostos::nueva(ApiCostos::OpenAi, t.clone());
        assert_eq!(costos.recolectar(&Config::default()), Ok(Value::Null));
        assert!(t.urls().is_empty());
    }

    #[test]
    fn cada_ronda_lee_por_el_transporte_y_lo_cierra_al_terminar_aunque_falle() {
        let organizaciones = json!([{ "uuid": "org-1", "capabilities": ["chat"] }]);
        let uso = json!({ "five_hour": { "utilization": 40.0, "resets_at": "2026-10-28T20:00:00Z" }, "seven_day": null });
        let t = Arc::new(Grabado::nuevo(vec![
            ("GET https://claude.ai/api/organizations", Ok(organizaciones)),
            ("GET https://claude.ai/api/organizations/org-1/usage", Ok(uso)),
        ]));
        let mut f = FuenteWeb::nueva(ServicioWeb::ClaudeAi, t.clone(), siempre(true));
        let d = f.recolectar(&Config::default()).unwrap();
        assert_eq!(d["cinco_horas"]["porcentaje"], 40.0);
        assert_eq!((t.urls().len(), *t.terminadas.lock()), (2, 1));
        let t = Arc::new(Grabado::nuevo(vec![(&format!("PAGINA {}", chatgpt::ORIGEN), Err(ErrorRed::Cloudflare))]));
        let mut f = FuenteWeb::nueva(ServicioWeb::Chatgpt, t.clone(), siempre(true));
        assert_eq!(f.recolectar(&Config::default()), Err(Motivo::Cloudflare));
        assert_eq!(*t.terminadas.lock(), 1);
    }

    #[test]
    fn la_migracion_borra_la_sesion_antigua_sin_leerla_y_la_boveda_no_recibe_sesiones_web() {
        let marcas = AlmacenEstado::nuevo(&crate::config::dir_temporal("web-migracion"));
        let servicios = [ServicioWeb::Cursor, ServicioWeb::ClaudeAi];
        let cursor_sesion = boveda::objetivo(ServicioWeb::Cursor.credencial_antigua());
        mock::sembrar(&cursor_sesion, "glassdeck-partes:1");
        mock::sembrar(&format!("{cursor_sesion}/1"), "WorkosCursorSessionToken=cookie-de-prueba");
        let perfil_existe = |s: ServicioWeb| s == ServicioWeb::Cursor || s == ServicioWeb::ClaudeAi;

        assert_eq!(migrar_sesiones_antiguas(&servicios, perfil_existe, &marcas), vec![ServicioWeb::Cursor]);
        assert!(marcas.sesion_web("cursor"), "Cursor queda por verificar con su perfil");
        assert!(!marcas.sesion_web("claude_ai"), "un perfil sin sesión antigua no se marca");
        assert_eq!(mock::lecturas(&cursor_sesion) + mock::lecturas(&format!("{cursor_sesion}/1")), 0);
        assert_eq!(estado_cuenta(marcas.sesion_web("cursor"), None), (EstadoConexion::Conectada, None));
        assert_eq!(migrar_sesiones_antiguas(&servicios, perfil_existe, &marcas), vec![], "solo migra una vez");
        assert!(marcas.sesion_web("cursor"));

        let resumen = json!({ "billingCycleStart": "2026-10-05T00:00:00Z", "individualUsage": { "plan": { "used": 100, "limit": 2000 } } });
        let t = Arc::new(Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage-summary", Ok(resumen)),
            ("GET https://claude.ai/api/organizations", Err(ErrorRed::SesionExpirada)),
        ]));
        let conectada: Conectada = {
            let marcas = Arc::new(marcas);
            Arc::new(move || marcas.sesion_web("cursor"))
        };
        assert!(FuenteWeb::nueva(ServicioWeb::Cursor, t.clone(), conectada.clone()).recolectar(&Config::default()).is_ok());
        let _ = FuenteWeb::nueva(ServicioWeb::ClaudeAi, t.clone(), siempre(true)).recolectar(&Config::default());
        let sesiones: Vec<String> = mock::objetivos()
            .into_iter()
            .filter(|o| servicios.iter().any(|s| o.starts_with(&boveda::objetivo(s.credencial_antigua()))))
            .collect();
        assert!(sesiones.is_empty(), "{sesiones:?}");
        assert!(t.pedidas.lock().iter().all(|p| !p.encabezados.iter().any(|(k, _)| k.eq_ignore_ascii_case("cookie"))));
    }

    #[test]
    fn ni_cookies_ni_tokens_llegan_al_log_ni_a_lo_que_ve_la_interfaz() {
        let captura = crate::seguridad::registro::captura_de_prueba();
        let t = Grabado::nuevo(vec![
            ("PAGINA https://chatgpt.com", Err(ErrorRed::Cloudflare)),
            ("GET https://claude.ai/api/organizations", Err(ErrorRed::SesionExpirada)),
            ("GET https://cursor.com/api/usage-summary", Err(ErrorRed::Formato)),
        ]);
        let _ = chatgpt::leer(&t, Utc::now());
        let _ = claude_ai::leer(&t);
        let _ = cursor::leer(&t, Utc::now());
        let p = json_de(Peticion::post_lectura("https://cursor.com/api/dashboard/set-hard-limit", json!({})))
            .encabezado("authorization", "Bearer token-de-prueba");
        assert!(crate::conexion::validar_peticion(&p).is_err());
        log::warn!("depuración accidental: {p:?} {:?}", Respuesta::de_prueba(json!({})));

        let fuente = EstadoFuente { datos: Some(json!({ "cinco_horas": null })), error: Some(Motivo::Cloudflare), ..Default::default() };
        let (estado, motivo) = estado_cuenta(true, Some(&fuente));
        let cuentas = EstadoCuentas {
            web: vec![EstadoCuentaWeb { servicio: ServicioWeb::Chatgpt, estado, motivo, conectando: false }],
            claves: ClavesOpcionales { anthropic_admin: true, openai_admin: false },
        };
        let interfaz = format!("{} {}", serde_json::to_string(&cuentas).unwrap(), serde_json::to_string(&fuente).unwrap());

        let capturado = captura.lock().clone();
        assert!(capturado.contains("depuración accidental"), "el registrador de prueba está activo");
        let filtrado = crate::seguridad::filtro::enmascarar(&capturado);
        for texto in [capturado.as_str(), filtrado.as_str(), interfaz.as_str()] {
            assert!(!texto.contains("cookie-de-prueba") && !texto.contains("token-de-prueba"), "{texto}");
        }
    }

    #[test]
    fn cada_credencial_relee_su_propia_fuente() {
        assert_eq!(fuente_de_clave(Clave::AnthropicAdmin), Some("anthropic_consola"));
        assert_eq!(fuente_de_clave(Clave::OpenaiAdmin), Some("openai_api"));
        assert_eq!(fuente_de_clave(Clave::GoogleCliente), None);
    }

    #[test]
    fn las_direcciones_de_login_y_lectura_son_de_dominios_permitidos() {
        for s in ServicioWeb::TODOS {
            for u in [s.url_login(), s.origen()] {
                assert!(crate::conexion::validar_url(u, &[]).is_ok(), "{u}");
            }
            assert!(s.guion_sesion().starts_with(&format!("fetch('{}/", s.origen())), "el sondeo es del propio origen");
            assert!(boveda::SESIONES_WEB_ANTIGUAS.contains(&s.credencial_antigua()));
        }
    }

    /// GD-CUR-02, GD-NFR-SEC-01: revisión automática del código; ninguna API de cookies ni guardado de sesiones web.
    /// Los patrones se arman por partes para que esta prueba no se encuentre a sí misma.
    #[test]
    fn el_codigo_no_usa_ninguna_api_de_cookies() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let prohibidos: Vec<String> = [
            ["cookies", "_for_url"],
            [".cookies", "()"],
            ["Cookie", "Manager"],
            ["GetCookie", "s"],
            ["getCookie", "s"],
            ["getAllCookie", "s"],
            ["\"Network", "."],
            ["\"Storage", "."],
            ["document", ".cookie"],
            ["set_", "cookie"],
            ["SET_", "COOKIE"],
            ["cookie", "_store"],
            ["guardar", "_largo"],
            ["leer", "_largo"],
        ]
        .iter()
        .map(|p| p.concat())
        .collect();
        let mut archivos = Vec::new();
        let mut pendientes = vec![raiz.join("src"), raiz.join("..").join("src")];
        while let Some(dir) = pendientes.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let ruta = e.path();
                if ruta.is_dir() {
                    pendientes.push(ruta);
                } else if ruta.extension().is_some_and(|x| ["rs", "ts", "tsx"].contains(&x.to_str().unwrap_or_default())) {
                    archivos.push(ruta);
                }
            }
        }
        archivos.push(raiz.join("Cargo.toml"));
        assert!(archivos.len() > 50, "se revisó el código de verdad");
        for ruta in &archivos {
            let texto = std::fs::read_to_string(ruta).unwrap();
            for p in &prohibidos {
                assert!(!texto.contains(p.as_str()), "{} usa {p}", ruta.display());
            }
        }
        let cargo = std::fs::read_to_string(raiz.join("Cargo.toml")).unwrap();
        assert!(!cargo.contains("\"cookies\""), "reqwest sin almacén de cookies");
        let llamada = ["CallDevToolsProtocol", "Method("].concat();
        let devtools: usize = archivos.iter().map(|r| std::fs::read_to_string(r).unwrap().matches(llamada.as_str()).count()).sum();
        assert_eq!(devtools, 1, "una sola llamada a DevTools, la de `Runtime.evaluate`");
    }
}

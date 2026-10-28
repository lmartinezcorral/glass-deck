//! Fuentes web de gasto y uso (bloques 13 y 17). Cada servicio vive en su propio adaptador con todas sus
//! direcciones y nombres de campos (GD-CUR-04); aquí solo queda lo común: credenciales, fuentes y estado de las cuentas.

pub mod anthropic_consola;
pub mod chatgpt;
pub mod claude_ai;
pub mod cursor;
pub mod openai_api;

use super::servicios::{EstadoFuente, EstadoServicios, Fuente, Motivo};
use crate::avisos::Uso;
use crate::conexion::{ErrorRed, Peticion, Respuesta, TransporteWeb};
use crate::config::Config;
use crate::seguridad::boveda::{self, Clave, Secreto};
use chrono::{DateTime, Datelike, Local, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Únicas direcciones a las que `conexion` permite un POST: consultas del panel de Cursor (GD-CUR-03).
pub const POSTS_DE_LECTURA: &[&str] = cursor::POSTS_LECTURA;

/// El dock y Customize recargan el estado de las cuentas al recibirlo.
pub const EVENTO_CUENTAS: &str = "gd://cuentas";

/// cursor.com, claude.ai y chatgpt.com reciben las peticiones como si vinieran de Edge.
pub const UA_EDGE: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0";

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

    pub fn clave(self) -> Clave {
        match self {
            ServicioWeb::Cursor => Clave::CursorSesion,
            ServicioWeb::ClaudeAi => Clave::ClaudeAiSesion,
            ServicioWeb::Chatgpt => Clave::ChatgptSesion,
        }
    }

    pub fn url_login(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => cursor::URL_LOGIN,
            ServicioWeb::ClaudeAi => claude_ai::URL_LOGIN,
            ServicioWeb::Chatgpt => chatgpt::URL_LOGIN,
        }
    }

    /// Dirección cuyas cookies se leen del perfil de login; también es el origen del WebView oculto.
    pub fn url_cookies(self) -> &'static str {
        match self {
            ServicioWeb::Cursor => cursor::URL_COOKIES,
            ServicioWeb::ClaudeAi => claude_ai::URL_COOKIES,
            ServicioWeb::Chatgpt => chatgpt::URL_COOKIES,
        }
    }

    pub fn cookies_sesion(self) -> &'static [&'static str] {
        match self {
            ServicioWeb::Cursor => cursor::COOKIES_SESION,
            ServicioWeb::ClaudeAi => claude_ai::COOKIES_SESION,
            ServicioWeb::Chatgpt => chatgpt::COOKIES_SESION,
        }
    }
}

/// Arma el encabezado `Cookie` solo con las cookies de sesión; acepta una cookie partida en `.0`, `.1`… (ChatGPT).
/// `None` si falta alguna: el login todavía no termina.
pub fn seleccionar_cookies(requeridas: &[&str], cookies: &[(String, String)]) -> Option<String> {
    let mut pares = Vec::new();
    for base in requeridas {
        if let Some((n, v)) = cookies.iter().find(|(n, v)| n == base && !v.is_empty()) {
            pares.push(format!("{n}={v}"));
            continue;
        }
        let mut partes: Vec<(u32, &String, &String)> = cookies
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .filter_map(|(n, v)| n.strip_prefix(base)?.strip_prefix('.')?.parse::<u32>().ok().map(|i| (i, n, v)))
            .collect();
        partes.sort_by_key(|p| p.0);
        if partes.is_empty() || partes.iter().enumerate().any(|(i, p)| p.0 as usize != i) {
            return None;
        }
        pares.extend(partes.iter().map(|(_, n, v)| format!("{n}={v}")));
    }
    Some(pares.join("; "))
}

pub(crate) fn como_navegador(p: Peticion, origen: &str, cookie: &Secreto) -> Peticion {
    p.encabezado("user-agent", UA_EDGE)
        .encabezado("accept", "application/json")
        .encabezado("accept-language", "es-MX,es;q=0.9,en;q=0.8")
        .encabezado("referer", format!("{origen}/"))
        .encabezado("cookie", cookie.exponer())
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

pub struct FuenteWeb {
    servicio: ServicioWeb,
    transporte: Arc<dyn TransporteWeb>,
}

impl FuenteWeb {
    pub fn nueva(servicio: ServicioWeb, transporte: Arc<dyn TransporteWeb>) -> Self {
        Self { servicio, transporte }
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

    /// Sin sesión guardada no se toca la red: el dato queda en `null` y la cuenta en gris.
    fn recolectar(&mut self, _cfg: &Config) -> Result<Value, Motivo> {
        let Some(cookie) = boveda::leer_largo(self.servicio.clave().nombre()).map_err(|_| Motivo::Interno)? else {
            return Ok(Value::Null);
        };
        let t = &*self.transporte;
        let r = match self.servicio {
            ServicioWeb::Cursor => cursor::leer(t, &cookie, Utc::now()),
            ServicioWeb::ClaudeAi => claude_ai::leer(t, &cookie),
            ServicioWeb::Chatgpt => chatgpt::leer(t, &cookie, Utc::now()),
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

/// Si el primero topa con Cloudflare, el resto de la ronda pasa por el respaldo (el WebView oculto).
pub struct ConRespaldo {
    principal: Arc<dyn TransporteWeb>,
    respaldo: Arc<dyn TransporteWeb>,
    en_respaldo: AtomicBool,
}

impl ConRespaldo {
    pub fn nuevo(principal: Arc<dyn TransporteWeb>, respaldo: Arc<dyn TransporteWeb>) -> Self {
        Self { principal, respaldo, en_respaldo: AtomicBool::new(false) }
    }
}

impl TransporteWeb for ConRespaldo {
    fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed> {
        if !self.en_respaldo.load(Ordering::Relaxed) {
            match self.principal.enviar(p) {
                Err(ErrorRed::Cloudflare) => {
                    log::info!("Cloudflare rechazó la lectura directa; se reintenta con el WebView oculto");
                    self.en_respaldo.store(true, Ordering::Relaxed);
                }
                r => return r,
            }
        }
        self.respaldo.enviar(p)
    }

    fn terminar(&self) {
        self.en_respaldo.store(false, Ordering::Relaxed);
        self.principal.terminar();
        self.respaldo.terminar();
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
    let web = ServicioWeb::TODOS
        .iter()
        .map(|s| {
            let presente = boveda::existe(s.clave().nombre());
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

/// Cursor, claude.ai y ChatGPT leen primero directo y, si Cloudflare lo impide, con el WebView oculto de su perfil de login.
pub fn fuentes(app: &AppHandle) -> Vec<Box<dyn Fuente>> {
    let directo: Arc<dyn TransporteWeb> = Arc::new(crate::conexion::Internet);
    let mut v: Vec<Box<dyn Fuente>> = ServicioWeb::TODOS
        .iter()
        .map(|s| {
            let oculto = Arc::new(crate::ventanas::login::NavegadorOculto::nuevo(app.clone(), *s));
            Box::new(FuenteWeb::nueva(*s, Arc::new(ConRespaldo::nuevo(directo.clone(), oculto)))) as Box<dyn Fuente>
        })
        .collect();
    v.extend([ApiCostos::Anthropic, ApiCostos::OpenAi].map(|a| Box::new(FuenteCostos::nueva(a, directo.clone())) as Box<dyn Fuente>));
    v
}

/// Fuente que lee con cada credencial: al guardarla o borrarla se relee de inmediato.
pub fn fuente_de_clave(c: Clave) -> Option<&'static str> {
    ServicioWeb::TODOS
        .iter()
        .find(|s| s.clave() == c)
        .map(|s| s.id())
        .or_else(|| [ApiCostos::Anthropic, ApiCostos::OpenAi].into_iter().find(|a| a.clave() == c).map(|a| a.id()))
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

    /// Respuestas grabadas sintéticas por «MÉTODO dirección-sin-consulta»; valida dominio y lista de POST como el real.
    /// Si una dirección tiene varias respuestas se entregan en orden (paginación); la última se repite.
    pub struct Grabado {
        respuestas: Mutex<Vec<(String, Result<Value, ErrorRed>)>>,
        pub pedidas: Mutex<Vec<Pedida>>,
    }

    impl Grabado {
        pub fn nuevo(respuestas: Vec<(&str, Result<Value, ErrorRed>)>) -> Self {
            Self {
                respuestas: Mutex::new(respuestas.into_iter().map(|(k, v)| (k.to_string(), v)).collect()),
                pedidas: Mutex::new(Vec::new()),
            }
        }

        pub fn urls(&self) -> Vec<String> {
            self.pedidas.lock().iter().map(|p| p.url.clone()).collect()
        }
    }

    impl TransporteWeb for Grabado {
        fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed> {
            let u = validar_peticion(p)?;
            self.pedidas.lock().push(Pedida { metodo: p.metodo, url: u.to_string(), encabezados: p.encabezados.clone(), cuerpo: p.cuerpo.clone() });
            let metodo = if p.metodo == Metodo::Get { "GET" } else { "POST" };
            let mut sin_consulta = u.clone();
            sin_consulta.set_query(None);
            let clave = format!("{metodo} {sin_consulta}");
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
}

#[cfg(test)]
mod pruebas {
    use super::prueba::Grabado;
    use super::*;
    use crate::conexion::ErrorRed;
    use serde_json::json;

    fn cookies(pares: &[(&str, &str)]) -> Vec<(String, String)> {
        pares.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn solo_se_toman_las_cookies_de_sesion_y_sin_ellas_el_login_no_termina() {
        let todas = cookies(&[("_ga", "analitica"), ("WorkosCursorSessionToken", "cookie-de-prueba"), ("otra", "x")]);
        assert_eq!(seleccionar_cookies(cursor::COOKIES_SESION, &todas).as_deref(), Some("WorkosCursorSessionToken=cookie-de-prueba"));
        assert_eq!(seleccionar_cookies(claude_ai::COOKIES_SESION, &todas), None);
        assert_eq!(seleccionar_cookies(cursor::COOKIES_SESION, &cookies(&[("WorkosCursorSessionToken", "")])), None);
    }

    #[test]
    fn la_cookie_de_chatgpt_partida_se_junta_en_orden() {
        let base = chatgpt::COOKIES_SESION[0];
        let partes = cookies(&[(&format!("{base}.1"), "parte-b"), ("__cf_bm", "x"), (&format!("{base}.0"), "parte-a")]);
        assert_eq!(seleccionar_cookies(chatgpt::COOKIES_SESION, &partes), Some(format!("{base}.0=parte-a; {base}.1=parte-b")));
        let incompleta = cookies(&[(&format!("{base}.1"), "parte-b")]);
        assert_eq!(seleccionar_cookies(chatgpt::COOKIES_SESION, &incompleta), None);
        let entera = cookies(&[(base, "entera"), (&format!("{base}.0"), "vieja")]);
        assert_eq!(seleccionar_cookies(chatgpt::COOKIES_SESION, &entera), Some(format!("{base}=entera")));
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
    fn con_cloudflare_el_resto_de_la_ronda_pasa_por_el_respaldo_y_el_parseo_no_cambia() {
        let organizaciones = json!([{ "uuid": "org-1", "capabilities": ["chat"] }]);
        let uso = json!({ "five_hour": { "utilization": 40.0, "resets_at": "2026-10-28T20:00:00Z" }, "seven_day": null });
        let directo = Arc::new(Grabado::nuevo(vec![("GET https://claude.ai/api/organizations", Err(ErrorRed::Cloudflare))]));
        let oculto = Arc::new(Grabado::nuevo(vec![
            ("GET https://claude.ai/api/organizations", Ok(organizaciones)),
            ("GET https://claude.ai/api/organizations/org-1/usage", Ok(uso)),
        ]));
        let t = ConRespaldo::nuevo(directo.clone(), oculto.clone());
        let d = claude_ai::leer(&t, &Secreto::de_prueba("sessionKey=cookie-de-prueba")).unwrap();
        assert_eq!(d["cinco_horas"]["porcentaje"], 40.0);
        assert_eq!(directo.urls().len(), 1);
        assert_eq!(oculto.urls().len(), 2);
        t.terminar();
        assert!(!t.en_respaldo.load(Ordering::Relaxed), "cada ronda vuelve a intentar la lectura directa");
    }

    #[test]
    fn sin_sesion_guardada_la_fuente_no_toca_la_red() {
        let t = Arc::new(Grabado::nuevo(vec![]));
        let mut f = FuenteWeb::nueva(ServicioWeb::ClaudeAi, t.clone());
        boveda::borrar_largo(Clave::ClaudeAiSesion.nombre()).unwrap();
        assert_eq!(f.recolectar(&Config::default()), Ok(Value::Null));
        assert!(t.urls().is_empty());
        assert!(f.usos(&Value::Null, &Config::default()).is_empty());
        let mut costos = FuenteCostos::nueva(ApiCostos::OpenAi, t.clone());
        assert_eq!(costos.recolectar(&Config::default()), Ok(Value::Null));
        assert!(t.urls().is_empty());
    }

    #[test]
    fn ni_la_cookie_ni_el_token_llegan_al_log_ni_a_lo_que_ve_la_interfaz() {
        let captura = crate::seguridad::registro::captura_de_prueba();
        let cookie = Secreto::de_prueba("WorkosCursorSessionToken=user_x%3A%3Acookie-de-prueba; sessionKey=cookie-de-prueba");
        let sesion = json!({ "accessToken": "token-de-prueba" });
        let t = Grabado::nuevo(vec![
            ("GET https://chatgpt.com/api/auth/session", Ok(sesion)),
            ("GET https://chatgpt.com/backend-api/wham/usage", Err(ErrorRed::Cloudflare)),
            ("GET https://claude.ai/api/organizations", Err(ErrorRed::SesionExpirada)),
            ("GET https://cursor.com/api/usage-summary", Err(ErrorRed::Formato)),
        ]);
        let respaldo = Arc::new(Grabado::nuevo(vec![]));
        let r = ConRespaldo::nuevo(Arc::new(t), respaldo);
        let _ = chatgpt::leer(&r, &cookie, Utc::now());
        let _ = claude_ai::leer(&r, &cookie);
        let _ = cursor::leer(&r, &cookie, Utc::now());
        let p = como_navegador(Peticion::post_lectura("https://cursor.com/api/dashboard/set-hard-limit", json!({})), "https://cursor.com", &cookie)
            .encabezado("authorization", "Bearer token-de-prueba");
        assert!(crate::conexion::validar_peticion(&p).is_err());
        log::warn!("depuración accidental: {p:?} {cookie:?} {:?}", Respuesta::de_prueba(json!({})));

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
        assert_eq!(fuente_de_clave(Clave::CursorSesion), Some("cursor"));
        assert_eq!(fuente_de_clave(Clave::ChatgptSesion), Some("chatgpt"));
        assert_eq!(fuente_de_clave(Clave::AnthropicAdmin), Some("anthropic_consola"));
        assert_eq!(fuente_de_clave(Clave::OpenaiAdmin), Some("openai_api"));
        assert_eq!(fuente_de_clave(Clave::GoogleCliente), None);
    }

    #[test]
    fn las_direcciones_de_login_y_cookies_son_de_dominios_permitidos() {
        for s in ServicioWeb::TODOS {
            for u in [s.url_login(), s.url_cookies()] {
                assert!(crate::conexion::validar_url(u, &[]).is_ok(), "{u}");
            }
            assert!(!s.cookies_sesion().is_empty());
        }
    }
}

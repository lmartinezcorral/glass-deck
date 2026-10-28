use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;

/// SPEC §5.3: fuera de esta lista Glassdeck no abre ninguna conexión (GD-NFR-SEC-06, GD-INST-04).
pub const DOMINIOS_PERMITIDOS: &[&str] = &[
    "oauth2.googleapis.com",
    "accounts.google.com",
    "gmail.googleapis.com",
    "www.googleapis.com",
    "cursor.com",
    "claude.ai",
    "api.anthropic.com",
    "chatgpt.com",
    "api.openai.com",
    "api.open-meteo.com",
    "geocoding-api.open-meteo.com",
    "ipapi.co",
];

const TIEMPO_CONEXION: Duration = Duration::from_secs(10);
const TIEMPO_TOTAL: Duration = Duration::from_secs(20);
const TAMANO_MAX: usize = 4 * 1024 * 1024;
const REDIRECCIONES_MAX: usize = 5;

/// GD-NFR-REL-02: espera de 1, 2, 5 y 15 min; a partir de ahí se queda en 15.
pub const ESPERAS_REINTENTO: [Duration; 4] =
    [Duration::from_secs(60), Duration::from_secs(120), Duration::from_secs(300), Duration::from_secs(900)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorRed {
    DominioNoPermitido,
    SinConexion,
    TiempoAgotado,
    SesionExpirada,
    ServicioNoDisponible,
    Http(u16),
    Formato,
    /// Desafío o bloqueo de Cloudflare: no es una sesión vencida, la petición no pareció de un navegador.
    Cloudflare,
}

pub fn dominio_permitido(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.host_str().map(|h| DOMINIOS_PERMITIDOS.contains(&h)).unwrap_or(false)
}

pub fn validar_url(url: &str, consulta: &[(&str, String)]) -> Result<Url, ErrorRed> {
    let u = if consulta.is_empty() { Url::parse(url) } else { Url::parse_with_params(url, consulta) }
        .map_err(|_| ErrorRed::DominioNoPermitido)?;
    if dominio_permitido(&u) {
        Ok(u)
    } else {
        Err(ErrorRed::DominioNoPermitido)
    }
}

pub struct ClienteHttp {
    cliente: Client,
}

/// El cliente bloqueante no puede crearse ni soltarse dentro de un runtime async: vive en un estático
/// y solo se usa desde hilos propios o `spawn_blocking`.
pub fn cliente() -> &'static ClienteHttp {
    static CLIENTE: OnceLock<ClienteHttp> = OnceLock::new();
    CLIENTE.get_or_init(ClienteHttp::nuevo)
}

impl ClienteHttp {
    fn nuevo() -> Self {
        let politica = Policy::custom(|intento| {
            if intento.previous().len() >= REDIRECCIONES_MAX || !dominio_permitido(intento.url()) {
                intento.stop()
            } else {
                intento.follow()
            }
        });
        let cliente = Client::builder()
            .connect_timeout(TIEMPO_CONEXION)
            .timeout(TIEMPO_TOTAL)
            .redirect(politica)
            .https_only(true)
            .user_agent(concat!("Glassdeck/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { cliente }
    }

    pub fn obtener_json<T: DeserializeOwned>(&self, url: &str, consulta: &[(&str, String)]) -> Result<T, ErrorRed> {
        let u = validar_url(url, consulta)?;
        let host = u.host_str().unwrap_or_default().to_string();
        let r = self.cliente.get(u).send().map_err(|e| {
            let error = clasificar(&e);
            log::warn!("Red: {host} falló ({error:?})");
            error
        })?;
        let estado = r.status().as_u16();
        if !r.status().is_success() {
            log::warn!("Red: {host} respondió {estado}");
            return Err(por_estado(estado));
        }
        if r.content_length().map(|n| n as usize > TAMANO_MAX).unwrap_or(false) {
            return Err(ErrorRed::Formato);
        }
        let cuerpo = r.bytes().map_err(|e| clasificar(&e))?;
        if cuerpo.len() > TAMANO_MAX {
            return Err(ErrorRed::Formato);
        }
        serde_json::from_slice(&cuerpo).map_err(|_| {
            log::warn!("Red: {host} devolvió un formato inesperado");
            ErrorRed::Formato
        })
    }

    /// Bloques 13 y 17: GET o POST de lectura con encabezados; conserva dominios, redirecciones y límites de tamaño.
    pub fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed> {
        let u = validar_peticion(p)?;
        let host = u.host_str().unwrap_or_default().to_string();
        let mut rb = match p.metodo {
            Metodo::Get => self.cliente.get(u),
            Metodo::PostLectura => self
                .cliente
                .post(u)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(p.cuerpo.as_ref().map(|c| c.to_string()).unwrap_or_else(|| "{}".into())),
        };
        for (nombre, valor) in &p.encabezados {
            let mut v = reqwest::header::HeaderValue::from_str(valor).map_err(|_| ErrorRed::Formato)?;
            v.set_sensitive(encabezado_sensible(nombre));
            rb = rb.header(*nombre, v);
        }
        let r = rb.send().map_err(|e| {
            let error = clasificar(&e);
            log::warn!("Red: {host} falló ({error:?})");
            error
        })?;
        let estado = r.status().as_u16();
        let mut encabezados = Vec::new();
        let mut set_cookie = Vec::new();
        for (nombre, valor) in r.headers() {
            let Ok(v) = valor.to_str() else { continue };
            if nombre == reqwest::header::SET_COOKIE {
                set_cookie.push(v.to_string());
            } else {
                encabezados.push((nombre.as_str().to_string(), v.to_string()));
            }
        }
        if r.content_length().map(|n| n as usize > TAMANO_MAX).unwrap_or(false) {
            return Err(ErrorRed::Formato);
        }
        let cuerpo = r.bytes().map_err(|e| clasificar(&e))?;
        match interpretar(estado, &encabezados, &cuerpo) {
            Ok(json) => Ok(Respuesta { estado, cuerpo: json, encabezados, set_cookie }),
            Err(e) => {
                log::warn!("Red: {host} respondió {estado} ({e:?})");
                Err(e)
            }
        }
    }

    // Bloque 14 (Google): GET con `Authorization: Bearer` y POST de formulario. El token nunca llega a logs:
    // va en el encabezado y solo se registra el host.

    pub fn obtener_json_autorizado<T: DeserializeOwned>(
        &self,
        url: &str,
        consulta: &[(&str, String)],
        token: &str,
    ) -> Result<T, ErrorRed> {
        let u = validar_url(url, consulta)?;
        let host = u.host_str().unwrap_or_default().to_string();
        let r = self.cliente.get(u).bearer_auth(token).send();
        leer_json_autorizado(r, &host)
    }

    /// Un cuerpo vacío (p. ej. `revoke` de Google) se lee como `null`.
    pub fn enviar_formulario<T: DeserializeOwned>(&self, url: &str, formulario: &[(&str, String)]) -> Result<T, ErrorRed> {
        let u = validar_url(url, &[])?;
        let host = u.host_str().unwrap_or_default().to_string();
        let r = self.cliente.post(u).form(formulario).send();
        leer_json_autorizado(r, &host)
    }
}

/// Los recolectores piden datos a través de este rasgo para poder probarse con respuestas grabadas, sin red.
pub trait Transporte: Send + Sync {
    fn obtener(&self, url: &str, consulta: &[(&str, String)]) -> Result<serde_json::Value, ErrorRed>;
}

/// Crea el cliente en el primer uso, siempre desde un hilo de servicio o `spawn_blocking`.
pub struct Internet;

impl Transporte for Internet {
    fn obtener(&self, url: &str, consulta: &[(&str, String)]) -> Result<serde_json::Value, ErrorRed> {
        cliente().obtener_json(url, consulta)
    }
}

fn clasificar(e: &reqwest::Error) -> ErrorRed {
    if e.is_timeout() {
        ErrorRed::TiempoAgotado
    } else if e.is_decode() {
        ErrorRed::Formato
    } else {
        ErrorRed::SinConexion
    }
}

fn por_estado(estado: u16) -> ErrorRed {
    match estado {
        401 | 403 => ErrorRed::SesionExpirada,
        429 | 500..=599 => ErrorRed::ServicioNoDisponible,
        e => ErrorRed::Http(e),
    }
}

#[derive(Debug, Default, Clone)]
pub struct Reintentos {
    fallos: usize,
}

impl Reintentos {
    pub fn al_fallar(&mut self) -> Duration {
        let espera = ESPERAS_REINTENTO[self.fallos.min(ESPERAS_REINTENTO.len() - 1)];
        self.fallos = (self.fallos + 1).min(ESPERAS_REINTENTO.len());
        espera
    }

    pub fn al_tener_exito(&mut self) {
        self.fallos = 0;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_https_hacia_los_dominios_de_la_spec() {
        for d in DOMINIOS_PERMITIDOS {
            assert!(validar_url(&format!("https://{d}/x"), &[]).is_ok(), "{d}");
        }
        for malo in [
            "https://example.com/",
            "http://api.open-meteo.com/v1/forecast",
            "https://api.open-meteo.com.example.com/",
            "https://evil-ipapi.co/",
            "https://sub.cursor.com/",
            "https://usuario:clave@claude.ai/",
            "https://claude.ai:8443/",
            "https://93.184.216.34/",
            "file:///C:/Windows/win.ini",
            "no es una url",
        ] {
            assert_eq!(validar_url(malo, &[]), Err(ErrorRed::DominioNoPermitido), "{malo}");
        }
    }

    #[test]
    fn un_dominio_fuera_de_la_lista_se_rechaza_antes_de_conectar() {
        let r: Result<serde_json::Value, _> = cliente().obtener_json("https://example.com/datos.json", &[]);
        assert_eq!(r.unwrap_err(), ErrorRed::DominioNoPermitido);
    }

    #[test]
    fn los_parametros_se_codifican_sin_cambiar_el_dominio() {
        let u = validar_url("https://geocoding-api.open-meteo.com/v1/search", &[("name", "a&b=https://example.com".into())])
            .unwrap();
        assert_eq!(u.host_str(), Some("geocoding-api.open-meteo.com"));
        assert!(u.query().unwrap().contains("a%26b%3Dhttps"));
    }

    #[test]
    fn los_reintentos_esperan_1_2_5_y_15_min_y_se_quedan_en_15() {
        let mut r = Reintentos::default();
        let minutos: Vec<u64> = (0..6).map(|_| r.al_fallar().as_secs() / 60).collect();
        assert_eq!(minutos, vec![1, 2, 5, 15, 15, 15]);
    }

    #[test]
    fn un_exito_reinicia_la_secuencia() {
        let mut r = Reintentos::default();
        r.al_fallar();
        r.al_fallar();
        r.al_tener_exito();
        assert_eq!(r.al_fallar().as_secs(), 60);
    }

    #[test]
    fn los_codigos_http_se_traducen_a_motivos() {
        assert_eq!(por_estado(401), ErrorRed::SesionExpirada);
        assert_eq!(por_estado(503), ErrorRed::ServicioNoDisponible);
        assert_eq!(por_estado(429), ErrorRed::ServicioNoDisponible);
        assert_eq!(por_estado(404), ErrorRed::Http(404));
    }
}

// --- Bloques 13 y 17: peticiones con encabezados para Cursor, claude.ai, ChatGPT y las APIs de costos ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metodo {
    Get,
    /// POST que solo consulta; además del dominio, la dirección exacta debe estar en `web::POSTS_DE_LECTURA` (GD-CUR-03).
    PostLectura,
}

pub struct Peticion {
    pub metodo: Metodo,
    pub url: String,
    pub consulta: Vec<(&'static str, String)>,
    pub encabezados: Vec<(&'static str, String)>,
    pub cuerpo: Option<serde_json::Value>,
}

impl Peticion {
    pub fn get(url: impl Into<String>) -> Self {
        Self { metodo: Metodo::Get, url: url.into(), consulta: Vec::new(), encabezados: Vec::new(), cuerpo: None }
    }

    pub fn post_lectura(url: impl Into<String>, cuerpo: serde_json::Value) -> Self {
        Self { metodo: Metodo::PostLectura, cuerpo: Some(cuerpo), ..Self::get(url) }
    }

    pub fn consulta(mut self, nombre: &'static str, valor: impl Into<String>) -> Self {
        self.consulta.push((nombre, valor.into()));
        self
    }

    pub fn encabezado(mut self, nombre: &'static str, valor: impl Into<String>) -> Self {
        self.encabezados.push((nombre, valor.into()));
        self
    }
}

/// Cookies, tokens y claves viajan en los encabezados: el `Debug` solo muestra sus nombres.
impl std::fmt::Debug for Peticion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nombres: Vec<&str> = self.encabezados.iter().map(|(n, _)| *n).collect();
        f.debug_struct("Peticion").field("metodo", &self.metodo).field("url", &self.url).field("encabezados", &nombres).finish()
    }
}

/// Solo existe para respuestas 2xx con JSON; `set_cookie` puede traer una sesión renovada y no se imprime.
pub struct Respuesta {
    pub estado: u16,
    pub cuerpo: serde_json::Value,
    pub encabezados: Vec<(String, String)>,
    set_cookie: Vec<String>,
}

impl Respuesta {
    #[cfg(test)]
    pub fn de_prueba(cuerpo: serde_json::Value) -> Self {
        Self { estado: 200, cuerpo, encabezados: Vec::new(), set_cookie: Vec::new() }
    }

    /// Para el WebView oculto: su perfil guarda las cookies renovadas, así que no hay `Set-Cookie` que entregar.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn sin_cookies(estado: u16, cuerpo: serde_json::Value, encabezados: Vec<(String, String)>) -> Self {
        Self { estado, cuerpo, encabezados, set_cookie: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn encabezado(&self, nombre: &str) -> Option<&str> {
        valor_encabezado(&self.encabezados, nombre)
    }

    #[allow(dead_code)]
    pub(crate) fn cookies_nuevas(&self) -> &[String] {
        &self.set_cookie
    }
}

impl std::fmt::Debug for Respuesta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Respuesta").field("estado", &self.estado).field("set_cookie", &self.set_cookie.len()).finish()
    }
}

fn encabezado_sensible(nombre: &str) -> bool {
    ["cookie", "authorization", "x-api-key"].iter().any(|s| nombre.eq_ignore_ascii_case(s))
}

fn valor_encabezado<'a>(encabezados: &'a [(String, String)], nombre: &str) -> Option<&'a str> {
    encabezados.iter().find(|(n, _)| n.eq_ignore_ascii_case(nombre)).map(|(_, v)| v.as_str())
}

pub fn validar_peticion(p: &Peticion) -> Result<Url, ErrorRed> {
    let u = validar_url(&p.url, &p.consulta)?;
    let post_permitido = p.consulta.is_empty() && crate::recolectores::web::POSTS_DE_LECTURA.contains(&u.as_str());
    if p.metodo == Metodo::PostLectura && !post_permitido {
        log::warn!("Red: POST rechazado hacia {} (no está en la lista de lecturas)", u.host_str().unwrap_or_default());
        return Err(ErrorRed::DominioNoPermitido);
    }
    Ok(u)
}

const MARCAS_CLOUDFLARE: &[&str] =
    &["challenge-platform", "cf-chl", "_cf_chl_opt", "cf-browser-verification", "Just a moment...", "Attention Required! | Cloudflare"];
const BYTES_REVISION_HTML: usize = 64 * 1024;

pub fn es_desafio_cloudflare(estado: u16, encabezados: &[(String, String)], cuerpo: &[u8]) -> bool {
    if valor_encabezado(encabezados, "cf-mitigated").map(|v| v.trim().eq_ignore_ascii_case("challenge")).unwrap_or(false) {
        return true;
    }
    let html = valor_encabezado(encabezados, "content-type").map(|v| v.to_ascii_lowercase().contains("text/html")).unwrap_or(false);
    if !html {
        return false;
    }
    let texto = String::from_utf8_lossy(&cuerpo[..cuerpo.len().min(BYTES_REVISION_HTML)]);
    let servidor_cf = valor_encabezado(encabezados, "server").map(|v| v.eq_ignore_ascii_case("cloudflare")).unwrap_or(false);
    MARCAS_CLOUDFLARE.iter().any(|m| texto.contains(m)) || (estado == 403 && servidor_cf)
}

/// Pura para poder probarla con respuestas grabadas: Cloudflare, redirección a un login, códigos HTTP y JSON.
pub fn interpretar(estado: u16, encabezados: &[(String, String)], cuerpo: &[u8]) -> Result<serde_json::Value, ErrorRed> {
    if es_desafio_cloudflare(estado, encabezados, cuerpo) {
        return Err(ErrorRed::Cloudflare);
    }
    if (300..400).contains(&estado) {
        return Err(ErrorRed::SesionExpirada);
    }
    if !(200..300).contains(&estado) {
        return Err(por_estado(estado));
    }
    if cuerpo.len() > TAMANO_MAX {
        return Err(ErrorRed::Formato);
    }
    serde_json::from_slice(cuerpo).map_err(|_| ErrorRed::Formato)
}

/// Los adaptadores web piden por aquí para poder cambiar de mecanismo (reqwest o un WebView oculto) sin tocar el parseo.
pub trait TransporteWeb: Send + Sync {
    fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed>;
    /// Libera lo que el transporte haya abierto para una ronda de lecturas.
    fn terminar(&self) {}
}

impl TransporteWeb for Internet {
    fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed> {
        cliente().enviar(p)
    }
}

#[cfg(test)]
mod pruebas_web {
    use super::*;

    fn enc(pares: &[(&str, &str)]) -> Vec<(String, String)> {
        pares.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn chatgpt_y_la_api_de_openai_estan_permitidos_y_monitoring_aun_no() {
        for d in ["chatgpt.com", "api.openai.com"] {
            assert!(validar_url(&format!("https://{d}/x"), &[]).is_ok(), "{d}");
        }
        for malo in ["https://monitoring.googleapis.com/v3/x", "https://sub.chatgpt.com/", "https://openai.com/", "https://chatgpt.com.example.com/"] {
            assert_eq!(validar_url(malo, &[]), Err(ErrorRed::DominioNoPermitido), "{malo}");
        }
    }

    #[test]
    fn un_post_fuera_de_la_lista_de_lecturas_se_rechaza_antes_de_conectar() {
        for url in ["https://cursor.com/api/dashboard/set-hard-limit", "https://claude.ai/api/organizations", "https://cursor.com/api/dashboard/get-hard-limit/../x"] {
            let p = Peticion::post_lectura(url, serde_json::json!({}));
            assert_eq!(cliente().enviar(&p).unwrap_err(), ErrorRed::DominioNoPermitido, "{url}");
        }
        let con_consulta = Peticion::post_lectura(crate::recolectores::web::POSTS_DE_LECTURA[0], serde_json::json!({})).consulta("x", "1");
        assert_eq!(validar_peticion(&con_consulta).unwrap_err(), ErrorRed::DominioNoPermitido);
        for url in crate::recolectores::web::POSTS_DE_LECTURA {
            assert!(validar_peticion(&Peticion::post_lectura(*url, serde_json::json!({}))).is_ok(), "{url}");
        }
    }

    #[test]
    fn un_dominio_fuera_de_la_lista_se_rechaza_tambien_con_encabezados() {
        let p = Peticion::get("https://example.com/api").encabezado("cookie", "sesion=cookie-de-prueba");
        assert_eq!(cliente().enviar(&p).unwrap_err(), ErrorRed::DominioNoPermitido);
    }

    #[test]
    fn el_debug_de_la_peticion_no_muestra_valores_de_encabezados() {
        let p = Peticion::get("https://claude.ai/api/organizations")
            .encabezado("cookie", "sessionKey=cookie-de-prueba")
            .encabezado("authorization", "Bearer token-de-prueba");
        let d = format!("{p:?}");
        assert!(d.contains("cookie") && d.contains("authorization"));
        assert!(!d.contains("cookie-de-prueba") && !d.contains("token-de-prueba"));
        assert!(encabezado_sensible("Cookie") && encabezado_sensible("X-Api-Key") && !encabezado_sensible("accept"));
    }

    #[test]
    fn detecta_el_desafio_de_cloudflare_por_encabezado_o_por_html() {
        let html = "<!DOCTYPE html><title>Just a moment...</title><script src=\"/cdn-cgi/challenge-platform/x\"></script>";
        assert_eq!(interpretar(403, &enc(&[("cf-mitigated", "challenge")]), b"{}"), Err(ErrorRed::Cloudflare));
        assert_eq!(interpretar(200, &enc(&[("content-type", "text/html; charset=UTF-8")]), html.as_bytes()), Err(ErrorRed::Cloudflare));
        assert_eq!(
            interpretar(403, &enc(&[("content-type", "text/html"), ("server", "cloudflare")]), b"<html>bloqueado</html>"),
            Err(ErrorRed::Cloudflare)
        );
    }

    #[test]
    fn un_403_json_es_sesion_expirada_y_no_cloudflare() {
        let cuerpo = br#"{"type":"error","error":{"type":"permission_error"}}"#;
        let e = enc(&[("content-type", "application/json"), ("server", "cloudflare")]);
        assert_eq!(interpretar(403, &e, cuerpo), Err(ErrorRed::SesionExpirada));
        assert_eq!(interpretar(401, &e, b""), Err(ErrorRed::SesionExpirada));
    }

    #[test]
    fn redireccion_html_y_errores_se_traducen() {
        assert_eq!(interpretar(302, &enc(&[("location", "https://authenticator.cursor.sh/")]), b""), Err(ErrorRed::SesionExpirada));
        assert_eq!(interpretar(200, &enc(&[("content-type", "text/html")]), b"<html>otra cosa</html>"), Err(ErrorRed::Formato));
        assert_eq!(interpretar(503, &[], b""), Err(ErrorRed::ServicioNoDisponible));
        assert_eq!(interpretar(404, &[], b""), Err(ErrorRed::Http(404)));
        assert_eq!(interpretar(200, &[], &vec![b' '; TAMANO_MAX + 1]), Err(ErrorRed::Formato));
        assert_eq!(interpretar(200, &[], br#"{"a":1}"#), Ok(serde_json::json!({ "a": 1 })));
    }
}

// Bloque 14 (Google): lectura común de las respuestas autorizadas y de formulario.

fn leer_json_autorizado<T: DeserializeOwned>(
    r: reqwest::Result<reqwest::blocking::Response>,
    host: &str,
) -> Result<T, ErrorRed> {
    let r = r.map_err(|e| {
        let error = clasificar(&e);
        log::warn!("Red: {host} falló ({error:?})");
        error
    })?;
    let estado = r.status().as_u16();
    if !r.status().is_success() {
        log::warn!("Red: {host} respondió {estado}");
        return Err(por_estado(estado));
    }
    if r.content_length().map(|n| n as usize > TAMANO_MAX).unwrap_or(false) {
        return Err(ErrorRed::Formato);
    }
    let cuerpo = r.bytes().map_err(|e| clasificar(&e))?;
    if cuerpo.len() > TAMANO_MAX {
        return Err(ErrorRed::Formato);
    }
    let cuerpo: &[u8] = if cuerpo.iter().all(u8::is_ascii_whitespace) { b"null" } else { &cuerpo };
    serde_json::from_slice(cuerpo).map_err(|_| {
        log::warn!("Red: {host} devolvió un formato inesperado");
        ErrorRed::Formato
    })
}

#[cfg(test)]
mod pruebas_google {
    use super::*;

    #[test]
    fn bearer_y_formulario_rechazan_dominios_fuera_de_la_lista_antes_de_conectar() {
        let r: Result<serde_json::Value, _> =
            cliente().obtener_json_autorizado("https://example.com/gmail/v1/users/me/messages", &[], "token-ficticio");
        assert_eq!(r.unwrap_err(), ErrorRed::DominioNoPermitido);
        let r: Result<serde_json::Value, _> =
            cliente().enviar_formulario("http://oauth2.googleapis.com/token", &[("grant_type", "x".into())]);
        assert_eq!(r.unwrap_err(), ErrorRed::DominioNoPermitido);
        assert!(!DOMINIOS_PERMITIDOS.contains(&"monitoring.googleapis.com"));
    }
}

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

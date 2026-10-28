use super::ApiGoogle;
use crate::conexion::ErrorRed;
use crate::seguridad::google_cliente::ClienteOAuth;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const URL_AUTORIZACION: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const URL_TOKEN: &str = "https://oauth2.googleapis.com/token";
pub const URL_REVOCAR: &str = "https://oauth2.googleapis.com/revoke";

pub const SCOPE_GMAIL: &str = "https://www.googleapis.com/auth/gmail.metadata";
pub const SCOPE_EVENTOS: &str = "https://www.googleapis.com/auth/calendar.events.readonly";
/// Opcional: sin él solo se lee el calendario principal.
pub const SCOPE_CALENDARIOS: &str = "https://www.googleapis.com/auth/calendar.calendarlist.readonly";
pub const SCOPES: [&str; 5] = ["openid", "email", SCOPE_GMAIL, SCOPE_EVENTOS, SCOPE_CALENDARIOS];

const BYTES_VERIFICADOR: usize = 32;
const BYTES_ESTADO: usize = 24;

pub fn aleatorio_b64(bytes: usize) -> Result<String, String> {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b).map_err(|_| "No se pudo generar un valor aleatorio".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(b))
}

/// RFC 7636: verificador de 43 caracteres sin relleno y reto S256.
pub struct Pkce {
    pub verificador: String,
    pub reto: String,
}

impl Pkce {
    pub fn nuevo() -> Result<Self, String> {
        let verificador = aleatorio_b64(BYTES_VERIFICADOR)?;
        Ok(Self { reto: reto_s256(&verificador), verificador })
    }
}

pub fn reto_s256(verificador: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verificador.as_bytes()))
}

pub fn estado_aleatorio() -> Result<String, String> {
    aleatorio_b64(BYTES_ESTADO)
}

/// `prompt=consent` en cada conexión: Google solo entrega el token de actualización cuando hay consentimiento,
/// y cada «Conectar» es la primera vez para la cuenta que Luis elija (o una reconexión tras revocar).
pub fn url_autorizacion(client_id: &str, redireccion: &str, reto: &str, estado: &str) -> String {
    let scopes = SCOPES.join(" ");
    Url::parse_with_params(
        URL_AUTORIZACION,
        &[
            ("client_id", client_id),
            ("redirect_uri", redireccion),
            ("response_type", "code"),
            ("scope", scopes.as_str()),
            ("code_challenge", reto),
            ("code_challenge_method", "S256"),
            ("state", estado),
            ("access_type", "offline"),
            ("prompt", "consent select_account"),
        ],
    )
    .map(String::from)
    .unwrap_or_default()
}

/// Sin `Debug`: trae tokens.
#[derive(Deserialize)]
pub struct RespToken {
    pub access_token: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

fn leer_token(v: serde_json::Value) -> Result<RespToken, ErrorRed> {
    let t: RespToken = serde_json::from_value(v).map_err(|_| ErrorRed::Formato)?;
    if t.access_token.is_empty() {
        return Err(ErrorRed::Formato);
    }
    Ok(t)
}

pub fn intercambiar_codigo(
    api: &dyn ApiGoogle,
    cliente: &ClienteOAuth,
    codigo: &str,
    verificador: &str,
    redireccion: &str,
) -> Result<RespToken, ErrorRed> {
    leer_token(api.formulario(
        URL_TOKEN,
        &[
            ("grant_type", "authorization_code".into()),
            ("code", codigo.into()),
            ("client_id", cliente.client_id.clone()),
            ("client_secret", cliente.client_secret.clone()),
            ("redirect_uri", redireccion.into()),
            ("code_verifier", verificador.into()),
        ],
    )?)
}

pub fn refrescar(api: &dyn ApiGoogle, cliente: &ClienteOAuth, refresh_token: &str) -> Result<RespToken, ErrorRed> {
    leer_token(api.formulario(
        URL_TOKEN,
        &[
            ("grant_type", "refresh_token".into()),
            ("refresh_token", refresh_token.into()),
            ("client_id", cliente.client_id.clone()),
            ("client_secret", cliente.client_secret.clone()),
        ],
    )?)
}

pub fn revocar(api: &dyn ApiGoogle, token: &str) -> Result<(), ErrorRed> {
    api.formulario(URL_REVOCAR, &[("token", token.into())]).map(|_| ())
}

/// La conexión sirve si Luis dejó marcados Gmail y los eventos; la lista de calendarios es opcional.
pub fn permisos_suficientes(scope: Option<&str>) -> bool {
    match scope {
        None => true,
        Some(s) => {
            let dados: Vec<&str> = s.split_whitespace().collect();
            dados.contains(&SCOPE_GMAIL) && dados.contains(&SCOPE_EVENTOS)
        }
    }
}

pub struct Identidad {
    pub sub: String,
    pub correo: String,
}

#[derive(Deserialize)]
struct Reclamos {
    sub: String,
    email: Option<String>,
    iss: Option<String>,
    aud: Option<serde_json::Value>,
}

pub fn correo_valido(c: &str) -> bool {
    let partes = c.split_once('@');
    c.len() <= 254
        && partes.is_some_and(|(a, b)| !a.is_empty() && b.contains('.') && !b.starts_with('.') && !b.ends_with('.'))
        && c.chars().all(|ch| ch.is_ascii_graphic() && !"\"<>()[]\\,;:/?#&=%".contains(ch))
}

/// El `id_token` llega en la misma respuesta TLS de `oauth2.googleapis.com` que el código canjeado: no se valida su
/// firma con las llaves públicas de Google (sería otra conexión) y solo se usa para saber `sub` y correo.
pub fn identidad(id_token: &str, client_id: &str) -> Result<Identidad, String> {
    let error = || "Google no devolvió la identidad de la cuenta".to_string();
    let carga = id_token.split('.').nth(1).ok_or_else(error)?;
    let bytes = URL_SAFE_NO_PAD.decode(carga.trim_end_matches('=')).map_err(|_| error())?;
    let r: Reclamos = serde_json::from_slice(&bytes).map_err(|_| error())?;
    let emisor_ok = matches!(r.iss.as_deref(), Some("https://accounts.google.com" | "accounts.google.com"));
    let audiencia_ok = match &r.aud {
        Some(serde_json::Value::String(a)) => a == client_id,
        Some(serde_json::Value::Array(v)) => v.iter().any(|a| a.as_str() == Some(client_id)),
        _ => false,
    };
    let sub_ok = !r.sub.is_empty() && r.sub.len() <= 255 && r.sub.chars().all(|c| c.is_ascii_alphanumeric());
    let correo = r.email.unwrap_or_default();
    if !emisor_ok || !audiencia_ok || !sub_ok || !correo_valido(&correo) {
        return Err(error());
    }
    Ok(Identidad { sub: r.sub, correo })
}

/// Identificador estable y opaco: el `sub` de Google no se guarda tal cual en la bóveda ni en logs.
pub fn id_de_cuenta(sub: &str) -> String {
    let h = Sha256::digest(sub.as_bytes());
    let hex: String = h[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("cuenta-{hex}")
}

pub fn id_cuenta_valido(id: &str) -> bool {
    id.len() == 23 && id.starts_with("cuenta-") && id[7..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    pub const CLIENT_ID: &str = "000000000000-prueba.apps.googleusercontent.com";

    pub fn id_token_sintetico(sub: &str, correo: &str) -> String {
        let cabecera = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let carga = serde_json::json!({
            "iss": "https://accounts.google.com",
            "aud": CLIENT_ID,
            "sub": sub,
            "email": correo,
            "email_verified": true
        });
        format!("{cabecera}.{}.firma-ficticia", URL_SAFE_NO_PAD.encode(carga.to_string()))
    }

    #[test]
    fn pkce_s256_coincide_con_el_vector_del_rfc_7636() {
        assert_eq!(reto_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn el_verificador_cumple_el_rfc_y_cambia_cada_vez() {
        let a = Pkce::nuevo().unwrap();
        let b = Pkce::nuevo().unwrap();
        assert_ne!(a.verificador, b.verificador);
        assert!((43..=128).contains(&a.verificador.len()));
        assert!(a.verificador.chars().all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c)));
        assert_eq!(a.reto, reto_s256(&a.verificador));
        assert_ne!(estado_aleatorio().unwrap(), estado_aleatorio().unwrap());
    }

    #[test]
    fn la_url_de_autorizacion_pide_exactamente_los_scopes_minimos() {
        let u = Url::parse(&url_autorizacion(CLIENT_ID, "http://127.0.0.1:50123", "reto-x", "estado-y")).unwrap();
        assert_eq!(u.host_str(), Some("accounts.google.com"));
        let p: std::collections::HashMap<String, String> = u.query_pairs().into_owned().collect();
        let scopes: Vec<&str> = p["scope"].split(' ').collect();
        assert_eq!(
            scopes,
            vec![
                "openid",
                "email",
                "https://www.googleapis.com/auth/gmail.metadata",
                "https://www.googleapis.com/auth/calendar.events.readonly",
                "https://www.googleapis.com/auth/calendar.calendarlist.readonly",
            ]
        );
        assert!(!p["scope"].contains("gmail.readonly"));
        assert!(!p["scope"].contains("monitoring"));
        assert_eq!(p["code_challenge_method"], "S256");
        assert_eq!(p["code_challenge"], "reto-x");
        assert_eq!(p["state"], "estado-y");
        assert_eq!(p["access_type"], "offline");
        assert!(p["prompt"].contains("consent"));
        assert_eq!(p["redirect_uri"], "http://127.0.0.1:50123");
        assert_eq!(p["response_type"], "code");
        assert!(crate::conexion::validar_url(URL_TOKEN, &[]).is_ok());
        assert!(crate::conexion::validar_url(URL_REVOCAR, &[]).is_ok());
    }

    #[test]
    fn los_permisos_exigen_gmail_y_eventos() {
        assert!(permisos_suficientes(Some(&format!("openid {SCOPE_GMAIL} {SCOPE_EVENTOS}"))));
        assert!(!permisos_suficientes(Some(&format!("openid {SCOPE_GMAIL}"))));
        assert!(!permisos_suficientes(Some(&format!("{SCOPE_EVENTOS} {SCOPE_CALENDARIOS}"))));
    }

    #[test]
    fn la_identidad_sale_del_id_token_y_el_id_es_opaco() {
        let t = id_token_sintetico("000000000000000000001", "cuenta.uno@example.com");
        let i = identidad(&t, CLIENT_ID).unwrap();
        assert_eq!(i.correo, "cuenta.uno@example.com");
        let id = id_de_cuenta(&i.sub);
        assert!(id_cuenta_valido(&id));
        assert!(!id.contains("000000000000000000001"));
        assert_eq!(id, id_de_cuenta("000000000000000000001"));
        assert!(identidad(&t, "otro-cliente.apps.googleusercontent.com").is_err());
        assert!(identidad("no.es-un-jwt", CLIENT_ID).is_err());
        assert!(identidad(&id_token_sintetico("1", "sin-arroba"), CLIENT_ID).is_err());
        assert!(identidad(&id_token_sintetico("1", "a\"<x>@example.com"), CLIENT_ID).is_err());
    }

    #[test]
    fn la_respuesta_de_token_grabada_se_lee() {
        let v: serde_json::Value = serde_json::from_str(include_str!("../grabaciones/google_token_sintetico.json")).unwrap();
        let t = leer_token(v).unwrap();
        assert_eq!(t.access_token, "ya29.acceso-ficticio-0000");
        assert_eq!(t.refresh_token.as_deref(), Some("1//refresh-ficticio-0000"));
        assert_eq!(t.expires_in, Some(3599));
        assert!(permisos_suficientes(t.scope.as_deref()));
        assert!(leer_token(serde_json::json!({ "error": "invalid_grant" })).is_err());
    }
}

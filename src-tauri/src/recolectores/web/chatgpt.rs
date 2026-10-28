//! ChatGPT (no oficial, GD-OAI-04): todas las direcciones y nombres de campos de chatgpt.com viven aquí.
//! El token de acceso de `/api/auth/session` solo vive en memoria durante la lectura; nunca se guarda.

use super::{campo, ciclo_de, como_navegador, fecha, iso, numero, pedir, texto, VentanaUso};
use crate::avisos::{Medidor, Uso};
use crate::conexion::{Peticion, TransporteWeb};
use crate::recolectores::servicios::Motivo;
use crate::seguridad::boveda::Secreto;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const URL_LOGIN: &str = "https://chatgpt.com/auth/login";
pub const URL_COOKIES: &str = "https://chatgpt.com/";
/// Puede llegar partida en `.0`, `.1`… cuando no cabe en una sola cookie.
pub const COOKIES_SESION: &[&str] = &["__Secure-next-auth.session-token"];

const ORIGEN: &str = "https://chatgpt.com";
const URL_SESION: &str = "https://chatgpt.com/api/auth/session";
const URL_USO: &str = "https://chatgpt.com/backend-api/wham/usage";

const CAMPO_TOKEN: &str = "accessToken";
const CAMPO_CUENTA: &[&str] = &["account", "id"];
const ENCABEZADO_CUENTA: &str = "chatgpt-account-id";
const LARGO_MAX_TOKEN: usize = 8_192;
const LARGO_MAX_CUENTA: usize = 128;

const CAMPO_PLAN: &str = "plan_type";
const CAMPO_LIMITES: &str = "rate_limit";
const VENTANAS: [&str; 2] = ["primary_window", "secondary_window"];
const CAMPO_USADO: &str = "used_percent";
const CAMPO_DURACION: &str = "limit_window_seconds";
const CAMPO_REINICIO: &str = "reset_at";
const CAMPO_REINICIO_EN: &str = "reset_after_seconds";
/// Ventanas de hasta 6 h cuentan como la de 5 h; las más largas, como la semanal.
const SEGUNDOS_VENTANA_CORTA: f64 = 6.0 * 3600.0;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DatosChatgpt {
    pub plan: Option<String>,
    pub cinco_horas: Option<VentanaUso>,
    pub semanal: Option<VentanaUso>,
}

fn cuenta_valida(c: &str) -> bool {
    c.len() <= LARGO_MAX_CUENTA && c.chars().all(|x| x.is_ascii_alphanumeric() || x == '-' || x == '_')
}

fn ventana(w: &Value, ahora: DateTime<Utc>) -> Option<VentanaUso> {
    let porcentaje = numero(w, &[CAMPO_USADO])?;
    let reinicio = fecha(w, &[CAMPO_REINICIO]).or_else(|| {
        let s = numero(w, &[CAMPO_REINICIO_EN]).filter(|s| *s >= 0.0)?;
        Some(iso(ahora + Duration::seconds(s as i64)))
    });
    Some(VentanaUso { porcentaje, reinicio })
}

/// `None` si no trae `rate_limit`: el formato cambió.
pub fn interpretar_uso(v: &Value, ahora: DateTime<Utc>) -> Option<DatosChatgpt> {
    let limites = v.get(CAMPO_LIMITES)?.as_object()?;
    let mut d = DatosChatgpt { plan: texto(v, &[CAMPO_PLAN]), ..Default::default() };
    for (i, nombre) in VENTANAS.iter().enumerate() {
        let Some(w) = limites.get(*nombre).filter(|w| w.is_object()) else { continue };
        let Some(uso) = ventana(w, ahora) else { continue };
        let corta = numero(w, &[CAMPO_DURACION]).map(|s| s <= SEGUNDOS_VENTANA_CORTA).unwrap_or(i == 0);
        let destino = if corta { &mut d.cinco_horas } else { &mut d.semanal };
        destino.get_or_insert(uso);
    }
    Some(d)
}

pub fn leer(t: &dyn TransporteWeb, cookie: &Secreto, ahora: DateTime<Utc>) -> Result<Value, Motivo> {
    let sesion = pedir(t, como_navegador(Peticion::get(URL_SESION), ORIGEN, cookie))?;
    let token = campo(&sesion, &[CAMPO_TOKEN])
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= LARGO_MAX_TOKEN && s.is_ascii())
        .map(|s| format!("Bearer {s}"))
        .ok_or(Motivo::SesionExpirada)?;
    let mut p = como_navegador(Peticion::get(URL_USO), ORIGEN, cookie).encabezado("authorization", token);
    if let Some(cuenta) = texto(&sesion, CAMPO_CUENTA).filter(|c| cuenta_valida(c)) {
        p = p.encabezado(ENCABEZADO_CUENTA, cuenta);
    }
    drop(sesion);
    let uso = pedir(t, p)?;
    let datos = interpretar_uso(&uso, ahora).ok_or(Motivo::FormatoCambio)?;
    serde_json::to_value(datos).map_err(|_| Motivo::Interno)
}

pub fn usos(datos: &Value) -> Vec<Uso> {
    let Ok(d) = serde_json::from_value::<DatosChatgpt>(datos.clone()) else { return Vec::new() };
    [(Medidor::Chatgpt5h, d.cinco_horas), (Medidor::ChatgptSemanal, d.semanal)]
        .into_iter()
        .filter_map(|(medidor, w)| {
            let w = w?;
            Some(Uso { medidor, ciclo: ciclo_de(&w.reinicio)?, porcentaje: w.porcentaje })
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexion::ErrorRed;
    use crate::recolectores::web::prueba::Grabado;
    use chrono::TimeZone;
    use serde_json::json;

    const COOKIE: &str = "__Secure-next-auth.session-token.0=cookie-de-prueba; __Secure-next-auth.session-token.1=cookie-de-prueba";

    fn ahora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap()
    }

    fn sesion() -> Value {
        json!({ "user": { "name": "Persona de prueba" }, "expires": "2026-12-01T00:00:00Z", "accessToken": "token-de-prueba", "account": { "id": "cuenta-de-prueba", "planType": "plus" } })
    }

    fn uso() -> Value {
        json!({
            "plan_type": "plus",
            "rate_limit": {
                "allowed": true,
                "primary_window": { "used_percent": 23, "limit_window_seconds": 18000, "reset_after_seconds": 3600 },
                "secondary_window": { "used_percent": "61.5", "limit_window_seconds": 604800, "reset_at": 1_793_000_000 }
            },
            "credits": { "has_credits": false }
        })
    }

    #[test]
    fn lee_las_ventanas_de_5_h_y_semanal_con_el_token_solo_en_memoria() {
        let t = Grabado::nuevo(vec![
            ("GET https://chatgpt.com/api/auth/session", Ok(sesion())),
            ("GET https://chatgpt.com/backend-api/wham/usage", Ok(uso())),
        ]);
        let v = leer(&t, &Secreto::de_prueba(COOKIE), ahora()).unwrap();
        let d: DatosChatgpt = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(d.plan.as_deref(), Some("plus"));
        assert_eq!(d.cinco_horas, Some(VentanaUso { porcentaje: 23.0, reinicio: Some("2026-10-28T13:00:00Z".into()) }));
        assert_eq!(d.semanal.as_ref().map(|w| w.porcentaje), Some(61.5));
        assert!(d.semanal.unwrap().reinicio.is_some());
        let pedidas = t.pedidas.lock();
        let de = |i: usize, n: &str| pedidas[i].encabezados.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone());
        assert_eq!(de(0, "authorization"), None);
        assert_eq!(de(1, "authorization").as_deref(), Some("Bearer token-de-prueba"));
        assert_eq!(de(1, "chatgpt-account-id").as_deref(), Some("cuenta-de-prueba"));
        assert_eq!(de(1, "cookie").as_deref(), Some(COOKIE));
        let texto = v.to_string();
        assert!(!texto.contains("token-de-prueba") && !texto.contains("cookie-de-prueba"));
    }

    #[test]
    fn sin_duracion_la_primera_es_de_5_h_y_campos_faltantes_no_rompen() {
        let v = json!({ "rate_limit": { "primary_window": { "used_percent": 10 }, "secondary_window": null } });
        let d = interpretar_uso(&v, ahora()).unwrap();
        assert_eq!(d, DatosChatgpt { plan: None, cinco_horas: Some(VentanaUso { porcentaje: 10.0, reinicio: None }), semanal: None });
        let solo_semanal = json!({ "rate_limit": { "primary_window": { "used_percent": 5, "limit_window_seconds": 604800 } } });
        assert_eq!(interpretar_uso(&solo_semanal, ahora()).unwrap().semanal.map(|w| w.porcentaje), Some(5.0));
        assert_eq!(interpretar_uso(&json!({ "rate_limit": { "primary_window": { "otro": 1 } } }), ahora()), Some(DatosChatgpt::default()));
        assert_eq!(interpretar_uso(&json!({ "limits": {} }), ahora()), None);
    }

    #[test]
    fn sin_rate_limit_es_formato_cambio_y_sin_token_es_sesion_expirada() {
        let t = Grabado::nuevo(vec![
            ("GET https://chatgpt.com/api/auth/session", Ok(sesion())),
            ("GET https://chatgpt.com/backend-api/wham/usage", Ok(json!({ "detail": "x" }))),
        ]);
        assert_eq!(leer(&t, &Secreto::de_prueba(COOKIE), ahora()), Err(Motivo::FormatoCambio));
        let t = Grabado::nuevo(vec![("GET https://chatgpt.com/api/auth/session", Ok(json!({})))]);
        assert_eq!(leer(&t, &Secreto::de_prueba(COOKIE), ahora()), Err(Motivo::SesionExpirada));
        assert_eq!(t.urls().len(), 1, "sin token no se pide el uso");
    }

    #[test]
    fn cloudflare_401_y_cuenta_rara_se_manejan_sin_mandar_basura() {
        for (error, motivo) in [(ErrorRed::Cloudflare, Motivo::Cloudflare), (ErrorRed::SesionExpirada, Motivo::SesionExpirada)] {
            let t = Grabado::nuevo(vec![
                ("GET https://chatgpt.com/api/auth/session", Ok(sesion())),
                ("GET https://chatgpt.com/backend-api/wham/usage", Err(error)),
            ]);
            assert_eq!(leer(&t, &Secreto::de_prueba(COOKIE), ahora()), Err(motivo));
        }
        let mut rara = sesion();
        rara["account"]["id"] = json!("x\r\nset-cookie: y");
        let t = Grabado::nuevo(vec![
            ("GET https://chatgpt.com/api/auth/session", Ok(rara)),
            ("GET https://chatgpt.com/backend-api/wham/usage", Ok(uso())),
        ]);
        assert!(leer(&t, &Secreto::de_prueba(COOKIE), ahora()).is_ok());
        assert!(!t.pedidas.lock()[1].encabezados.iter().any(|(k, _)| *k == "chatgpt-account-id"));
    }

    #[test]
    fn traduce_ambas_ventanas_a_usos_separados() {
        let d = interpretar_uso(&uso(), ahora()).unwrap();
        let u = usos(&serde_json::to_value(d).unwrap());
        assert_eq!(u.iter().map(|u| u.medidor).collect::<Vec<_>>(), vec![Medidor::Chatgpt5h, Medidor::ChatgptSemanal]);
        assert_eq!(u[0].ciclo, "2026-10-28T13:00");
        assert!(usos(&json!({ "plan": 3 })).is_empty());
    }
}

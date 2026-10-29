//! ChatGPT (no oficial, GD-OAI-04): todas las direcciones y nombres de campos de chatgpt.com viven aquí.
//! El token de acceso de `/api/auth/session` se pide y se usa dentro de la página de chatgpt.com: nunca sale de ella.

use super::{ciclo_de, fecha, iso, numero, texto, VentanaUso};
use crate::avisos::{Medidor, Uso};
use crate::conexion::TransporteWeb;
use crate::recolectores::servicios::Motivo;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const URL_LOGIN: &str = "https://chatgpt.com/auth/login";
pub const ORIGEN: &str = "https://chatgpt.com";
/// Sondeo del login (GD-OAI-04): solo dice si la sesión ya trae token, sin devolverlo.
pub const GUION_SESION: &str = "fetch('https://chatgpt.com/api/auth/session', { credentials: 'include', redirect: 'manual', cache: 'no-store' }) \
     .then(async (r) => { if (!r.ok) return false; const j = await r.json().catch(() => null); \
     return !!j && typeof j.accessToken === 'string' && j.accessToken.length > 0; }).catch(() => false)";

#[cfg(test)]
const URL_SESION: &str = "https://chatgpt.com/api/auth/session";
#[cfg(test)]
const URL_USO: &str = "https://chatgpt.com/backend-api/wham/usage";

/// Lectura compuesta para `TransporteWeb::en_pagina`: pide la sesión, usa su token como `Authorization` para pedir
/// el uso y devuelve solo la respuesta del uso. Sin token es como un 401; si la sesión no es JSON, se devuelve tal
/// cual para que se reconozca Cloudflare. `OPC` lo define el transporte (`credentials: 'include'`, sin redirecciones).
pub const GUION_USO: &str = "async () => {
  const sesion = await fetch('https://chatgpt.com/api/auth/session', { ...OPC, headers: { accept: 'application/json' } });
  if (!sesion.ok) return sesion;
  const datos = await sesion.clone().json().catch(() => null);
  if (datos === null || typeof datos !== 'object') return sesion;
  const token = typeof datos.accessToken === 'string' ? datos.accessToken : '';
  if (!token || token.length > 8192 || !/^[\\x21-\\x7e]+$/.test(token)) return { estado: 401 };
  const cuenta = typeof datos.account?.id === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(datos.account.id) ? datos.account.id : '';
  const encabezados = { accept: 'application/json', authorization: 'Bearer ' + token };
  if (cuenta) encabezados['chatgpt-account-id'] = cuenta;
  return fetch('https://chatgpt.com/backend-api/wham/usage', { ...OPC, headers: encabezados });
}";

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

pub fn leer(t: &dyn TransporteWeb, ahora: DateTime<Utc>) -> Result<Value, Motivo> {
    let uso = t.en_pagina(ORIGEN, GUION_USO).map(|r| r.cuerpo).map_err(Motivo::from)?;
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

    const PAGINA: &str = "PAGINA https://chatgpt.com";

    fn ahora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap()
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
    fn lee_las_ventanas_de_5_h_y_semanal_con_una_sola_lectura_dentro_de_la_pagina() {
        let t = Grabado::nuevo(vec![(PAGINA, Ok(uso()))]);
        let v = leer(&t, ahora()).unwrap();
        let d: DatosChatgpt = serde_json::from_value(v).unwrap();
        assert_eq!(d.plan.as_deref(), Some("plus"));
        assert_eq!(d.cinco_horas, Some(VentanaUso { porcentaje: 23.0, reinicio: Some("2026-10-28T13:00:00Z".into()) }));
        assert_eq!(d.semanal.as_ref().map(|w| w.porcentaje), Some(61.5));
        assert!(d.semanal.unwrap().reinicio.is_some());
        assert!(t.urls().is_empty(), "ni la sesión ni el uso se piden desde el núcleo");
        assert_eq!(t.guiones.lock().as_slice(), [GUION_USO]);
    }

    /// Las expresiones que devuelve el guion de uso.
    fn retornos(guion: &str) -> Vec<&str> {
        guion.split("return ").skip(1).map(|r| r.split(';').next().unwrap().trim()).collect()
    }

    #[test]
    fn el_guion_de_uso_nunca_devuelve_el_token_ni_la_sesion() {
        assert!(GUION_USO.contains(URL_SESION) && GUION_USO.contains(URL_USO));
        assert_eq!(GUION_USO.matches("accessToken").count(), 2, "solo se lee dentro de la página");
        let r = retornos(GUION_USO);
        assert_eq!(r.len(), 4);
        for expresion in &r {
            let permitida = *expresion == "sesion"
                || *expresion == "{ estado: 401 }"
                || expresion.starts_with(&format!("fetch('{URL_USO}'"));
            assert!(permitida, "{expresion}");
            assert!(!expresion.contains("accessToken") && !expresion.contains("datos"), "{expresion}");
        }
        let sondeo = retornos(GUION_SESION);
        assert!(sondeo.iter().all(|e| *e == "false" || e.starts_with("!!j && typeof j.accessToken === 'string'")), "{sondeo:?}");
        assert!(!GUION_SESION.contains("return j") && !GUION_SESION.contains("return r"));
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
    fn sin_rate_limit_es_formato_cambio_y_los_errores_de_la_pagina_son_motivos() {
        let t = Grabado::nuevo(vec![(PAGINA, Ok(json!({ "detail": "x" })))]);
        assert_eq!(leer(&t, ahora()), Err(Motivo::FormatoCambio));
        for (error, motivo) in [
            (ErrorRed::SesionExpirada, Motivo::SesionExpirada),
            (ErrorRed::Cloudflare, Motivo::Cloudflare),
            (ErrorRed::SinConexion, Motivo::SinConexion),
        ] {
            let t = Grabado::nuevo(vec![(PAGINA, Err(error))]);
            assert_eq!(leer(&t, ahora()), Err(motivo));
        }
        assert!(GUION_USO.contains("return { estado: 401 }"), "sin token es sesión expirada");
        assert!(GUION_USO.contains("/^[A-Za-z0-9_-]{1,128}$/.test(datos.account.id)"), "una cuenta rara no se manda como encabezado");
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

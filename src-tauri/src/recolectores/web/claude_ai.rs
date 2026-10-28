//! claude.ai (no oficial, GD-CLA-04): todas las direcciones y nombres de campos de claude.ai viven aquí.
//! Direcciones candidatas conocidas por la comunidad, sin confirmar con una sesión real: si no cuadran, «Formato cambió».

use super::{campo, ciclo_de, como_navegador, fecha, numero, pedir, texto, VentanaUso};
use crate::avisos::{Medidor, Uso};
use crate::conexion::{Peticion, TransporteWeb};
use crate::recolectores::servicios::Motivo;
use crate::seguridad::boveda::Secreto;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const URL_LOGIN: &str = "https://claude.ai/login";
pub const URL_COOKIES: &str = "https://claude.ai/";
pub const COOKIES_SESION: &[&str] = &["sessionKey"];

const ORIGEN: &str = "https://claude.ai";
const URL_ORGANIZACIONES: &str = "https://claude.ai/api/organizations";
const RUTA_USO: &str = "usage";

const CAMPO_UUID: &str = "uuid";
const CAMPO_CAPACIDADES: &str = "capabilities";
const CAPACIDADES_PLAN: &[&str] = &["claude_max", "claude_pro"];
const CAPACIDAD_CHAT: &str = "chat";

const VENTANA_5H: &str = "five_hour";
const VENTANA_SEMANAL: &str = "seven_day";
const PREFIJO_SEMANAL_EXTRA: &str = "seven_day_";
const CAMPO_USO: &str = "utilization";
const CAMPO_REINICIO: &str = "resets_at";
const LARGO_MAX_UUID: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VentanaExtra {
    pub nombre: String,
    #[serde(flatten)]
    pub ventana: VentanaUso,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DatosClaudeAi {
    pub cinco_horas: Option<VentanaUso>,
    pub semanal: Option<VentanaUso>,
    #[serde(default)]
    pub extras: Vec<VentanaExtra>,
}

fn uuid_valido(u: &str) -> bool {
    !u.is_empty() && u.len() <= LARGO_MAX_UUID && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Prefiere la organización con plan de suscripción, luego la que tenga chat, luego la primera.
pub fn elegir_organizacion(lista: &Value) -> Option<String> {
    let orgs = lista.as_array()?;
    let tiene = |o: &Value, cap: &str| {
        o.get(CAMPO_CAPACIDADES).and_then(Value::as_array).map(|c| c.iter().any(|x| x.as_str() == Some(cap))).unwrap_or(false)
    };
    let uuid = |o: &Value| texto(o, &[CAMPO_UUID]).filter(|u| uuid_valido(u));
    orgs.iter()
        .find(|o| CAPACIDADES_PLAN.iter().any(|c| tiene(o, c)) && uuid(o).is_some())
        .or_else(|| orgs.iter().find(|o| tiene(o, CAPACIDAD_CHAT) && uuid(o).is_some()))
        .or_else(|| orgs.iter().find(|o| uuid(o).is_some()))
        .and_then(uuid)
}

fn ventana(v: &Value, nombre: &str) -> Option<VentanaUso> {
    let w = campo(v, &[nombre])?;
    Some(VentanaUso { porcentaje: numero(w, &[CAMPO_USO])?, reinicio: fecha(w, &[CAMPO_REINICIO]) })
}

/// `None` si no trae ni la ventana de 5 h ni la semanal (aunque sea en `null`): el formato cambió.
pub fn interpretar_uso(v: &Value) -> Option<DatosClaudeAi> {
    let o = v.as_object()?;
    if !o.contains_key(VENTANA_5H) && !o.contains_key(VENTANA_SEMANAL) {
        return None;
    }
    let mut extras: Vec<VentanaExtra> = o
        .keys()
        .filter_map(|k| k.strip_prefix(PREFIJO_SEMANAL_EXTRA).map(|n| (k, n)))
        .filter_map(|(k, n)| ventana(v, k).map(|w| VentanaExtra { nombre: n.to_string(), ventana: w }))
        .collect();
    extras.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    Some(DatosClaudeAi { cinco_horas: ventana(v, VENTANA_5H), semanal: ventana(v, VENTANA_SEMANAL), extras })
}

pub fn leer(t: &dyn TransporteWeb, cookie: &Secreto) -> Result<Value, Motivo> {
    let orgs = pedir(t, como_navegador(Peticion::get(URL_ORGANIZACIONES), ORIGEN, cookie))?;
    let uuid = elegir_organizacion(&orgs).ok_or(Motivo::FormatoCambio)?;
    let url = format!("{URL_ORGANIZACIONES}/{uuid}/{RUTA_USO}");
    let uso = pedir(t, como_navegador(Peticion::get(url), ORIGEN, cookie))?;
    let datos = interpretar_uso(&uso).ok_or(Motivo::FormatoCambio)?;
    serde_json::to_value(datos).map_err(|_| Motivo::Interno)
}

pub fn usos(datos: &Value) -> Vec<Uso> {
    let Ok(d) = serde_json::from_value::<DatosClaudeAi>(datos.clone()) else { return Vec::new() };
    [(Medidor::Claude5h, d.cinco_horas), (Medidor::ClaudeSemanal, d.semanal)]
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
    use serde_json::json;

    const COOKIE: &str = "sessionKey=cookie-de-prueba";

    fn orgs() -> Value {
        json!([
            { "uuid": "org-api-0000", "name": "API", "capabilities": ["api"] },
            { "uuid": "org-pro-1111", "name": "Personal", "capabilities": ["chat", "claude_pro"], "extra": { "x": 1 } }
        ])
    }

    fn uso() -> Value {
        json!({
            "five_hour": { "utilization": 37.0, "resets_at": "2026-10-28T20:00:00.123456+00:00" },
            "seven_day": { "utilization": 12, "resets_at": "2026-11-02T08:00:00+00:00" },
            "seven_day_opus": { "utilization": 5.5, "resets_at": null },
            "seven_day_oauth_apps": null,
            "campo_nuevo": "ignorado"
        })
    }

    #[test]
    fn lee_el_uso_de_5_h_y_semanal_de_la_organizacion_con_plan() {
        let t = Grabado::nuevo(vec![
            ("GET https://claude.ai/api/organizations", Ok(orgs())),
            ("GET https://claude.ai/api/organizations/org-pro-1111/usage", Ok(uso())),
        ]);
        let d: DatosClaudeAi = serde_json::from_value(leer(&t, &Secreto::de_prueba(COOKIE)).unwrap()).unwrap();
        assert_eq!(d.cinco_horas, Some(VentanaUso { porcentaje: 37.0, reinicio: Some("2026-10-28T20:00:00Z".into()) }));
        assert_eq!(d.semanal.as_ref().map(|w| w.porcentaje), Some(12.0));
        assert_eq!(d.extras, vec![VentanaExtra { nombre: "opus".into(), ventana: VentanaUso { porcentaje: 5.5, reinicio: None } }]);
        let pedidas = t.pedidas.lock();
        let cookie = pedidas[0].encabezados.iter().find(|(n, _)| *n == "cookie").map(|(_, v)| v.as_str());
        assert_eq!(cookie, Some(COOKIE));
        assert!(pedidas[0].encabezados.iter().any(|(n, v)| *n == "user-agent" && v.contains("Edg/")));
    }

    #[test]
    fn campos_faltantes_no_rompen_y_sin_ventanas_es_formato_cambio() {
        let sin_reinicio = json!({ "five_hour": { "utilization": 50 }, "seven_day": null });
        let d = interpretar_uso(&sin_reinicio).unwrap();
        assert_eq!(d.cinco_horas, Some(VentanaUso { porcentaje: 50.0, reinicio: None }));
        assert_eq!(d.semanal, None);
        assert!(usos(&serde_json::to_value(&d).unwrap()).is_empty(), "sin hora de reinicio no hay ciclo para avisar");
        assert_eq!(interpretar_uso(&json!({ "otra": 1 })), None);
        assert_eq!(interpretar_uso(&json!([1, 2])), None);
        let t = Grabado::nuevo(vec![
            ("GET https://claude.ai/api/organizations", Ok(orgs())),
            ("GET https://claude.ai/api/organizations/org-pro-1111/usage", Ok(json!({ "limits": [] }))),
        ]);
        assert_eq!(leer(&t, &Secreto::de_prueba(COOKIE)), Err(Motivo::FormatoCambio));
    }

    #[test]
    fn organizaciones_sin_uuid_valido_son_formato_cambio() {
        assert_eq!(elegir_organizacion(&json!([{ "uuid": "../../x" }, { "name": "sin uuid" }])), None);
        assert_eq!(elegir_organizacion(&json!({ "uuid": "no-es-lista" })), None);
        assert_eq!(elegir_organizacion(&json!([{ "uuid": "solo-una" }])).as_deref(), Some("solo-una"));
        let t = Grabado::nuevo(vec![("GET https://claude.ai/api/organizations", Ok(json!([])))]);
        assert_eq!(leer(&t, &Secreto::de_prueba(COOKIE)), Err(Motivo::FormatoCambio));
    }

    #[test]
    fn sesion_vencida_y_cloudflare_son_motivos_distintos() {
        for (error, motivo) in [(ErrorRed::SesionExpirada, Motivo::SesionExpirada), (ErrorRed::Cloudflare, Motivo::Cloudflare)] {
            let t = Grabado::nuevo(vec![("GET https://claude.ai/api/organizations", Err(error))]);
            assert_eq!(leer(&t, &Secreto::de_prueba(COOKIE)), Err(motivo));
        }
        assert_ne!(Motivo::Cloudflare.texto(), Motivo::SesionExpirada.texto());
    }

    #[test]
    fn traduce_al_80_por_ciento_de_5_h_y_semanal_por_separado() {
        let d = DatosClaudeAi {
            cinco_horas: Some(VentanaUso { porcentaje: 81.0, reinicio: Some("2026-10-28T20:00:00Z".into()) }),
            semanal: Some(VentanaUso { porcentaje: 30.0, reinicio: Some("2026-11-02T08:00:00Z".into()) }),
            extras: Vec::new(),
        };
        let u = usos(&serde_json::to_value(d).unwrap());
        assert_eq!(u.len(), 2);
        assert_eq!((u[0].medidor, u[0].ciclo.as_str(), u[0].porcentaje), (Medidor::Claude5h, "2026-10-28T20:00", 81.0));
        assert_eq!((u[1].medidor, u[1].ciclo.as_str()), (Medidor::ClaudeSemanal, "2026-11-02T08:00"));
        assert!(usos(&Value::Null).is_empty());
    }
}

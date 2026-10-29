//! Cursor (no oficial, GD-CUR-02..04): todas las direcciones y nombres de campos de cursor.com viven aquí.
//! Solo lectura: los POST de `POSTS_LECTURA` son consultas del panel (no cambian nada en la cuenta) y `conexion`
//! rechaza cualquier otro POST. Los montos del panel llegan en centavos salvo el límite duro, que llega en dólares.

use super::{campo, corta, fecha, json_de, numero, pedir, texto};
use crate::avisos::{self, Medidor, Uso};
use crate::config::Config;
use crate::conexion::{Peticion, TransporteWeb};
use crate::recolectores::servicios::Motivo;
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const URL_LOGIN: &str = "https://cursor.com/dashboard";
pub const ORIGEN: &str = "https://cursor.com";
/// Sondeo del login (GD-CUR-01): solo dice si `/api/auth/me` reconoce a alguien.
pub const GUION_SESION: &str = "fetch('https://cursor.com/api/auth/me', { credentials: 'include', redirect: 'manual', cache: 'no-store' }) \
     .then(async (r) => { if (!r.ok) return false; const j = await r.json().catch(() => null); \
     return !!j && typeof j === 'object' && ['sub', 'id', 'userId', 'email'].some((k) => !!j[k]); }).catch(() => false)";

const URL_YO: &str = "https://cursor.com/api/auth/me";
const URL_RESUMEN: &str = "https://cursor.com/api/usage-summary";
const URL_USO: &str = "https://cursor.com/api/usage";
const URL_PERIODO: &str = "https://cursor.com/api/dashboard/get-current-period-usage";
const URL_LIMITE: &str = "https://cursor.com/api/dashboard/get-hard-limit";
const URL_FACTURA: &str = "https://cursor.com/api/dashboard/get-monthly-invoice";

/// Únicos POST permitidos en todo Glassdeck (GD-CUR-03): consultas del ciclo, del límite y de la factura del mes.
pub const POSTS_LECTURA: &[&str] = &[URL_PERIODO, URL_LIMITE, URL_FACTURA];

const CAMPOS_ID: &[&str] = &["sub", "id", "userId"];
const CONSULTA_USUARIO: &str = "user";
const LARGO_MAX_ID: usize = 64;

const CICLO_INICIO: &str = "billingCycleStart";
const CICLO_FIN: &str = "billingCycleEnd";
const MEMBRESIA: &str = "membershipType";
const INCLUIDO_USADO: &[&str] = &["individualUsage", "plan", "used"];
const INCLUIDO_LIMITE: &[&str] = &["individualUsage", "plan", "limit"];
const DEMANDA_USADO: &[&str] = &["individualUsage", "onDemand", "used"];
const DEMANDA_LIMITE: &[&str] = &["individualUsage", "onDemand", "limit"];

const PERIODO_INCLUIDO: &[&str] = &["planUsage", "totalSpend"];
const PERIODO_INCLUIDO_LIMITE: &[&str] = &["planUsage", "limit"];
const PERIODO_DEMANDA: &[&str] = &["spendLimitUsage", "individualUsed"];
const PERIODO_DEMANDA_TOTAL: &[&str] = &["spendLimitUsage", "totalSpend"];
const PERIODO_DEMANDA_LIMITE: &[&str] = &["spendLimitUsage", "individualLimit"];

const LIMITE_DURO_USD: &str = "hardLimit";
const FACTURA_RENGLONES: &str = "items";
const FACTURA_CENTAVOS: &str = "cents";

const USO_MODELO: &str = "gpt-4";
const USO_SOLICITUDES: &str = "numRequests";
const USO_SOLICITUDES_MAX: &str = "maxRequestUsage";
const USO_INICIO_MES: &str = "startOfMonth";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DatosCursor {
    /// Identifica el ciclo para los avisos: inicio del ciclo de facturación o, sin él, el mes.
    pub ciclo: Option<String>,
    pub ciclo_inicio: Option<String>,
    /// Fecha de corte.
    pub ciclo_fin: Option<String>,
    pub plan: Option<String>,
    pub gasto_usd: Option<f64>,
    pub limite_usd: Option<f64>,
    pub incluido_usado_usd: Option<f64>,
    pub incluido_limite_usd: Option<f64>,
    pub bajo_demanda_usd: Option<f64>,
    pub bajo_demanda_limite_usd: Option<f64>,
    pub solicitudes: Option<f64>,
    pub solicitudes_limite: Option<f64>,
}

fn dolares(v: &Value, ruta: &[&str]) -> Option<f64> {
    numero(v, ruta).map(|c| c / 100.0)
}

fn suma(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
    }
}

fn aplicar_ciclo(d: &mut DatosCursor, v: &Value) {
    d.ciclo_inicio = d.ciclo_inicio.take().or_else(|| fecha(v, &[CICLO_INICIO]));
    d.ciclo_fin = d.ciclo_fin.take().or_else(|| fecha(v, &[CICLO_FIN]));
}

fn aplicar_resumen(d: &mut DatosCursor, v: &Value) {
    aplicar_ciclo(d, v);
    d.plan = texto(v, &[MEMBRESIA]);
    d.incluido_usado_usd = dolares(v, INCLUIDO_USADO);
    d.incluido_limite_usd = dolares(v, INCLUIDO_LIMITE);
    d.bajo_demanda_usd = dolares(v, DEMANDA_USADO);
    d.bajo_demanda_limite_usd = dolares(v, DEMANDA_LIMITE);
}

fn aplicar_periodo(d: &mut DatosCursor, v: &Value) {
    aplicar_ciclo(d, v);
    d.incluido_usado_usd = dolares(v, PERIODO_INCLUIDO);
    d.incluido_limite_usd = dolares(v, PERIODO_INCLUIDO_LIMITE);
    d.bajo_demanda_usd = dolares(v, PERIODO_DEMANDA).or_else(|| dolares(v, PERIODO_DEMANDA_TOTAL));
    d.bajo_demanda_limite_usd = dolares(v, PERIODO_DEMANDA_LIMITE);
}

fn totalizar(d: &mut DatosCursor) {
    d.gasto_usd = suma(d.incluido_usado_usd, d.bajo_demanda_usd);
    d.limite_usd = suma(d.incluido_limite_usd, d.bajo_demanda_limite_usd).filter(|l| *l > 0.0);
}

fn total_factura(v: &Value) -> Option<f64> {
    let renglones = campo(v, &[FACTURA_RENGLONES])?.as_array()?;
    Some(renglones.iter().filter_map(|r| numero(r, &[FACTURA_CENTAVOS])).sum::<f64>() / 100.0)
}

fn aplicar_solicitudes(d: &mut DatosCursor, v: &Value) {
    d.solicitudes = numero(v, &[USO_MODELO, USO_SOLICITUDES]);
    d.solicitudes_limite = numero(v, &[USO_MODELO, USO_SOLICITUDES_MAX]).filter(|m| *m > 0.0);
    d.ciclo_inicio = d.ciclo_inicio.take().or_else(|| fecha(v, &[USO_INICIO_MES]));
}

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= LARGO_MAX_ID && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Sesión vencida y Cloudflare cortan; otro error deja sin ese dato y se recuerda el primero.
fn intentar(r: Result<Value, Motivo>, primer_error: &mut Option<Motivo>) -> Result<Option<Value>, Motivo> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(m) if corta(m) => Err(m),
        Err(m) => {
            primer_error.get_or_insert(m);
            Ok(None)
        }
    }
}

fn post(url: &str, cuerpo: Value) -> Peticion {
    json_de(Peticion::post_lectura(url, cuerpo))
}

/// Corre dentro del WebView oculto con el perfil de Cursor: la sesión la pone el navegador, no Glassdeck.
pub fn leer(t: &dyn TransporteWeb, ahora: DateTime<Utc>) -> Result<Value, Motivo> {
    let get = |url: &str| json_de(Peticion::get(url));
    let mut error = None;
    let mut d = DatosCursor::default();

    if let Some(v) = intentar(pedir(t, get(URL_RESUMEN)), &mut error)? {
        aplicar_resumen(&mut d, &v);
        totalizar(&mut d);
    }
    if d.gasto_usd.is_none() {
        if let Some(v) = intentar(pedir(t, post(URL_PERIODO, json!({}))), &mut error)? {
            aplicar_periodo(&mut d, &v);
            totalizar(&mut d);
        }
    }
    if d.gasto_usd.is_some() && d.bajo_demanda_limite_usd.is_none() {
        if let Some(v) = intentar(pedir(t, post(URL_LIMITE, json!({}))), &mut error)? {
            d.bajo_demanda_limite_usd = numero(&v, &[LIMITE_DURO_USD]).filter(|l| *l > 0.0);
            totalizar(&mut d);
        }
    }
    if d.gasto_usd.is_none() {
        let cuerpo = json!({ "month": ahora.month(), "year": ahora.year(), "includeUsageEvents": false });
        if let Some(v) = intentar(pedir(t, post(URL_FACTURA, cuerpo)), &mut error)? {
            d.gasto_usd = total_factura(&v);
        }
    }
    if d.gasto_usd.is_none() {
        let yo = intentar(pedir(t, get(URL_YO)), &mut error)?;
        let id = yo.and_then(|v| CAMPOS_ID.iter().find_map(|k| texto(&v, &[k]))).filter(|id| id_valido(id));
        if let Some(id) = id {
            if let Some(v) = intentar(pedir(t, get(URL_USO).consulta(CONSULTA_USUARIO, id)), &mut error)? {
                aplicar_solicitudes(&mut d, &v);
            }
        }
    }
    if d.gasto_usd.is_none() && d.solicitudes.is_none() {
        return Err(error.unwrap_or(Motivo::FormatoCambio));
    }
    d.ciclo = d.ciclo_inicio.clone().or_else(|| Some(format!("{:04}-{:02}", ahora.year(), ahora.month())));
    serde_json::to_value(d).map_err(|_| Motivo::Interno)
}

/// GD-ALERT-01: gasto contra el presupuesto o el límite; en planes por solicitudes, solicitudes contra su máximo.
pub fn usos(datos: &Value, cfg: &Config) -> Vec<Uso> {
    let Ok(d) = serde_json::from_value::<DatosCursor>(datos.clone()) else { return Vec::new() };
    let porcentaje = match d.gasto_usd {
        Some(g) => avisos::porcentaje_cursor(g, d.limite_usd, cfg.umbrales.presupuesto_cursor_usd),
        None => d.solicitudes.zip(d.solicitudes_limite).map(|(s, m)| s * 100.0 / m),
    };
    let ciclo = d.ciclo.map(|c| c.chars().take(16).collect::<String>());
    match (porcentaje, ciclo) {
        (Some(porcentaje), Some(ciclo)) => vec![Uso { medidor: Medidor::Cursor, ciclo, porcentaje }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexion::{validar_peticion, ErrorRed, Metodo};
    use crate::recolectores::web::prueba::Grabado;
    use chrono::TimeZone;

    fn ahora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap()
    }

    fn leer_con(t: &Grabado) -> Result<DatosCursor, Motivo> {
        leer(t, ahora()).map(|v| serde_json::from_value(v).unwrap())
    }

    fn resumen() -> Value {
        json!({
            "billingCycleStart": "2026-10-05T00:00:00.000Z",
            "billingCycleEnd": "2026-11-05T00:00:00.000Z",
            "membershipType": "pro",
            "individualUsage": {
                "plan": { "enabled": true, "used": 1500, "limit": 2000, "remaining": 500 },
                "onDemand": { "enabled": true, "used": 250, "limit": null }
            },
            "teamUsage": {}
        })
    }

    #[test]
    fn el_resumen_da_gasto_del_ciclo_limite_y_fecha_de_corte_y_pide_el_limite_duro() {
        let t = Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage-summary", Ok(resumen())),
            ("POST https://cursor.com/api/dashboard/get-hard-limit", Ok(json!({ "hardLimit": 30, "noUsageBasedAllowed": false }))),
        ]);
        let d = leer_con(&t).unwrap();
        assert_eq!(d.ciclo.as_deref(), Some("2026-10-05T00:00:00Z"));
        assert_eq!(d.ciclo_fin.as_deref(), Some("2026-11-05T00:00:00Z"));
        assert_eq!(d.plan.as_deref(), Some("pro"));
        assert_eq!((d.gasto_usd, d.incluido_usado_usd, d.bajo_demanda_usd), (Some(17.5), Some(15.0), Some(2.5)));
        assert_eq!(d.limite_usd, Some(50.0), "incluido 20 + límite duro 30");
        assert_eq!(t.urls().len(), 2);
        let pedidas = t.pedidas.lock();
        assert_eq!(pedidas[1].metodo, Metodo::PostLectura);
        assert!(pedidas.iter().all(|p| p.encabezados.iter().all(|(k, _)| *k == "accept")), "la sesión la pone el WebView");
    }

    #[test]
    fn sin_resumen_usa_el_periodo_y_luego_la_factura() {
        let t = Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage-summary", Err(ErrorRed::Http(404))),
            (
                "POST https://cursor.com/api/dashboard/get-current-period-usage",
                Ok(json!({ "billingCycleStart": "1759622400000", "planUsage": { "totalSpend": 900, "limit": 2000 }, "spendLimitUsage": { "individualLimit": 1000 } })),
            ),
        ]);
        let d = leer_con(&t).unwrap();
        assert_eq!((d.gasto_usd, d.limite_usd), (Some(9.0), Some(30.0)));
        assert_eq!(d.ciclo.as_deref(), Some("2025-10-05T00:00:00Z"));

        let t = Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage-summary", Ok(json!({ "otro": 1 }))),
            ("POST https://cursor.com/api/dashboard/get-current-period-usage", Err(ErrorRed::ServicioNoDisponible)),
            ("POST https://cursor.com/api/dashboard/get-monthly-invoice", Ok(json!({ "items": [{ "cents": 1234 }, { "cents": "66" }, { "descripcion": "sin monto" }] }))),
        ]);
        let d = leer_con(&t).unwrap();
        assert_eq!(d.gasto_usd, Some(13.0));
        assert_eq!(d.ciclo.as_deref(), Some("2026-10"), "sin inicio de ciclo se usa el mes");
        let factura = t.pedidas.lock().iter().find(|p| p.url.ends_with("get-monthly-invoice")).and_then(|p| p.cuerpo.clone());
        assert_eq!(factura, Some(json!({ "month": 10, "year": 2026, "includeUsageEvents": false })));
    }

    #[test]
    fn en_planes_por_solicitudes_lee_el_uso_con_el_id_de_usuario() {
        let mut respuestas = vec![
            ("GET https://cursor.com/api/usage-summary", Err(ErrorRed::Http(404))),
            ("POST https://cursor.com/api/dashboard/get-current-period-usage", Err(ErrorRed::Http(404))),
            ("POST https://cursor.com/api/dashboard/get-monthly-invoice", Err(ErrorRed::Http(404))),
            ("GET https://cursor.com/api/usage", Ok(json!({ "gpt-4": { "numRequests": 120, "maxRequestUsage": 500 }, "startOfMonth": "2026-10-03T10:00:00.000Z" }))),
        ];
        respuestas.push(("GET https://cursor.com/api/auth/me", Ok(json!({ "email": "persona@ejemplo.com", "sub": "user_desdeyo" }))));
        let t = Grabado::nuevo(respuestas);
        let d = leer_con(&t).unwrap();
        assert_eq!((d.solicitudes, d.solicitudes_limite), (Some(120.0), Some(500.0)));
        assert_eq!(d.ciclo.as_deref(), Some("2026-10-03T10:00:00Z"));
        assert!(t.urls().last().unwrap().ends_with("/api/usage?user=user_desdeyo"));
        let u = usos(&serde_json::to_value(&d).unwrap(), &Config::default());
        assert_eq!(u, vec![Uso { medidor: Medidor::Cursor, ciclo: "2026-10-03T10:00".into(), porcentaje: 24.0 }]);

        let t = Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage", Ok(json!({ "gpt-4": { "numRequests": 1 } }))),
            ("GET https://cursor.com/api/auth/me", Ok(json!({ "sub": "../otro" }))),
        ]);
        assert_eq!(leer_con(&t), Err(Motivo::ServicioNoDisponible), "id inválido: no se pide el uso");
        assert!(!t.urls().iter().any(|u| u.contains("/api/usage?")));
    }

    #[test]
    fn sin_ningun_dato_reconocible_es_formato_cambio_o_el_primer_error() {
        let t = Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage-summary", Ok(json!({ "nuevo": true }))),
            ("POST https://cursor.com/api/dashboard/get-current-period-usage", Ok(json!({}))),
            ("POST https://cursor.com/api/dashboard/get-monthly-invoice", Ok(json!({ "facturas": [] }))),
            ("GET https://cursor.com/api/auth/me", Ok(json!({}))),
            ("GET https://cursor.com/api/usage", Ok(json!({ "claude": {} }))),
        ]);
        assert_eq!(leer_con(&t), Err(Motivo::FormatoCambio));
        let t = Grabado::nuevo(vec![]);
        assert_eq!(leer_con(&t), Err(Motivo::ServicioNoDisponible));
    }

    #[test]
    fn sesion_vencida_y_cloudflare_cortan_la_ronda() {
        for (error, motivo) in [(ErrorRed::SesionExpirada, Motivo::SesionExpirada), (ErrorRed::Cloudflare, Motivo::Cloudflare)] {
            let t = Grabado::nuevo(vec![("GET https://cursor.com/api/usage-summary", Err(error))]);
            assert_eq!(leer_con(&t), Err(motivo));
            assert_eq!(t.urls().len(), 1);
        }
    }

    #[test]
    fn solo_se_permiten_los_post_de_lectura_y_sin_consulta() {
        for url in POSTS_LECTURA {
            assert!(validar_peticion(&Peticion::post_lectura(*url, json!({}))).is_ok(), "{url}");
        }
        for url in [
            "https://cursor.com/api/dashboard/set-hard-limit",
            "https://cursor.com/api/dashboard/get-hard-limit/",
            "https://cursor.com/api/usage-summary",
            "https://claude.ai/api/dashboard/get-hard-limit",
        ] {
            assert_eq!(validar_peticion(&Peticion::post_lectura(url, json!({}))).err(), Some(ErrorRed::DominioNoPermitido), "{url}");
        }
        let con_consulta = Peticion::post_lectura(URL_LIMITE, json!({})).consulta("x", "1");
        assert_eq!(validar_peticion(&con_consulta).err(), Some(ErrorRed::DominioNoPermitido));
        let t = Grabado::nuevo(vec![]);
        assert_eq!(pedir(&t, Peticion::post_lectura("https://cursor.com/api/dashboard/set-hard-limit", json!({}))), Err(Motivo::DominioNoPermitido));
    }

    #[test]
    fn el_presupuesto_manda_en_los_avisos() {
        let t = Grabado::nuevo(vec![
            ("GET https://cursor.com/api/usage-summary", Ok(resumen())),
            ("POST https://cursor.com/api/dashboard/get-hard-limit", Ok(json!({}))),
        ]);
        let v = leer(&t, ahora()).unwrap();
        let mut cfg = Config::default();
        assert_eq!(usos(&v, &cfg)[0].porcentaje, 87.5, "17.5 de 20 incluidos");
        cfg.umbrales.presupuesto_cursor_usd = Some(35.0);
        assert_eq!(usos(&v, &cfg)[0].porcentaje, 50.0);
        assert_eq!(usos(&v, &cfg)[0].ciclo, "2026-10-05T00:00");
    }
}

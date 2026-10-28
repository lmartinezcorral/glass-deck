//! Consola de Anthropic (API oficial con clave de administrador, GD-CLA-05): costo del mes en curso.
//! `amount` llega como texto decimal en centavos.

use super::{campo, iso, numero, pedir, texto, CostoMes};
use crate::conexion::{Peticion, TransporteWeb};
use crate::recolectores::servicios::Motivo;
use crate::seguridad::boveda::Secreto;
use chrono::{DateTime, Utc};
use serde_json::Value;

const URL_COSTOS: &str = "https://api.anthropic.com/v1/organizations/cost_report";
const VERSION_API: &str = "2023-06-01";
const DIAS_POR_PAGINA: &str = "31";
const ANCHO_CUBETA: &str = "1d";
const PAGINAS_MAX: usize = 12;

const CAMPO_CUBETAS: &str = "data";
const CAMPO_RESULTADOS: &str = "results";
const CAMPO_MONTO: &str = "amount";
const CAMPO_HAY_MAS: &str = "has_more";
const CAMPO_SIGUIENTE: &str = "next_page";

/// Centavos de una página; `None` si no trae la lista de cubetas.
fn centavos(pagina: &Value) -> Option<f64> {
    let cubetas = campo(pagina, &[CAMPO_CUBETAS])?.as_array()?;
    Some(
        cubetas
            .iter()
            .filter_map(|c| campo(c, &[CAMPO_RESULTADOS])?.as_array())
            .flatten()
            .filter_map(|r| numero(r, &[CAMPO_MONTO]))
            .sum(),
    )
}

pub fn leer(t: &dyn TransporteWeb, clave: &Secreto, inicio: DateTime<Utc>, mes: &str) -> Result<Value, Motivo> {
    let mut total = 0.0;
    let mut pagina: Option<String> = None;
    for _ in 0..PAGINAS_MAX {
        let mut p = Peticion::get(URL_COSTOS)
            .consulta("starting_at", iso(inicio))
            .consulta("bucket_width", ANCHO_CUBETA)
            .consulta("limit", DIAS_POR_PAGINA)
            .encabezado("x-api-key", clave.exponer())
            .encabezado("anthropic-version", VERSION_API);
        if let Some(sig) = pagina.take() {
            p = p.consulta("page", sig);
        }
        let v = pedir(t, p)?;
        total += centavos(&v).ok_or(Motivo::FormatoCambio)?;
        if v.get(CAMPO_HAY_MAS).and_then(Value::as_bool) != Some(true) {
            break;
        }
        match texto(&v, &[CAMPO_SIGUIENTE]) {
            Some(sig) => pagina = Some(sig),
            None => break,
        }
    }
    serde_json::to_value(CostoMes { mes: mes.to_string(), costo_usd: total / 100.0 }).map_err(|_| Motivo::Interno)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexion::ErrorRed;
    use crate::recolectores::web::prueba::Grabado;
    use chrono::TimeZone;
    use serde_json::json;

    const CLAVE: &str = "clave-de-prueba";
    const URL: &str = "GET https://api.anthropic.com/v1/organizations/cost_report";

    fn inicio() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 6, 0, 0).unwrap()
    }

    #[test]
    fn suma_todas_las_cubetas_y_paginas_del_mes() {
        let t = Grabado::nuevo(vec![
            (URL, Ok(json!({ "data": [{ "results": [{ "amount": "1050.5", "currency": "USD" }, { "amount": 49.5 }] }], "has_more": true, "next_page": "pagina-2" }))),
            (URL, Ok(json!({ "data": [{ "results": [{ "amount": "900" }, { "sin_monto": 1 }] }, { "results": null }], "has_more": false, "next_page": null }))),
        ]);
        let v = leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10").unwrap();
        assert_eq!(serde_json::from_value::<CostoMes>(v.clone()).unwrap(), CostoMes { mes: "2026-10".into(), costo_usd: 20.0 });
        assert!(!v.to_string().contains(CLAVE));
        let urls = t.urls();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("starting_at=2026-10-01T06%3A00%3A00Z") && urls[0].contains("bucket_width=1d") && !urls[0].contains("page="));
        assert!(urls[1].contains("page=pagina-2"));
        let pedidas = t.pedidas.lock();
        let de = |n: &str| pedidas[0].encabezados.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone());
        assert_eq!(de("x-api-key").as_deref(), Some(CLAVE));
        assert_eq!(de("anthropic-version").as_deref(), Some("2023-06-01"));
    }

    #[test]
    fn sin_cubetas_es_formato_cambio_y_401_es_sesion_expirada() {
        let t = Grabado::nuevo(vec![(URL, Ok(json!({ "error": "x" })))]);
        assert_eq!(leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10"), Err(Motivo::FormatoCambio));
        let t = Grabado::nuevo(vec![(URL, Err(ErrorRed::SesionExpirada))]);
        assert_eq!(leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10"), Err(Motivo::SesionExpirada));
        let t = Grabado::nuevo(vec![(URL, Ok(json!({ "data": [], "has_more": true })))]);
        assert_eq!(serde_json::from_value::<CostoMes>(leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10").unwrap()).unwrap().costo_usd, 0.0);
        assert_eq!(t.urls().len(), 1, "sin página siguiente no se sigue pidiendo");
    }
}

//! API de OpenAI (oficial con clave de administrador, GD-OAI-05): costo del mes en curso.
//! `amount.value` llega en dólares; se suman todas las cubetas de todas las páginas.

use super::{campo, numero, pedir, texto, CostoMes};
use crate::conexion::{Peticion, TransporteWeb};
use crate::recolectores::servicios::Motivo;
use crate::seguridad::boveda::Secreto;
use chrono::{DateTime, Utc};
use serde_json::Value;

const URL_COSTOS: &str = "https://api.openai.com/v1/organization/costs";
const DIAS_POR_PAGINA: &str = "31";
const PAGINAS_MAX: usize = 12;

const CAMPO_CUBETAS: &str = "data";
const CAMPO_RESULTADOS: &str = "results";
const CAMPO_MONTO: &[&str] = &["amount", "value"];
const CAMPO_HAY_MAS: &str = "has_more";
const CAMPO_SIGUIENTE: &str = "next_page";

/// Dólares de una página; `None` si no trae la lista de cubetas.
fn dolares(pagina: &Value) -> Option<f64> {
    let cubetas = campo(pagina, &[CAMPO_CUBETAS])?.as_array()?;
    Some(
        cubetas
            .iter()
            .filter_map(|c| campo(c, &[CAMPO_RESULTADOS])?.as_array())
            .flatten()
            .filter_map(|r| numero(r, CAMPO_MONTO))
            .sum(),
    )
}

pub fn leer(t: &dyn TransporteWeb, clave: &Secreto, inicio: DateTime<Utc>, mes: &str) -> Result<Value, Motivo> {
    let mut total = 0.0;
    let mut pagina: Option<String> = None;
    for _ in 0..PAGINAS_MAX {
        let mut p = Peticion::get(URL_COSTOS)
            .consulta("start_time", inicio.timestamp().to_string())
            .consulta("limit", DIAS_POR_PAGINA)
            .encabezado("authorization", format!("Bearer {}", clave.exponer()));
        if let Some(sig) = pagina.take() {
            p = p.consulta("page", sig);
        }
        let v = pedir(t, p)?;
        total += dolares(&v).ok_or(Motivo::FormatoCambio)?;
        if v.get(CAMPO_HAY_MAS).and_then(Value::as_bool) != Some(true) {
            break;
        }
        match texto(&v, &[CAMPO_SIGUIENTE]) {
            Some(sig) => pagina = Some(sig),
            None => break,
        }
    }
    serde_json::to_value(CostoMes { mes: mes.to_string(), costo_usd: total }).map_err(|_| Motivo::Interno)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexion::ErrorRed;
    use crate::recolectores::web::prueba::Grabado;
    use chrono::TimeZone;
    use serde_json::json;

    const CLAVE: &str = "clave-de-prueba";
    const URL: &str = "GET https://api.openai.com/v1/organization/costs";

    fn inicio() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 6, 0, 0).unwrap()
    }

    #[test]
    fn suma_el_valor_de_todas_las_cubetas_y_paginas() {
        let t = Grabado::nuevo(vec![
            (
                URL,
                Ok(json!({ "object": "page", "data": [
                    { "object": "bucket", "results": [{ "amount": { "value": 1.25, "currency": "usd" } }, { "amount": { "value": "0.75" } }] },
                    { "object": "bucket", "results": [] }
                ], "has_more": true, "next_page": "pagina-2" })),
            ),
            (URL, Ok(json!({ "data": [{ "results": [{ "amount": { "value": 3 } }, { "amount": null }] }], "has_more": false }))),
        ]);
        let v = leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10").unwrap();
        assert_eq!(serde_json::from_value::<CostoMes>(v.clone()).unwrap(), CostoMes { mes: "2026-10".into(), costo_usd: 5.0 });
        assert!(!v.to_string().contains(CLAVE));
        let urls = t.urls();
        assert!(urls[0].contains(&format!("start_time={}", inicio().timestamp())) && urls[0].contains("limit=31"));
        assert!(urls[1].contains("page=pagina-2"));
        let bearer = t.pedidas.lock()[0].encabezados.iter().find(|(k, _)| *k == "authorization").map(|(_, v)| v.clone());
        assert_eq!(bearer, Some(format!("Bearer {CLAVE}")));
    }

    #[test]
    fn sin_cubetas_es_formato_cambio_y_401_es_sesion_expirada() {
        let t = Grabado::nuevo(vec![(URL, Ok(json!({ "object": "page" })))]);
        assert_eq!(leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10"), Err(Motivo::FormatoCambio));
        let t = Grabado::nuevo(vec![(URL, Err(ErrorRed::SesionExpirada))]);
        assert_eq!(leer(&t, &Secreto::de_prueba(CLAVE), inicio(), "2026-10"), Err(Motivo::SesionExpirada));
    }
}

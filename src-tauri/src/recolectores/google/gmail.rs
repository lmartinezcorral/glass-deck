use super::ApiGoogle;
use crate::conexion::ErrorRed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const URL_MENSAJES: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages";
const URL_INBOX: &str = "https://gmail.googleapis.com/gmail/v1/users/me/labels/INBOX";
pub const MAX_MENSAJES: usize = 25;
const LARGO_REMITENTE: usize = 120;
const LARGO_ASUNTO: usize = 300;

#[derive(Deserialize)]
struct RespLista {
    #[serde(default)]
    messages: Vec<RespRef>,
    #[serde(rename = "resultSizeEstimate", default)]
    estimado: Option<u32>,
}

#[derive(Deserialize)]
struct RespRef {
    id: String,
}

#[derive(Deserialize)]
struct RespEtiqueta {
    #[serde(rename = "messagesUnread", default)]
    no_leidos: Option<u32>,
}

/// Solo lo que pide GD-MAIL-03: sin `snippet`, `body` ni `parts`, que serde descarta.
#[derive(Deserialize)]
struct RespMensaje {
    id: String,
    #[serde(rename = "threadId", default)]
    hilo: Option<String>,
    #[serde(rename = "internalDate", default)]
    fecha: Option<String>,
    #[serde(default)]
    payload: Option<RespCarga>,
}

#[derive(Deserialize)]
struct RespCarga {
    #[serde(default)]
    headers: Vec<RespEncabezado>,
}

#[derive(Deserialize)]
struct RespEncabezado {
    name: String,
    value: String,
}

/// Encabezados de un no leído; vive solo en memoria (GD-MAIL-05) y no tiene campo para el cuerpo.
#[derive(Clone, PartialEq, Serialize)]
pub struct CorreoNoLeido {
    pub id: String,
    pub hilo: String,
    pub remitente: String,
    pub asunto: String,
    pub fecha_ms: i64,
}

pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn consulta_lista() -> Vec<(&'static str, String)> {
    vec![("labelIds", "INBOX".into()), ("labelIds", "UNREAD".into()), ("maxResults", MAX_MENSAJES.to_string())]
}

/// Nunca `format=full` ni `raw`: `gmail.metadata` tampoco los permitiría.
pub fn consulta_metadatos() -> Vec<(&'static str, String)> {
    vec![
        ("format", "metadata".into()),
        ("metadataHeaders", "From".into()),
        ("metadataHeaders", "Subject".into()),
        ("metadataHeaders", "Date".into()),
    ]
}

fn recortar(s: &str, largo: usize) -> String {
    let limpio: String = s.chars().filter(|c| !c.is_control()).collect();
    let limpio = limpio.trim();
    if limpio.chars().count() <= largo {
        limpio.to_string()
    } else {
        let mut r: String = limpio.chars().take(largo - 1).collect();
        r.push('…');
        r
    }
}

/// `"Nombre" <a@b>` → `Nombre`; sin nombre queda la dirección.
pub fn remitente(from: &str) -> String {
    let from = from.trim();
    let nombre = match from.rfind('<') {
        Some(i) if from.ends_with('>') => {
            let n = from[..i].trim().trim_matches('"').trim();
            if n.is_empty() {
                from[i + 1..from.len() - 1].trim()
            } else {
                n
            }
        }
        _ => from,
    };
    recortar(nombre, LARGO_REMITENTE)
}

fn convertir(v: serde_json::Value) -> Result<CorreoNoLeido, ErrorRed> {
    let m: RespMensaje = serde_json::from_value(v).map_err(|_| ErrorRed::Formato)?;
    if !id_valido(&m.id) {
        return Err(ErrorRed::Formato);
    }
    let encabezado = |nombre: &str| {
        m.payload
            .as_ref()
            .and_then(|p| p.headers.iter().find(|h| h.name.eq_ignore_ascii_case(nombre)))
            .map(|h| h.value.as_str())
    };
    let fecha_ms = m
        .fecha
        .as_deref()
        .and_then(|f| f.parse::<i64>().ok())
        .or_else(|| encabezado("Date").and_then(|d| chrono::DateTime::parse_from_rfc2822(d).ok()).map(|d| d.timestamp_millis()))
        .unwrap_or(0);
    let asunto = recortar(encabezado("Subject").unwrap_or_default(), LARGO_ASUNTO);
    let hilo = m.hilo.filter(|h| id_valido(h)).unwrap_or_else(|| m.id.clone());
    Ok(CorreoNoLeido {
        remitente: remitente(encabezado("From").unwrap_or("(Sin remitente)")),
        asunto: if asunto.is_empty() { "(Sin asunto)".into() } else { asunto },
        hilo,
        id: m.id,
        fecha_ms,
    })
}

/// Conteo de no leídos de INBOX y los más recientes. `cache` evita volver a pedir los encabezados que ya
/// están en memoria; se recorta a los que siguen sin leer.
pub fn leer_buzon(
    api: &dyn ApiGoogle,
    token: &str,
    cache: &mut HashMap<String, CorreoNoLeido>,
) -> Result<(u32, Vec<CorreoNoLeido>), ErrorRed> {
    let lista: RespLista = serde_json::from_value(api.obtener(URL_MENSAJES, &consulta_lista(), token)?).map_err(|_| ErrorRed::Formato)?;
    let ids: Vec<String> = lista.messages.into_iter().map(|m| m.id).filter(|id| id_valido(id)).take(MAX_MENSAJES).collect();
    cache.retain(|id, _| ids.contains(id));
    for id in &ids {
        if !cache.contains_key(id) {
            let m = convertir(api.obtener(&format!("{URL_MENSAJES}/{id}"), &consulta_metadatos(), token)?)?;
            cache.insert(id.clone(), m);
        }
    }
    let no_leidos = api
        .obtener(URL_INBOX, &[], token)
        .ok()
        .and_then(|v| serde_json::from_value::<RespEtiqueta>(v).ok())
        .and_then(|e| e.no_leidos)
        .or(lista.estimado)
        .unwrap_or_default()
        .max(ids.len() as u32);
    let mut mensajes: Vec<CorreoNoLeido> = ids.iter().filter_map(|id| cache.get(id).cloned()).collect();
    mensajes.sort_by(|a, b| b.fecha_ms.cmp(&a.fecha_ms));
    Ok((no_leidos, mensajes))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mensaje_grabado_solo_deja_remitente_asunto_y_fecha() {
        let v: serde_json::Value = serde_json::from_str(include_str!("../grabaciones/google_gmail_mensaje_sintetico.json")).unwrap();
        let m = convertir(v).unwrap();
        assert_eq!(m.remitente, "Banco Ejemplo");
        assert_eq!(m.asunto, "Tu estado de cuenta de prueba");
        assert_eq!(m.fecha_ms, 1_793_210_400_000);
        let json = serde_json::to_value(&m).unwrap();
        let campos: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(campos, vec!["asunto", "fecha_ms", "hilo", "id", "remitente"]);
        let texto = json.to_string();
        assert!(!texto.contains("CUERPO") && !texto.contains("VEVYVE8") && !texto.contains("snippet"));
    }

    #[test]
    fn los_remitentes_y_asuntos_se_limpian() {
        assert_eq!(remitente("\"Equipo Demo\" <demo@example.com>"), "Equipo Demo");
        assert_eq!(remitente("<solo@example.com>"), "solo@example.com");
        assert_eq!(remitente("directo@example.com"), "directo@example.com");
        assert_eq!(recortar("a\u{0007}b", 10), "ab");
        assert_eq!(recortar(&"x".repeat(400), LARGO_ASUNTO).chars().count(), LARGO_ASUNTO);
    }

    #[test]
    fn las_consultas_piden_solo_no_leidos_de_inbox_y_metadatos() {
        assert_eq!(consulta_lista()[..2], [("labelIds", "INBOX".to_string()), ("labelIds", "UNREAD".to_string())]);
        let m = consulta_metadatos();
        assert!(m.contains(&("format", "metadata".into())));
        assert!(!m.iter().any(|(_, v)| v == "full" || v == "raw"));
        assert!(id_valido("18f0000000000001"));
        assert!(!id_valido("../labels") && !id_valido("a?b") && !id_valido(""));
    }

    #[test]
    fn la_lista_grabada_se_lee() {
        let v: serde_json::Value = serde_json::from_str(include_str!("../grabaciones/google_gmail_lista_sintetica.json")).unwrap();
        let l: RespLista = serde_json::from_value(v).unwrap();
        assert_eq!(l.messages.len(), 2);
        assert_eq!(l.estimado, Some(2));
    }
}

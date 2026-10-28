use super::navegador;
use super::ApiGoogle;
use crate::conexion::ErrorRed;
use chrono::{DateTime, Duration as Lapso, Local, NaiveDate, SecondsFormat, TimeZone};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const URL_CALENDARIOS: &str = "https://www.googleapis.com/calendar/v3/users/me/calendarList";
const URL_CALENDARIOS_BASE: &str = "https://www.googleapis.com/calendar/v3/calendars";
const MAX_CALENDARIOS: usize = 10;
const MAX_EVENTOS: usize = 50;
const LARGO_TITULO: usize = 200;
const LARGO_UBICACION: usize = 200;
/// Solo se piden estos campos: la descripción y los invitados de un evento nunca llegan.
const CAMPOS: &str = "items(id,status,summary,location,htmlLink,start,end)";

#[derive(Deserialize)]
struct RespCalendarios {
    #[serde(default)]
    items: Vec<RespCalendario>,
}

#[derive(Deserialize)]
struct RespCalendario {
    id: String,
    #[serde(default)]
    selected: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct RespEventos {
    #[serde(default)]
    items: Vec<RespEvento>,
}

#[derive(Deserialize)]
struct RespEvento {
    id: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(rename = "htmlLink", default)]
    enlace: Option<String>,
    start: Option<RespMomento>,
    end: Option<RespMomento>,
}

#[derive(Deserialize)]
struct RespMomento {
    #[serde(rename = "dateTime", default)]
    fecha_hora: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

/// Solo título, hora, ubicación y enlace (GD-CAL-01); en memoria, nunca en disco.
#[derive(Clone, PartialEq, Serialize)]
pub struct EventoAgenda {
    pub id: String,
    pub titulo: String,
    pub ubicacion: Option<String>,
    pub inicio_ms: i64,
    pub fin_ms: i64,
    pub todo_el_dia: bool,
    /// Vacío si Google devolvió un enlace fuera de `www.google.com/calendar` o `calendar.google.com`.
    pub enlace: String,
}

/// Hoy desde la medianoche local y las próximas 24 h.
pub fn ventana(ahora: DateTime<Local>) -> (String, String) {
    let medianoche = ahora
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|d| Local.from_local_datetime(&d).earliest())
        .unwrap_or(ahora);
    let fin = ahora + Lapso::hours(24);
    (medianoche.to_rfc3339_opts(SecondsFormat::Secs, false), fin.to_rfc3339_opts(SecondsFormat::Secs, false))
}

fn momento_ms(m: &RespMomento) -> Option<(i64, bool)> {
    if let Some(fh) = &m.fecha_hora {
        return DateTime::parse_from_rfc3339(fh).ok().map(|d| (d.timestamp_millis(), false));
    }
    let d = NaiveDate::parse_from_str(m.date.as_deref()?, "%Y-%m-%d").ok()?;
    let local = Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).earliest()?;
    Some((local.timestamp_millis(), true))
}

fn texto(s: Option<String>, largo: usize) -> Option<String> {
    let s: String = s?.chars().filter(|c| !c.is_control()).take(largo).collect();
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn convertir(e: RespEvento) -> Option<EventoAgenda> {
    if e.status.as_deref() == Some("cancelled") || e.id.is_empty() || e.id.len() > 1_024 {
        return None;
    }
    let (inicio_ms, todo_el_dia) = momento_ms(e.start.as_ref()?)?;
    let fin_ms = e.end.as_ref().and_then(momento_ms).map(|(f, _)| f).unwrap_or(inicio_ms).max(inicio_ms);
    let enlace = e.enlace.filter(|u| navegador::url_permitida(u)).unwrap_or_default();
    Some(EventoAgenda {
        id: e.id,
        titulo: texto(e.summary, LARGO_TITULO).unwrap_or_else(|| "(Sin título)".into()),
        ubicacion: texto(e.location, LARGO_UBICACION),
        inicio_ms,
        fin_ms,
        todo_el_dia,
        enlace,
    })
}

fn calendarios(v: serde_json::Value) -> Result<Vec<String>, ErrorRed> {
    let r: RespCalendarios = serde_json::from_value(v).map_err(|_| ErrorRed::Formato)?;
    let ids: Vec<String> = r
        .items
        .into_iter()
        .filter(|c| (c.selected || c.primary) && !c.hidden && !c.id.is_empty())
        .map(|c| c.id)
        .take(MAX_CALENDARIOS)
        .collect();
    Ok(if ids.is_empty() { vec!["primary".into()] } else { ids })
}

pub fn url_eventos(calendario: &str) -> String {
    let mut u = Url::parse(URL_CALENDARIOS_BASE).expect("URL fija");
    if let Ok(mut segmentos) = u.path_segments_mut() {
        segmentos.push(calendario).push("events");
    }
    u.into()
}

fn eventos(v: serde_json::Value) -> Result<Vec<EventoAgenda>, ErrorRed> {
    let r: RespEventos = serde_json::from_value(v).map_err(|_| ErrorRed::Formato)?;
    Ok(r.items.into_iter().filter_map(convertir).collect())
}

/// Sin el permiso opcional de la lista de calendarios (403) se lee solo el principal.
pub fn leer_agenda(api: &dyn ApiGoogle, token: &str, desde: &str, hasta: &str) -> Result<Vec<EventoAgenda>, ErrorRed> {
    let ids = match api.obtener(URL_CALENDARIOS, &[("minAccessRole", "reader".into())], token) {
        Ok(v) => calendarios(v)?,
        Err(ErrorRed::SesionExpirada) => vec!["primary".into()],
        Err(e) => return Err(e),
    };
    let consulta = [
        ("timeMin", desde.to_string()),
        ("timeMax", hasta.to_string()),
        ("singleEvents", "true".into()),
        ("orderBy", "startTime".into()),
        ("maxResults", MAX_EVENTOS.to_string()),
        ("fields", CAMPOS.into()),
    ];
    let mut vistos = HashSet::new();
    let mut todos = Vec::new();
    for id in ids {
        for e in eventos(api.obtener(&url_eventos(&id), &consulta, token)?)? {
            if vistos.insert(e.id.clone()) {
                todos.push(e);
            }
        }
    }
    todos.sort_by(|a, b| a.inicio_ms.cmp(&b.inicio_ms).then(a.titulo.cmp(&b.titulo)));
    Ok(todos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_eventos_grabados_dejan_solo_titulo_hora_ubicacion_y_enlace() {
        let v: serde_json::Value = serde_json::from_str(include_str!("../grabaciones/google_eventos_sinteticos.json")).unwrap();
        let e = eventos(v).unwrap();
        assert_eq!(e.len(), 3, "el cancelado se descarta");
        assert_eq!(e[0].titulo, "Revisión semanal (prueba)");
        assert_eq!(e[0].ubicacion.as_deref(), Some("Sala virtual"));
        assert_eq!(e[0].inicio_ms, DateTime::parse_from_rfc3339("2026-10-28T23:00:00Z").unwrap().timestamp_millis());
        assert_eq!(e[0].fin_ms - e[0].inicio_ms, 45 * 60_000);
        assert!(!e[0].todo_el_dia && e[1].todo_el_dia);
        assert!(e[0].enlace.starts_with("https://www.google.com/calendar/event?eid="));
        assert_eq!(e[2].titulo, "(Sin título)");
        assert_eq!(e[2].enlace, "", "un enlace ajeno a Google no se conserva");
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("DESCRIPCION-SINTETICA"));
    }

    #[test]
    fn la_lista_grabada_deja_los_calendarios_seleccionados_y_visibles() {
        let v: serde_json::Value = serde_json::from_str(include_str!("../grabaciones/google_calendarios_sinteticos.json")).unwrap();
        assert_eq!(calendarios(v).unwrap(), vec!["cuenta.uno@example.com", "equipo@group.calendar.google.com"]);
        assert_eq!(calendarios(serde_json::json!({ "items": [] })).unwrap(), vec!["primary"]);
    }

    #[test]
    fn el_id_del_calendario_se_codifica_en_la_ruta() {
        let u = url_eventos("es.mexican#holiday@group.v.calendar.google.com");
        assert!(u.starts_with("https://www.googleapis.com/calendar/v3/calendars/es.mexican%23holiday@"));
        assert!(u.ends_with("/events"));
        assert!(crate::conexion::validar_url(&u, &[]).is_ok());
    }

    #[test]
    fn la_ventana_va_de_la_medianoche_a_24_h() {
        let ahora = Local.with_ymd_and_hms(2026, 10, 28, 15, 10, 0).unwrap();
        let (desde, hasta) = ventana(ahora);
        let d = DateTime::parse_from_rfc3339(&desde).unwrap();
        let h = DateTime::parse_from_rfc3339(&hasta).unwrap();
        assert_eq!(d.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(), "2026-10-28 00:00");
        assert_eq!((h.timestamp() - ahora.timestamp()) / 3600, 24);
    }
}

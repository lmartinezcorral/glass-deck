use crate::config::estado::ahora_ms;
use crate::config::{AlmacenConfig, AlmacenEstado, Umbrales};
use crate::ventanas::{self, catalogo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// GD-ALERT-02: el clic en cualquier aviso de gasto abre este widget maximizado.
pub const WIDGET_DESTINO: &str = "spending";
const EXTRA_ABRIR: &str = "abrir";
const LARGO_MAX_CICLO: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Medidor {
    Cursor,
    Claude5h,
    ClaudeSemanal,
    Chatgpt5h,
    ChatgptSemanal,
}

impl Medidor {
    pub fn id(self) -> &'static str {
        match self {
            Medidor::Cursor => "cursor",
            Medidor::Claude5h => "claude_5h",
            Medidor::ClaudeSemanal => "claude_semanal",
            Medidor::Chatgpt5h => "chatgpt_5h",
            Medidor::ChatgptSemanal => "chatgpt_semanal",
        }
    }

    fn umbrales(self, u: &Umbrales) -> &[u8] {
        match self {
            Medidor::Cursor => &u.cursor,
            Medidor::Claude5h => &u.claude_5h,
            Medidor::ClaudeSemanal => &u.claude_semanal,
            Medidor::Chatgpt5h => &u.chatgpt_5h,
            Medidor::ChatgptSemanal => &u.chatgpt_semanal,
        }
    }

    fn titulo(self, umbral: u8) -> String {
        match self {
            Medidor::Cursor => format!("Cursor: {umbral} % del presupuesto del mes"),
            Medidor::Claude5h => format!("Claude: {umbral} % del límite de 5 h"),
            Medidor::ClaudeSemanal => format!("Claude: {umbral} % del límite semanal"),
            Medidor::Chatgpt5h => format!("ChatGPT: {umbral} % del límite de 5 h"),
            Medidor::ChatgptSemanal => format!("ChatGPT: {umbral} % del límite semanal"),
        }
    }
}

/// Uso de un medidor en su ciclo de facturación o ventana de límite; `ciclo` identifica ese periodo
/// (p. ej. la fecha de inicio del ciclo o la hora en que se reinicia la ventana).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Uso {
    pub medidor: Medidor,
    pub ciclo: String,
    pub porcentaje: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Aviso {
    pub medidor: Medidor,
    pub ciclo: String,
    pub umbral: u8,
    pub porcentaje: f64,
    pub titulo: String,
    pub cuerpo: String,
    #[serde(skip)]
    claves: Vec<String>,
}

pub fn clave(medidor: Medidor, ciclo: &str, umbral: u8) -> String {
    format!("{}:{ciclo}:{umbral}", medidor.id())
}

/// El presupuesto editable manda; sin él se usa el límite que reporte Cursor.
pub fn porcentaje_cursor(gastado_usd: f64, limite_usd: Option<f64>, presupuesto_usd: Option<f64>) -> Option<f64> {
    let valido = |v: &f64| v.is_finite() && *v > 0.0;
    let base = presupuesto_usd.filter(valido).or(limite_usd.filter(valido))?;
    gastado_usd.is_finite().then(|| gastado_usd.max(0.0) / base * 100.0)
}

/// Si en una sola lectura se cruzan varios umbrales, se avisa solo el más alto y los demás quedan como enviados.
fn evaluar_uno(u: &Uso, umbrales: &[u8], enviados: &BTreeMap<String, i64>) -> Option<Aviso> {
    if !u.porcentaje.is_finite() || u.ciclo.trim().is_empty() || u.ciclo.chars().count() > LARGO_MAX_CICLO {
        return None;
    }
    let pendientes: Vec<u8> = umbrales
        .iter()
        .copied()
        .filter(|t| u.porcentaje >= f64::from(*t) && !enviados.contains_key(&clave(u.medidor, &u.ciclo, *t)))
        .collect();
    let umbral = pendientes.iter().copied().max()?;
    Some(Aviso {
        medidor: u.medidor,
        ciclo: u.ciclo.clone(),
        umbral,
        porcentaje: u.porcentaje,
        titulo: u.medidor.titulo(umbral),
        cuerpo: format!("Uso actual: {:.0} %. Haz clic para ver el detalle en Spending.", u.porcentaje),
        claves: pendientes.iter().map(|t| clave(u.medidor, &u.ciclo, *t)).collect(),
    })
}

pub fn evaluar(usos: &[Uso], umbrales: &Umbrales, enviados: &BTreeMap<String, i64>) -> Vec<Aviso> {
    if !umbrales.notificaciones {
        return Vec::new();
    }
    usos.iter().filter_map(|u| evaluar_uno(u, u.medidor.umbrales(umbrales), enviados)).collect()
}

/// Evalúa contra el historial de `state.json` y lo actualiza antes de notificar, para no repetir tras un reinicio.
pub fn registrar_pendientes(estado: &AlmacenEstado, usos: &[Uso], umbrales: &Umbrales, ahora: i64) -> Vec<Aviso> {
    let avisos = evaluar(usos, umbrales, &estado.leer().avisos);
    let claves: Vec<String> = avisos.iter().flat_map(|a| a.claves.iter().cloned()).collect();
    if let Err(e) = estado.registrar_avisos(&claves, ahora) {
        log::warn!("{e}");
    }
    avisos
}

pub fn procesar(app: &AppHandle, usos: &[Uso]) {
    if usos.is_empty() {
        return;
    }
    let umbrales = app.state::<AlmacenConfig>().leer().umbrales;
    for a in registrar_pendientes(&app.state::<AlmacenEstado>(), usos, &umbrales, ahora_ms()) {
        log::info!("Aviso de umbral: {} {} %", a.medidor.id(), a.umbral);
        notificar(app, &a.titulo, &a.cuerpo);
        let _ = app.emit("gd://aviso", &a);
    }
}

pub fn notificar(app: &AppHandle, titulo: &str, cuerpo: &str) {
    let r = app.notification().builder().title(titulo).body(cuerpo).extra(EXTRA_ABRIR, WIDGET_DESTINO).show();
    if let Err(e) = r {
        log::warn!("No se pudo mostrar la notificación: {e}");
    }
}

/// En Windows el clic solo llega mientras el aviso está en pantalla; desde el Centro de actividades no se reporta.
pub fn instalar(app: &AppHandle) {
    let h = app.clone();
    let _ = app.notification().on_action(move |p| {
        if p.action_id() != "tap" {
            return;
        }
        let destino = p.notification().and_then(|n| n.extra().get(EXTRA_ABRIR)).and_then(|v| v.as_str());
        if let Some(id) = destino.and_then(catalogo::buscar).map(|d| d.id) {
            ventanas::diferir(&h, move |a| crate::comandos::abrir_maximizado(a, id));
        }
    });
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AvisoEnviado {
    pub clave: String,
    pub hora_ms: i64,
}

pub fn historial(estado: &AlmacenEstado) -> Vec<AvisoEnviado> {
    let mut v: Vec<AvisoEnviado> =
        estado.leer().avisos.into_iter().map(|(clave, hora_ms)| AvisoEnviado { clave, hora_ms }).collect();
    v.sort_by(|a, b| b.hora_ms.cmp(&a.hora_ms).then_with(|| a.clave.cmp(&b.clave)));
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::dir_temporal;

    fn cursor(ciclo: &str, porcentaje: f64) -> Uso {
        Uso { medidor: Medidor::Cursor, ciclo: ciclo.into(), porcentaje }
    }

    fn umbrales_de(avisos: &[Aviso]) -> Vec<u8> {
        avisos.iter().map(|a| a.umbral).collect()
    }

    #[test]
    fn de_49_a_51_y_52_avisa_una_vez_y_un_ciclo_nuevo_vuelve_a_avisar() {
        let d = dir_temporal("avisos-ciclo");
        let estado = AlmacenEstado::nuevo(&d);
        let u = Umbrales::default();
        let mut enviados = Vec::new();
        for (i, p) in [49.0, 51.0, 52.0].into_iter().enumerate() {
            enviados.extend(registrar_pendientes(&estado, &[cursor("2026-10-01", p)], &u, i as i64));
        }
        assert_eq!(umbrales_de(&enviados), vec![50]);
        assert_eq!(enviados[0].titulo, "Cursor: 50 % del presupuesto del mes");
        let nuevo_ciclo = registrar_pendientes(&estado, &[cursor("2026-11-01", 51.0)], &u, 9);
        assert_eq!(umbrales_de(&nuevo_ciclo), vec![50]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn si_cruza_varios_umbrales_avisa_el_mas_alto_y_no_repite_los_menores() {
        let d = dir_temporal("avisos-salto");
        let estado = AlmacenEstado::nuevo(&d);
        let u = Umbrales::default();
        let a = registrar_pendientes(&estado, &[cursor("c1", 85.0)], &u, 1);
        assert_eq!(umbrales_de(&a), vec![80]);
        assert!(registrar_pendientes(&estado, &[cursor("c1", 90.0)], &u, 2).is_empty());
        let b = registrar_pendientes(&estado, &[cursor("c1", 100.0)], &u, 3);
        assert_eq!(umbrales_de(&b), vec![100]);
        let claves: Vec<String> = historial(&estado).into_iter().map(|h| h.clave).collect();
        assert_eq!(claves, vec!["cursor:c1:100", "cursor:c1:50", "cursor:c1:80"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn claude_avisa_por_separado_la_ventana_de_5_h_y_la_semanal() {
        let usos = [
            Uso { medidor: Medidor::Claude5h, ciclo: "2026-10-28T15:00".into(), porcentaje: 81.0 },
            Uso { medidor: Medidor::ClaudeSemanal, ciclo: "2026-10-27".into(), porcentaje: 79.9 },
        ];
        let a = evaluar(&usos, &Umbrales::default(), &BTreeMap::new());
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].titulo, "Claude: 80 % del límite de 5 h");
        let usos = [Uso { porcentaje: 80.0, ..usos[1].clone() }];
        assert_eq!(evaluar(&usos, &Umbrales::default(), &BTreeMap::new())[0].titulo, "Claude: 80 % del límite semanal");
    }

    #[test]
    fn chatgpt_avisa_al_80_por_ciento_de_5_h_y_del_semanal_por_separado() {
        let usos = [
            Uso { medidor: Medidor::Chatgpt5h, ciclo: "2026-10-28T20:00".into(), porcentaje: 80.0 },
            Uso { medidor: Medidor::ChatgptSemanal, ciclo: "2026-11-02T00:00".into(), porcentaje: 92.0 },
            Uso { medidor: Medidor::Claude5h, ciclo: "2026-10-28T20:00".into(), porcentaje: 10.0 },
        ];
        let a = evaluar(&usos, &Umbrales::default(), &BTreeMap::new());
        let titulos: Vec<&str> = a.iter().map(|x| x.titulo.as_str()).collect();
        assert_eq!(titulos, vec!["ChatGPT: 80 % del límite de 5 h", "ChatGPT: 80 % del límite semanal"]);
        assert_eq!(clave(Medidor::Chatgpt5h, "c", 80), "chatgpt_5h:c:80");
        let sin = Umbrales { chatgpt_5h: vec![], ..Umbrales::default() };
        assert_eq!(evaluar(&usos, &sin, &BTreeMap::new()).len(), 1, "una lista vacía apaga ese medidor");
    }

    #[test]
    fn el_historial_sobrevive_a_un_reinicio() {
        let d = dir_temporal("avisos-reinicio");
        let u = Umbrales::default();
        assert_eq!(registrar_pendientes(&AlmacenEstado::nuevo(&d), &[cursor("c", 60.0)], &u, 1).len(), 1);
        let despues = AlmacenEstado::nuevo(&d);
        assert!(registrar_pendientes(&despues, &[cursor("c", 60.0)], &u, 2).is_empty());
        assert_eq!(historial(&despues)[0], AvisoEnviado { clave: "cursor:c:50".into(), hora_ms: 1 });
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cambiar_un_umbral_surte_efecto_en_la_siguiente_evaluacion() {
        let mut u = Umbrales { cursor: vec![90], ..Umbrales::default() };
        let usos = [cursor("c", 85.0)];
        assert!(evaluar(&usos, &u, &BTreeMap::new()).is_empty());
        u.cursor = vec![80];
        assert_eq!(umbrales_de(&evaluar(&usos, &u, &BTreeMap::new())), vec![80]);
        u.notificaciones = false;
        assert!(evaluar(&usos, &u, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn datos_invalidos_no_avisan() {
        let u = Umbrales::default();
        for malo in [cursor("c", f64::NAN), cursor(" ", 90.0), cursor(&"x".repeat(65), 90.0), cursor("c", 49.99)] {
            assert!(evaluar(&[malo], &u, &BTreeMap::new()).is_empty());
        }
    }

    #[test]
    fn el_porcentaje_de_cursor_usa_el_presupuesto_o_el_limite() {
        assert_eq!(porcentaje_cursor(30.0, Some(20.0), Some(60.0)), Some(50.0));
        assert_eq!(porcentaje_cursor(30.0, Some(20.0), None), Some(150.0));
        assert_eq!(porcentaje_cursor(30.0, Some(20.0), Some(0.0)), Some(150.0));
        assert_eq!(porcentaje_cursor(30.0, None, None), None);
        assert_eq!(porcentaje_cursor(f64::NAN, Some(20.0), None), None);
    }

    #[test]
    fn el_aviso_no_expone_las_claves_internas() {
        let a = evaluar(&[cursor("c", 80.0)], &Umbrales::default(), &BTreeMap::new());
        let v = serde_json::to_value(&a[0]).unwrap();
        assert!(v.get("claves").is_none());
        assert_eq!(v["medidor"], "cursor");
        assert_eq!(v["umbral"], 80);
    }
}

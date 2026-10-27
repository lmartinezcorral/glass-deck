use crate::config::AlmacenConfig;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Accion {
    MostrarOcultar,
    Privacidad,
}

#[derive(Debug, Clone, Serialize)]
pub struct EstadoAtajo {
    pub accion: Accion,
    pub combinacion: String,
    pub registrado: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Atajos {
    registrados: Mutex<HashMap<u32, Accion>>,
    estado: Mutex<HashMap<Accion, EstadoAtajo>>,
}

pub const MENSAJE_CONFLICTO: &str = "Este atajo lo usa otro programa";

fn combinacion_de(app: &AppHandle, accion: Accion) -> String {
    let c = app.state::<AlmacenConfig>().leer().atajos;
    match accion {
        Accion::MostrarOcultar => c.mostrar_ocultar,
        Accion::Privacidad => c.privacidad,
    }
}

fn intentar(app: &AppHandle, accion: Accion, combinacion: &str) -> Result<Shortcut, String> {
    let atajo: Shortcut = combinacion.parse().map_err(|_| format!("Combinación no válida: {combinacion}"))?;
    if app.global_shortcut().is_registered(atajo) {
        let propio = app.state::<Atajos>().registrados.lock().get(&atajo.id()).copied();
        return if propio == Some(accion) { Ok(atajo) } else { Err("Ya lo usa otro atajo de Glassdeck".into()) };
    }
    app.global_shortcut().register(atajo).map_err(|_| MENSAJE_CONFLICTO.to_string())?;
    Ok(atajo)
}

fn anotar(app: &AppHandle, accion: Accion, combinacion: &str, r: &Result<Shortcut, String>) {
    let estado = EstadoAtajo {
        accion,
        combinacion: combinacion.to_string(),
        registrado: r.is_ok(),
        error: r.as_ref().err().cloned(),
    };
    app.state::<Atajos>().estado.lock().insert(accion, estado);
    if let Ok(a) = r {
        app.state::<Atajos>().registrados.lock().insert(a.id(), accion);
    }
}

pub fn registrar_todos(app: &AppHandle) {
    for accion in [Accion::MostrarOcultar, Accion::Privacidad] {
        let comb = combinacion_de(app, accion);
        let r = intentar(app, accion, &comb);
        if let Err(e) = &r {
            log::warn!("Atajo {accion:?} sin registrar: {e}");
        }
        anotar(app, accion, &comb, &r);
    }
}

pub fn estado(app: &AppHandle) -> Vec<EstadoAtajo> {
    let mut v: Vec<EstadoAtajo> = app.state::<Atajos>().estado.lock().values().cloned().collect();
    v.sort_by_key(|e| e.accion as u8);
    v
}

pub fn cambiar(app: &AppHandle, accion: Accion, combinacion: &str) -> Result<Vec<EstadoAtajo>, String> {
    let anterior = combinacion_de(app, accion);
    if anterior == combinacion {
        return Ok(estado(app));
    }
    let nuevo = intentar(app, accion, combinacion)?;
    if let Ok(viejo) = anterior.parse::<Shortcut>() {
        let mut reg = app.state::<Atajos>().registrados.lock();
        if reg.get(&viejo.id()) == Some(&accion) {
            reg.remove(&viejo.id());
            let _ = app.global_shortcut().unregister(viejo);
        }
    }
    app.state::<AlmacenConfig>().modificar(|c| match accion {
        Accion::MostrarOcultar => c.atajos.mostrar_ocultar = combinacion.to_string(),
        Accion::Privacidad => c.atajos.privacidad = combinacion.to_string(),
    })?;
    anotar(app, accion, combinacion, &Ok(nuevo));
    Ok(estado(app))
}

pub fn al_pulsar(app: &AppHandle, atajo: &Shortcut, evento: ShortcutEvent) {
    if evento.state() != ShortcutState::Pressed {
        return;
    }
    let accion = app.state::<Atajos>().registrados.lock().get(&atajo.id()).copied();
    match accion {
        Some(Accion::MostrarOcultar) => crate::ventanas::mostrar_ocultar_todo(app),
        Some(Accion::Privacidad) => alternar_privacidad(app),
        None => {}
    }
}

pub fn alternar_privacidad(app: &AppHandle) -> bool {
    let nuevo = app
        .state::<AlmacenConfig>()
        .modificar(|c| c.privacidad = !c.privacidad)
        .map(|c| c.privacidad)
        .unwrap_or(false);
    let _ = app.emit("gd://privacidad", nuevo);
    crate::bandeja::sincronizar(app);
    nuevo
}

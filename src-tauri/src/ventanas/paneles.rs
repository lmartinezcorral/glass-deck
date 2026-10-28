use super::{aplicar_efecto, Gestor};
use crate::config::AlmacenConfig;
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const DOCK: &str = "dock";
pub const CUSTOMIZE: &str = "customize";
pub const LIMPIEZA: &str = "limpieza";

const DISTANCIA_BORDE: i32 = 32;
const ESPERA_GUARDAR_DOCK: Duration = Duration::from_millis(350);

#[derive(Default)]
pub struct EstadoDock {
    pendiente: Mutex<Option<Instant>>,
    ignorar_hasta: Mutex<Option<Instant>>,
}

struct DefPanel {
    etiqueta: &'static str,
    vista: &'static str,
    titulo: &'static str,
    ancho: f64,
    alto: f64,
    en_barra_tareas: bool,
}

const PANELES: &[DefPanel] = &[
    DefPanel { etiqueta: DOCK, vista: "dock", titulo: "Glassdeck · Dock", ancho: 860.0, alto: 64.0, en_barra_tareas: false },
    DefPanel { etiqueta: CUSTOMIZE, vista: "customize", titulo: "Glassdeck · Customize", ancho: 780.0, alto: 580.0, en_barra_tareas: true },
    DefPanel { etiqueta: LIMPIEZA, vista: "limpieza", titulo: "Glassdeck · Limpiar temporales", ancho: 540.0, alto: 480.0, en_barra_tareas: true },
];

fn crear(app: &AppHandle, def: &DefPanel, pestana: Option<&str>) -> tauri::Result<WebviewWindow> {
    let consulta = pestana.map(|p| format!("&pestana={p}")).unwrap_or_default();
    let url = WebviewUrl::App(format!("index.html?vista={}{consulta}", def.vista).into());
    let v = WebviewWindowBuilder::new(app, def.etiqueta, url)
        .title(def.titulo)
        .additional_browser_args(super::ARGUMENTOS_WEBVIEW)
        .initialization_script(super::CONGELAR_PROTOTIPO)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .skip_taskbar(!def.en_barra_tareas)
        .inner_size(def.ancho, def.alto)
        .visible(false)
        .build()?;
    aplicar_efecto(app, &v, false);
    Ok(v)
}

fn centrar_en_monitor_activo(app: &AppHandle, v: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(m), Ok(tam)) = (monitor, v.outer_size()) else { return };
    let area = m.work_area();
    let x = area.position.x + (area.size.width as i32 - tam.width as i32) / 2;
    let y = area.position.y + (area.size.height as i32 - tam.height as i32) / 2;
    let _ = v.set_position(PhysicalPosition::new(x, y));
}

pub fn abrir(app: &AppHandle, etiqueta: &str) -> Result<(), String> {
    abrir_en(app, etiqueta, None)
}

/// Con el panel ya abierto, la pestaña llega por `gd://pestana`; al crearlo, por la URL (aún no hay oyentes).
pub fn abrir_en(app: &AppHandle, etiqueta: &str, pestana: Option<&str>) -> Result<(), String> {
    if etiqueta == DOCK {
        return mostrar_dock(app, true);
    }
    let def = PANELES.iter().find(|p| p.etiqueta == etiqueta).ok_or("Panel desconocido")?;
    let v = match app.get_webview_window(etiqueta) {
        Some(v) => {
            if let Some(p) = pestana {
                let _ = app.emit_to(etiqueta, "gd://pestana", p);
            }
            v
        }
        None => {
            let v = crear(app, def, pestana).map_err(|e| e.to_string())?;
            centrar_en_monitor_activo(app, &v);
            v
        }
    };
    v.show().map_err(|e| e.to_string())?;
    let _ = v.unminimize();
    let _ = v.set_focus();
    Ok(())
}

pub fn cerrar(app: &AppHandle, etiqueta: &str) {
    if etiqueta == DOCK {
        let _ = mostrar_dock(app, false);
        return;
    }
    if let Some(v) = app.get_webview_window(etiqueta) {
        let _ = v.destroy();
    }
}

fn posicion_dock(app: &AppHandle, v: &WebviewWindow) -> PhysicalPosition<i32> {
    let cfg = app.state::<AlmacenConfig>().leer().dock;
    if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
        if app.monitor_from_point(x as f64 + 20.0, y as f64 + 20.0).ok().flatten().is_some() {
            return PhysicalPosition::new(x, y);
        }
    }
    let m = app.primary_monitor().ok().flatten();
    let tam = v.outer_size().unwrap_or_default();
    match m {
        Some(m) => {
            let a = m.work_area();
            let margen = (16.0 * m.scale_factor()) as i32;
            PhysicalPosition::new(
                a.position.x + (a.size.width as i32 - tam.width as i32) / 2,
                a.position.y + a.size.height as i32 - tam.height as i32 - margen,
            )
        }
        None => PhysicalPosition::new(100, 100),
    }
}

pub fn mostrar_dock(app: &AppHandle, visible: bool) -> Result<(), String> {
    app.state::<AlmacenConfig>().modificar(|c| c.dock.visible = visible)?;
    if !visible {
        if let Some(v) = app.get_webview_window(DOCK) {
            let _ = v.hide();
        }
        return Ok(());
    }
    let v = match app.get_webview_window(DOCK) {
        Some(v) => v,
        None => {
            let def = &PANELES[0];
            let v = crear(app, def, None).map_err(|e| e.to_string())?;
            #[cfg(windows)]
            super::memoria_baja(app, DOCK);
            let p = posicion_dock(app, &v);
            *app.state::<EstadoDock>().ignorar_hasta.lock() = Some(Instant::now() + Duration::from_millis(400));
            let _ = v.set_position(p);
            let app2 = app.clone();
            v.on_window_event(move |e| {
                if let WindowEvent::Moved(_) = e {
                    let estado = app2.state::<EstadoDock>();
                    let propio = estado.ignorar_hasta.lock().map(|t| Instant::now() < t).unwrap_or(false);
                    if !propio {
                        *estado.pendiente.lock() = Some(Instant::now());
                    }
                }
            });
            v
        }
    };
    super::mostrar_ventana(&v).map_err(|e| e.to_string())?;
    aplicar_capa_dock(app);
    let _ = app.emit("gd://config", ());
    Ok(())
}

pub fn aplicar_capa_dock(app: &AppHandle) {
    let Some(v) = app.get_webview_window(DOCK) else { return };
    #[cfg(windows)]
    if let Some(hwnd) = super::win32::hwnd_de(&v) {
        let escritorio = app.state::<Gestor>().escritorio_visible.load(Ordering::Relaxed);
        let encima = app.state::<AlmacenConfig>().leer().capa_defecto == crate::config::Capa::SiempreEncima;
        if escritorio || encima {
            super::win32::siempre_encima(hwnd, true);
        } else {
            super::win32::enviar_al_fondo(hwnd);
        }
    }
    #[cfg(not(windows))]
    let _ = v;
}

pub fn revisar_dock(app: &AppHandle) {
    let estado = app.state::<EstadoDock>();
    let listo = {
        let mut p = estado.pendiente.lock();
        match *p {
            Some(t) if t.elapsed() >= ESPERA_GUARDAR_DOCK => {
                *p = None;
                true
            }
            _ => false,
        }
    };
    if !listo {
        return;
    }
    let Some(v) = app.get_webview_window(DOCK) else { return };
    let Ok(mut pos) = v.outer_position() else { return };
    if let (Ok(tam), Some(m)) = (v.outer_size(), app.monitor_from_point(pos.x as f64 + 20.0, pos.y as f64 + 20.0).ok().flatten()) {
        let a = m.work_area();
        let d = (DISTANCIA_BORDE as f64 * m.scale_factor()) as i32;
        let (izq, arr) = (a.position.x, a.position.y);
        let der = a.position.x + a.size.width as i32 - tam.width as i32;
        let aba = a.position.y + a.size.height as i32 - tam.height as i32;
        let mut nuevo = pos;
        if (pos.x - izq).abs() <= d {
            nuevo.x = izq;
        } else if (pos.x - der).abs() <= d {
            nuevo.x = der;
        }
        if (pos.y - arr).abs() <= d {
            nuevo.y = arr;
        } else if (pos.y - aba).abs() <= d {
            nuevo.y = aba;
        }
        if nuevo != pos {
            *estado.ignorar_hasta.lock() = Some(Instant::now() + Duration::from_millis(400));
            let _ = v.set_position(nuevo);
            pos = nuevo;
        }
    }
    let _ = app.state::<AlmacenConfig>().modificar(|c| {
        c.dock.x = Some(pos.x);
        c.dock.y = Some(pos.y);
    });
    aplicar_capa_dock(app);
}

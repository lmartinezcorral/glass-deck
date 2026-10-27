use crate::config::AlmacenConfig;
use crate::recolectores::EstadoRecoleccion;
use crate::ventanas::{self, paneles};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

#[derive(Default)]
pub struct Bandeja {
    privacidad: Mutex<Option<CheckMenuItem<Wry>>>,
    pausa: Mutex<Option<CheckMenuItem<Wry>>>,
}

pub fn crear(app: &AppHandle) -> tauri::Result<()> {
    let cfg = app.state::<AlmacenConfig>().leer();
    let mostrar = MenuItem::with_id(app, "mostrar", "Mostrar u ocultar todo", true, None::<&str>)?;
    let dock = MenuItem::with_id(app, "dock", "Mostrar el dock", true, None::<&str>)?;
    let customize = MenuItem::with_id(app, "customize", "Customize…", true, None::<&str>)?;
    let zonas = MenuItem::with_id(app, "zonas", "Zonas (fase 4)", false, None::<&str>)?;
    let limpiar = MenuItem::with_id(app, "limpiar", "Limpiar temporales…", true, None::<&str>)?;
    let procesos = MenuItem::with_id(app, "procesos", "Cerrar procesos pesados…", true, None::<&str>)?;
    let privacidad = CheckMenuItem::with_id(app, "privacidad", "Modo privacidad", true, cfg.privacidad, None::<&str>)?;
    let pausa = CheckMenuItem::with_id(app, "pausar", "Pausar", true, false, None::<&str>)?;
    let salir = MenuItem::with_id(app, "salir", "Salir", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&mostrar, &dock, &customize, &zonas, &sep1, &limpiar, &procesos, &sep2, &privacidad, &pausa, &salir],
    )?;
    let estado = app.state::<Bandeja>();
    *estado.privacidad.lock() = Some(privacidad);
    *estado.pausa.lock() = Some(pausa);

    let mut b = TrayIconBuilder::with_id("glassdeck")
        .tooltip("Glassdeck")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            "mostrar" => ventanas::mostrar_ocultar_todo(app),
            "dock" => {
                let _ = paneles::mostrar_dock(app, true);
            }
            "customize" => {
                let _ = paneles::abrir(app, paneles::CUSTOMIZE);
            }
            "limpiar" => {
                let _ = paneles::abrir(app, paneles::LIMPIEZA);
            }
            "procesos" => crate::comandos::abrir_procesos_maximizado(app),
            "privacidad" => {
                crate::atajos::alternar_privacidad(app);
            }
            "pausar" => {
                let e = app.state::<EstadoRecoleccion>();
                let nuevo = !e.pausado.load(Ordering::Relaxed);
                e.pausado.store(nuevo, Ordering::Relaxed);
                sincronizar(app);
            }
            "salir" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, evento| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = evento {
                let _ = paneles::mostrar_dock(tray.app_handle(), true);
            }
        });
    if let Some(icono) = app.default_window_icon() {
        b = b.icon(icono.clone());
    }
    b.build(app)?;
    Ok(())
}

pub fn sincronizar(app: &AppHandle) {
    let estado = app.state::<Bandeja>();
    if let Some(i) = estado.privacidad.lock().as_ref() {
        let _ = i.set_checked(app.state::<AlmacenConfig>().leer().privacidad);
    }
    if let Some(i) = estado.pausa.lock().as_ref() {
        let _ = i.set_checked(app.state::<EstadoRecoleccion>().pausado.load(Ordering::Relaxed));
    }
}

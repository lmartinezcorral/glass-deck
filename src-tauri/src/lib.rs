mod acciones;
mod atajos;
mod bandeja;
mod comandos;
mod config;
mod poc;
mod recolectores;
mod seguridad;
mod ventanas;

use config::AlmacenConfig;
use std::path::PathBuf;
use tauri::{Manager, RunEvent};
use ventanas::paneles::{self, EstadoDock};

fn carpeta_config(app: &tauri::App) -> PathBuf {
    app.path().config_dir().map(|d| d.join("Glassdeck")).unwrap_or_else(|_| PathBuf::from("."))
}

fn carpeta_logs(app: &tauri::App) -> PathBuf {
    app.path()
        .local_data_dir()
        .map(|d| d.join("Glassdeck").join("logs"))
        .unwrap_or_else(|_| PathBuf::from("logs"))
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            ventanas::diferir(app, |a| {
                let _ = paneles::mostrar_dock(a, true);
            });
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--arranque"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, atajo, evento| atajos::al_pulsar(app, atajo, evento))
                .build(),
        )
        .setup(|app| {
            seguridad::registro::iniciar(&carpeta_logs(app));
            log::info!("Glassdeck {} iniciando", env!("CARGO_PKG_VERSION"));
            app.manage(AlmacenConfig::nuevo(&carpeta_config(app)));
            app.manage(ventanas::Gestor::nuevo());
            app.manage(recolectores::EstadoRecoleccion::default());
            app.manage(atajos::Atajos::default());
            app.manage(bandeja::Bandeja::default());
            app.manage(EstadoDock::default());
            let h = app.handle();
            if let Ok(variante) = std::env::var("GLASSDECK_POC") {
                poc::ejecutar(h, variante.trim());
                ventanas::marcar_arranque_listo();
                return Ok(());
            }
            if let Err(e) = bandeja::crear(h) {
                log::error!("No se pudo crear el icono de la bandeja: {e}");
            }
            atajos::registrar_todos(h);
            let cfg = app.state::<AlmacenConfig>().leer();
            comandos::aplicar_arranque(h, cfg.arranque_windows);
            ventanas::abrir_activos(h);
            if cfg.dock.visible {
                let _ = paneles::mostrar_dock(h, true);
            }
            ventanas::iniciar_vigilancia(h);
            recolectores::iniciar(h);
            ventanas::marcar_arranque_listo();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comandos::config_leer,
            comandos::config_guardar,
            comandos::widget_activar,
            comandos::widget_estado,
            comandos::widget_maximizar,
            comandos::widget_fijar,
            comandos::widget_cerrar,
            comandos::historial,
            comandos::procesos_cerrar,
            comandos::proceso_es_fijo,
            comandos::limpieza_vista_previa,
            comandos::limpieza_ejecutar,
            comandos::panel_abrir,
            comandos::panel_cerrar,
            comandos::atajos_estado,
            comandos::atajo_cambiar,
            comandos::privacidad_alternar,
            comandos::mostrar_ocultar_todo,
            comandos::restablecer_posiciones,
            comandos::reiniciar_explorer,
            comandos::abrir_administrador_tareas,
            comandos::estado_entorno,
        ])
        .build(tauri::generate_context!())
        .expect("no se pudo iniciar Glassdeck");

    app.run(|_app, evento| {
        if let RunEvent::ExitRequested { api, code: None, .. } = evento {
            api.prevent_exit();
        }
    });
}

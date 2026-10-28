mod acciones;
mod arranque;
mod atajos;
mod avisos;
mod bandeja;
mod comandos;
mod conexion;
mod config;
mod poc;
mod recolectores;
mod seguridad;
mod ventanas;

use config::{AlmacenConfig, AlmacenEstado};
use recolectores::google::{self, Google};
use recolectores::servicios::{self, EstadoServicios};
use std::path::PathBuf;
use tauri::{Emitter, Manager, RunEvent};
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

pub fn borrar_credenciales() {
    #[cfg(windows)]
    seguridad::boveda::borrar_todas();
}

pub fn run() {
    arranque::esperar_escritorio_si_hace_falta();
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
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            seguridad::registro::iniciar(&carpeta_logs(app));
            log::info!("Glassdeck {} iniciando", env!("CARGO_PKG_VERSION"));
            app.manage(AlmacenConfig::nuevo(&carpeta_config(app)));
            app.manage(AlmacenEstado::nuevo(&carpeta_config(app)));
            app.manage(EstadoServicios::default());
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
            avisos::instalar(h);
            let cfg = app.state::<AlmacenConfig>().leer();
            arranque::aplicar(h, cfg.arranque_windows);
            ventanas::abrir_activos(h);
            if cfg.dock.visible {
                let _ = paneles::mostrar_dock(h, true);
            }
            ventanas::iniciar_vigilancia(h);
            recolectores::iniciar(h);
            let cuentas_google = std::sync::Arc::new(Google::nuevo(std::sync::Arc::new(google::InternetGoogle)));
            let avisador = h.clone();
            cuentas_google.al_cambiar(move || {
                let _ = avisador.emit("gd://google", ());
            });
            app.manage(cuentas_google.clone());
            let inicio = app.path().home_dir().unwrap_or_else(|_| PathBuf::from("."));
            servicios::iniciar(
                h,
                vec![
                    Box::new(recolectores::claude_code::ClaudeCode::nuevo(&inicio)),
                    Box::new(recolectores::codex::Codex::nuevo(&inicio)),
                    Box::new(recolectores::clima::FuenteClima::nueva(std::sync::Arc::new(conexion::Internet))),
                    Box::new(google::FuenteCorreo(cuentas_google.clone())),
                    Box::new(google::FuenteAgenda(cuentas_google.clone())),
                    Box::new(google::FuenteAlerta::nueva(cuentas_google)),
                ],
            );
            ventanas::marcar_arranque_listo();
            arranque::registrar_arranque_listo();
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
            comandos::procesos_fijos,
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
            comandos::boveda_estado,
            comandos::boveda_guardar,
            comandos::boveda_borrar,
            comandos::servicio_ultimo,
            comandos::catalogo_widgets,
            comandos::clima_buscar_ciudad,
            comandos::clima_fijar_ubicacion,
            comandos::avisos_probar,
            comandos::avisos_historial,
            comandos::google_cliente_importar,
            google::comandos::google_cuentas,
            google::comandos::google_conectar,
            google::comandos::google_cancelar,
            google::comandos::google_desconectar,
            google::comandos::google_abrir,
        ])
        .build(tauri::generate_context!())
        .expect("no se pudo iniciar Glassdeck");

    app.run(|_app, evento| {
        if let RunEvent::ExitRequested { api, code: None, .. } = evento {
            api.prevent_exit();
        }
    });
}

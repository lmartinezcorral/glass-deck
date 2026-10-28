use super::{navegador, Google, VistaCuenta};
use crate::recolectores::servicios::EstadoServicios;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

type R<T> = Result<T, String>;

fn releer(app: &AppHandle) {
    let s = app.state::<EstadoServicios>();
    for fuente in ["correo", "calendario", "alerta"] {
        s.forzar(fuente);
    }
}

#[tauri::command]
pub fn google_cuentas(google: State<'_, Arc<Google>>) -> Vec<VistaCuenta> {
    google.cuentas()
}

/// Espera hasta 5 min a que Luis autorice en el navegador; `google_cancelar` la corta antes.
#[tauri::command]
pub async fn google_conectar(app: AppHandle) -> R<VistaCuenta> {
    let google = app.state::<Arc<Google>>().inner().clone();
    let cuenta = tauri::async_runtime::spawn_blocking(move || google.conectar()).await.map_err(|e| e.to_string())??;
    releer(&app);
    Ok(cuenta)
}

#[tauri::command]
pub fn google_cancelar(google: State<'_, Arc<Google>>) {
    google.cancelar_conexion();
}

#[tauri::command]
pub async fn google_desconectar(app: AppHandle, id: String) -> R<()> {
    let google = app.state::<Arc<Google>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || google.desconectar(&id)).await.map_err(|e| e.to_string())??;
    releer(&app);
    Ok(())
}

/// GD-ROT-03: solo Gmail y Calendar, siempre en el navegador del sistema.
#[tauri::command]
pub async fn google_abrir(url: String) -> R<()> {
    tauri::async_runtime::spawn_blocking(move || navegador::abrir(&url)).await.map_err(|e| e.to_string())?
}

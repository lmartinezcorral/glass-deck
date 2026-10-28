use crate::acciones::limpieza::{self, Opciones, Origen, Resumen};
use crate::acciones::procesos::{Politica, Rechazo, ResultadoCierre};
use crate::atajos::{self, Accion, EstadoAtajo};
use crate::avisos;
use crate::conexion::Internet;
use crate::config::{AlmacenConfig, AlmacenEstado, Config, EstadoWidget, ModoUbicacion, Ubicacion};
use crate::recolectores::clima;
use crate::recolectores::web::{self, EstadoCuentas, ServicioWeb};
use crate::recolectores::servicios::{EstadoFuente, EstadoServicios};
use crate::recolectores::{EstadoRecoleccion, SerieHistorial};
use crate::seguridad::boveda::{self, Clave};
use crate::seguridad::google_cliente;
use crate::ventanas::{self, catalogo, paneles};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

type R<T> = Result<T, String>;

fn id_de(ventana: &WebviewWindow) -> R<String> {
    catalogo::id_de_etiqueta(ventana.label()).map(str::to_string).ok_or_else(|| "Solo un widget puede hacer esto".into())
}

#[tauri::command]
pub fn config_leer(app: AppHandle) -> Config {
    app.state::<AlmacenConfig>().leer()
}

#[tauri::command]
pub fn config_guardar(app: AppHandle, config: Config) -> R<Config> {
    let anterior = app.state::<AlmacenConfig>().leer();
    let mut nueva = config;
    nueva.widgets = anterior.widgets.clone();
    nueva.dock = anterior.dock.clone();
    nueva.atajos = anterior.atajos.clone();
    let guardada = app.state::<AlmacenConfig>().reemplazar(nueva)?;
    if guardada.apariencia.tema != anterior.apariencia.tema {
        ventanas::aplicar_tema_a_todos(&app);
    }
    if guardada.arranque_windows != anterior.arranque_windows {
        crate::arranque::aplicar(&app, guardada.arranque_windows);
    }
    if guardada.capa_defecto != anterior.capa_defecto {
        for w in catalogo::WIDGETS {
            ventanas::aplicar_capa(&app, w.id);
        }
        paneles::aplicar_capa_dock(&app);
    }
    if guardada.precios_modelos != anterior.precios_modelos {
        app.state::<EstadoServicios>().forzar("claude_code");
        app.state::<EstadoServicios>().forzar("codex");
    }
    if guardada.clima != anterior.clima {
        app.state::<EstadoServicios>().forzar("clima");
    }
    let _ = app.emit("gd://config", ());
    Ok(guardada)
}

#[tauri::command]
pub async fn widget_activar(app: AppHandle, id: String, activo: bool) -> R<()> {
    catalogo::buscar(&id).ok_or("Widget desconocido")?;
    ventanas::activar_widget(&app, &id, activo)?;
    if activo {
        app.state::<EstadoServicios>().forzar_widget(&id);
    }
    Ok(())
}

#[tauri::command]
pub fn widget_estado(app: AppHandle, ventana: WebviewWindow, estado: EstadoWidget) -> R<()> {
    ventanas::cambiar_estado(&app, &id_de(&ventana)?, estado)
}

#[tauri::command]
pub fn widget_maximizar(app: AppHandle, ventana: WebviewWindow, activo: bool) -> R<()> {
    ventanas::maximizar(&app, &id_de(&ventana)?, activo)
}

#[tauri::command]
pub fn widget_fijar(app: AppHandle, ventana: WebviewWindow, fijado: bool) -> R<()> {
    ventanas::fijar(&app, &id_de(&ventana)?, fijado)
}

#[tauri::command]
pub fn widget_cerrar(app: AppHandle, ventana: WebviewWindow) -> R<()> {
    ventanas::cerrar_widget(&app, &id_de(&ventana)?)
}

#[tauri::command]
pub fn historial(app: AppHandle, fuente: String) -> Vec<SerieHistorial> {
    let intervalos = app.state::<AlmacenConfig>().leer().intervalos;
    app.state::<EstadoRecoleccion>().historiales.lock().series(&fuente, &intervalos)
}

#[derive(Serialize)]
pub struct ResultadoProceso {
    pub pid: u32,
    pub nombre: String,
    pub resultado: ResultadoCierre,
    pub motivo: Option<Rechazo>,
}

#[tauri::command]
pub async fn procesos_cerrar(app: AppHandle, pids: Vec<u32>, confirmacion: Option<String>) -> R<Vec<ResultadoProceso>> {
    let lista_negra = app.state::<AlmacenConfig>().leer().lista_negra_usuario;
    tauri::async_runtime::spawn_blocking(move || cerrar_procesos(&lista_negra, &pids, confirmacion.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(windows)]
fn cerrar_procesos(lista_negra: &[String], pids: &[u32], confirmacion: Option<&str>) -> R<Vec<ResultadoProceso>> {
    use crate::acciones::procesos::sistema;
    let politica = Politica::nueva(lista_negra, std::process::id());
    let vivos: Vec<_> = sistema::enumerar()
        .map_err(|e| format!("No se pudo leer la lista de procesos ({})", e.code()))?
        .iter()
        .map(|p| p.candidato())
        .collect();
    let v = politica.validar_pids(pids, &vivos, confirmacion);
    let mut resultados: Vec<ResultadoProceso> = v
        .rechazados
        .into_iter()
        .map(|r| ResultadoProceso {
            pid: r.candidato.pid,
            nombre: r.candidato.nombre,
            resultado: r.motivo.into(),
            motivo: Some(r.motivo),
        })
        .collect();
    let hilos: Vec<_> = v
        .permitidos
        .into_iter()
        .map(|c| {
            std::thread::spawn(move || ResultadoProceso {
                pid: c.pid,
                resultado: sistema::cerrar(c.pid, sistema::ESPERA_CIERRE_AMABLE),
                nombre: c.nombre,
                motivo: None,
            })
        })
        .collect();
    for h in hilos {
        if let Ok(r) = h.join() {
            resultados.push(r);
        }
    }
    for r in &resultados {
        log::info!("Cierre de proceso pid={} resultado={:?}", r.pid, r.resultado);
    }
    Ok(resultados)
}

#[cfg(not(windows))]
fn cerrar_procesos(_: &[String], _: &[u32], _: Option<&str>) -> R<Vec<ResultadoProceso>> {
    Err("Solo disponible en Windows".into())
}

#[tauri::command]
pub fn proceso_es_fijo(nombre: String) -> bool {
    Politica::es_fijo_protegido(&nombre)
}

#[tauri::command]
pub fn procesos_fijos() -> Vec<&'static str> {
    crate::acciones::procesos::politica::PROTEGIDOS_FIJOS.to_vec()
}

#[tauri::command]
pub async fn limpieza_vista_previa() -> R<Vec<Origen>> {
    tauri::async_runtime::spawn_blocking(limpieza::vista_previa).await.map_err(|e| e.to_string())
}

fn ruta_helper(app: &AppHandle) -> Option<PathBuf> {
    let mut candidatos = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidatos.push(dir.join("glassdeck-helper.exe"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidatos.push(dir.join("glassdeck-helper.exe"));
        }
    }
    if cfg!(debug_assertions) {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("helper").join("target");
        candidatos.push(base.join("release").join("glassdeck-helper.exe"));
        candidatos.push(base.join("debug").join("glassdeck-helper.exe"));
    }
    candidatos.into_iter().find(|p| p.is_file())
}

#[tauri::command]
pub async fn limpieza_ejecutar(app: AppHandle, opciones: Opciones) -> R<Resumen> {
    let helper = ruta_helper(&app);
    let r = tauri::async_runtime::spawn_blocking(move || limpieza::ejecutar(opciones, helper.as_deref()))
        .await
        .map_err(|e| e.to_string())?;
    log::info!("Limpieza: liberado={} borrados={} omitidos={}", r.liberado, r.archivos_borrados, r.omitidos);
    Ok(r)
}

pub fn abrir_maximizado(app: &AppHandle, id: &str) {
    if ventanas::activar_widget(app, id, true).is_ok() {
        app.state::<EstadoServicios>().forzar_widget(id);
        let _ = ventanas::maximizar(app, id, true);
    }
}

#[tauri::command]
pub async fn panel_abrir(app: AppHandle, panel: String) -> R<()> {
    match panel.as_str() {
        "procesos" => {
            abrir_maximizado(&app, "procesos");
            Ok(())
        }
        "cuentas" | "clima" => paneles::abrir_en(&app, paneles::CUSTOMIZE, Some(panel.as_str())),
        p => paneles::abrir(&app, p),
    }
}

#[tauri::command]
pub fn panel_cerrar(app: AppHandle, ventana: WebviewWindow) {
    paneles::cerrar(&app, ventana.label());
}

#[tauri::command]
pub fn atajos_estado(app: AppHandle) -> Vec<EstadoAtajo> {
    atajos::estado(&app)
}

#[tauri::command]
pub fn atajo_cambiar(app: AppHandle, accion: Accion, combinacion: String) -> R<Vec<EstadoAtajo>> {
    atajos::cambiar(&app, accion, combinacion.trim())
}

#[tauri::command]
pub fn privacidad_alternar(app: AppHandle) -> bool {
    atajos::alternar_privacidad(&app)
}

#[tauri::command]
pub fn mostrar_ocultar_todo(app: AppHandle) {
    ventanas::mostrar_ocultar_todo(&app);
}

#[tauri::command]
pub fn restablecer_posiciones(app: AppHandle) -> R<()> {
    ventanas::restablecer_posiciones(&app)
}

#[tauri::command]
pub async fn reiniciar_explorer() -> R<()> {
    tauri::async_runtime::spawn_blocking(crate::acciones::sistema::reiniciar_explorer).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn abrir_administrador_tareas() -> R<()> {
    crate::acciones::sistema::abrir_administrador_tareas()
}

#[tauri::command]
pub fn estado_entorno(app: AppHandle) -> serde_json::Value {
    use std::sync::atomic::Ordering;
    let e = app.state::<EstadoRecoleccion>();
    serde_json::json!({
        "con_bateria": e.con_bateria.load(Ordering::Relaxed),
        "pausado": e.pausado.load(Ordering::Relaxed),
        "version": env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
pub struct EstadoCredencial {
    pub clave: Clave,
    pub presente: bool,
}

/// GD-NFR-SEC-01: a la interfaz solo llega si la credencial existe, nunca su valor.
#[tauri::command]
pub async fn boveda_estado() -> Vec<EstadoCredencial> {
    tauri::async_runtime::spawn_blocking(|| {
        Clave::TODAS.iter().map(|c| EstadoCredencial { clave: *c, presente: boveda::existe(c.nombre()) }).collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn boveda_guardar(app: AppHandle, clave: Clave, valor: String) -> R<()> {
    if !clave.se_captura_a_mano() {
        return Err("Esta credencial se obtiene con su propio inicio de sesión".into());
    }
    boveda::guardar(clave.nombre(), valor.trim())?;
    web::credencial_cambio(&app, clave);
    Ok(())
}

#[tauri::command]
pub fn boveda_borrar(app: AppHandle, clave: Clave) -> R<()> {
    boveda::borrar_largo(clave.nombre())?;
    web::credencial_cambio(&app, clave);
    Ok(())
}

/// GD-MAIL-07: el núcleo abre el diálogo, lee y valida el archivo; a la interfaz no llegan ni la ruta ni el contenido.
/// `false` si se cancela el diálogo.
#[tauri::command]
pub async fn google_cliente_importar(ventana: WebviewWindow) -> R<bool> {
    #[cfg(windows)]
    {
        let dueno = ventanas::win32::hwnd_de(&ventana).map(|h| h.0 as isize).unwrap_or(0);
        tauri::async_runtime::spawn_blocking(move || {
            let Some(ruta) = ventanas::win32::elegir_json(dueno, "Importar cliente de Google")? else {
                return Ok(false);
            };
            google_cliente::importar_archivo(&ruta)?;
            Ok(true)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = ventana;
        Err("Solo disponible en Windows".into())
    }
}

#[tauri::command]
pub fn catalogo_widgets() -> &'static [catalogo::DefWidget] {
    catalogo::WIDGETS
}

#[tauri::command]
pub fn servicio_ultimo(app: AppHandle, fuente: String) -> Option<EstadoFuente> {
    app.state::<EstadoServicios>().ultimo(&fuente)
}

#[tauri::command]
pub async fn clima_buscar_ciudad(texto: String) -> R<Vec<Ubicacion>> {
    if !clima::LARGO_BUSQUEDA.contains(&texto.trim().chars().count()) {
        return Err("Escribe entre 2 y 80 caracteres".into());
    }
    tauri::async_runtime::spawn_blocking(move || clima::buscar_ciudad(&Internet, &texto))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|m| m.texto().to_string())
}

/// `None` vuelve a la ubicación automática por IP.
#[tauri::command]
pub fn clima_fijar_ubicacion(app: AppHandle, ubicacion: Option<Ubicacion>) -> R<Config> {
    if let Some(u) = &ubicacion {
        u.validar()?;
    }
    let guardada = app.state::<AlmacenConfig>().modificar(|c| {
        c.clima.modo = if ubicacion.is_some() { ModoUbicacion::Manual } else { ModoUbicacion::Automatica };
        if ubicacion.is_some() {
            c.clima.ubicacion = ubicacion;
        }
    })?;
    app.state::<EstadoServicios>().forzar("clima");
    let _ = app.emit("gd://config", ());
    Ok(guardada)
}

/// Aviso de muestra para comprobar que Windows lo muestra y que el clic abre Spending; no entra al historial.
#[tauri::command]
pub fn avisos_probar(app: AppHandle) {
    avisos::notificar(&app, "Glassdeck: aviso de prueba", "Así se verán los avisos de gasto. Haz clic para abrir Spending.");
}

#[tauri::command]
pub fn avisos_historial(app: AppHandle) -> Vec<avisos::AvisoEnviado> {
    avisos::historial(&app.state::<AlmacenEstado>())
}

/// Abre la ventana de login aislada del servicio; la cookie de sesión va directo a la bóveda (GD-NFR-SEC-04).
/// Asíncrono: en Windows crear un WebView2 desde un comando síncrono bloquea la app.
#[tauri::command]
pub async fn cuenta_conectar(app: AppHandle, servicio: ServicioWeb) -> R<()> {
    ventanas::login::conectar(&app, servicio)
}

/// Borra la sesión guardada y el perfil de login de ese servicio.
#[tauri::command]
pub async fn cuenta_desconectar(app: AppHandle, servicio: ServicioWeb) -> R<()> {
    ventanas::login::desconectar(&app, servicio)
}

/// Solo estados y booleanos: lo usan el dock (GD-DOCK-02), Customize y Spending, que no tiene `boveda_estado`.
#[tauri::command]
pub async fn cuentas_estado(app: AppHandle) -> R<EstadoCuentas> {
    tauri::async_runtime::spawn_blocking(move || web::estado_cuentas(&app)).await.map_err(|e| e.to_string())
}

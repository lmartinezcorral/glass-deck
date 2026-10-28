pub mod catalogo;
mod geometria;
pub mod paneles;
#[cfg(windows)]
pub mod win32;

use crate::config::{AlmacenConfig, Capa, EstadoWidget, Tema};
use catalogo::{DefWidget, ALTO_PILDORA, ANCHO_PILDORA, RADIO_ESQUINA};
use geometria::Rect;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::window::{Color, Effect, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

const ESPERA_FIN_ARRASTRE: Duration = Duration::from_millis(250);
const IGNORAR_MOVIMIENTO_PROPIO: Duration = Duration::from_millis(400);
const DISTANCIA_IMAN: i32 = 12;
const SEPARACION_IMAN: i32 = 8;

/// Todas las ventanas comparten un entorno de WebView2, así que todas deben crearse con los mismos argumentos.
pub const ARGUMENTOS_WEBVIEW: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --renderer-process-limit=1";

static ARRANQUE_LISTO: AtomicBool = AtomicBool::new(false);

pub fn marcar_arranque_listo() {
    ARRANQUE_LISTO.store(true, Ordering::Release);
}

/// En Windows, crear un WebView2 desde un callback del hilo principal (bandeja, segunda instancia)
/// mientras otro se está creando cuelga la app; esas acciones corren en otro hilo y tras el arranque.
pub fn diferir(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || {
        while !ARRANQUE_LISTO.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(50));
        }
        f(&app);
    });
}

#[derive(Default)]
struct EstadoRuntime {
    ultimo_movimiento: Option<Instant>,
    ignorar_hasta: Option<Instant>,
    arrastrando: bool,
    antes_de_maximizar: Option<(PhysicalPosition<i32>, PhysicalSize<u32>)>,
    desplazado: bool,
}

#[derive(Default)]
pub struct Gestor {
    pub escritorio_visible: AtomicBool,
    pub todos_visibles: AtomicBool,
    runtime: Mutex<HashMap<String, EstadoRuntime>>,
    firma_monitores: Mutex<String>,
}

impl Gestor {
    pub fn nuevo() -> Self {
        let g = Self::default();
        g.todos_visibles.store(true, Ordering::Relaxed);
        g
    }

    fn con_runtime<R>(&self, id: &str, f: impl FnOnce(&mut EstadoRuntime) -> R) -> R {
        let mut m = self.runtime.lock();
        f(m.entry(id.to_string()).or_default())
    }

    fn marcar_movimiento_propio(&self, id: &str) {
        self.con_runtime(id, |r| r.ignorar_hasta = Some(Instant::now() + IGNORAR_MOVIMIENTO_PROPIO));
    }

    pub fn maximizado(&self, id: &str) -> bool {
        self.con_runtime(id, |r| r.antes_de_maximizar.is_some())
    }
}

fn gestor(app: &AppHandle) -> tauri::State<'_, Gestor> {
    app.state::<Gestor>()
}

fn almacen(app: &AppHandle) -> tauri::State<'_, AlmacenConfig> {
    app.state::<AlmacenConfig>()
}

pub fn ventana_widget(app: &AppHandle, id: &str) -> Option<WebviewWindow> {
    app.get_webview_window(&catalogo::etiqueta(id))
}

fn color_acrylic(tema: Tema) -> Color {
    match tema {
        Tema::Claro => Color(236, 240, 247, 90),
        _ => Color(16, 20, 32, 110),
    }
}

pub fn aplicar_efecto(app: &AppHandle, ventana: &WebviewWindow, arrastrando: bool) {
    let tema = almacen(app).leer().apariencia.tema;
    let efecto = if arrastrando { Effect::Blur } else { Effect::Acrylic };
    let _ = ventana.set_effects(EffectsBuilder::new().effect(efecto).color(color_acrylic(tema)).build());
}

pub fn mostrar_ventana(ventana: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(windows)]
    let hwnd = win32::hwnd_de(ventana);
    #[cfg(windows)]
    if let Some(h) = hwnd {
        win32::proteger_estilos_widget(ventana);
        win32::aplicar_estilos_widget(h);
    }
    ventana.show()?;
    #[cfg(windows)]
    if let Some(h) = hwnd {
        win32::repintar_superficie(h);
    }
    Ok(())
}

/// Solo para el dock y los widgets: en paneles interactivos el nivel bajo puede volver lenta la página.
/// Durante el arranque el WebView2 todavía no existe: la petición viaja por la cola de eventos, detrás de su creación.
#[cfg(windows)]
pub fn memoria_baja(app: &AppHandle, etiqueta: &str) {
    let etiqueta = etiqueta.to_string();
    diferir(app, move |a| {
        if let Some(v) = a.get_webview_window(&etiqueta) {
            pedir_memoria_baja(&v);
        }
    });
}

#[cfg(windows)]
fn pedir_memoria_baja(ventana: &WebviewWindow) {
    let r = ventana.with_webview(|w| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW};
        use windows_core::Interface;
        let nivel = unsafe {
            w.controller()
                .CoreWebView2()
                .and_then(|c| c.cast::<ICoreWebView2_19>())
                .and_then(|c| c.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW))
        };
        if let Err(e) = nivel {
            log::warn!("WebView2 no aceptó el nivel de memoria bajo: {e}");
        }
    });
    if let Err(e) = r {
        log::warn!("Sin acceso a WebView2 para bajar la memoria: {e}");
    }
}

fn redondear(ventana: &WebviewWindow) {
    #[cfg(windows)]
    if let (Some(hwnd), Ok(tam), Ok(escala)) = (win32::hwnd_de(ventana), ventana.inner_size(), ventana.scale_factor()) {
        win32::esquinas_redondeadas(hwnd, tam.width as i32, tam.height as i32, (RADIO_ESQUINA * escala) as i32);
    }
}

pub fn aplicar_capa(app: &AppHandle, id: &str) {
    let Some(v) = ventana_widget(app, id) else { return };
    let cfg = almacen(app).leer();
    let encima = cfg.widget(id).fijado || cfg.capa_defecto == Capa::SiempreEncima;
    #[cfg(windows)]
    if let Some(hwnd) = win32::hwnd_de(&v) {
        if encima || gestor(app).escritorio_visible.load(Ordering::Relaxed) {
            win32::siempre_encima(hwnd, true);
        } else {
            win32::enviar_al_fondo(hwnd);
        }
    }
    #[cfg(not(windows))]
    let _ = (v, encima);
}

pub fn al_cambiar_primer_plano(app: &AppHandle, es_escritorio: bool) {
    gestor(app).escritorio_visible.store(es_escritorio, Ordering::Relaxed);
    for w in catalogo::WIDGETS {
        aplicar_capa(app, w.id);
    }
    paneles::aplicar_capa_dock(app);
}

fn monitores(app: &AppHandle) -> Vec<Monitor> {
    app.available_monitors().unwrap_or_default()
}

fn monitor_principal(app: &AppHandle) -> Option<Monitor> {
    app.primary_monitor().ok().flatten().or_else(|| monitores(app).into_iter().next())
}

fn nombre_monitor(m: &Monitor) -> String {
    m.name().cloned().unwrap_or_default()
}

fn contiene(m: &Monitor, x: i32, y: i32) -> bool {
    let p = m.position();
    let s = m.size();
    x >= p.x && y >= p.y && x < p.x + s.width as i32 && y < p.y + s.height as i32
}

fn monitor_de_punto(app: &AppHandle, x: i32, y: i32) -> Option<Monitor> {
    monitores(app).into_iter().find(|m| contiene(m, x, y))
}

fn tamano_fisico(def: &DefWidget, ancho: u32, alto: u32, escala: f64) -> PhysicalSize<u32> {
    let a = if ancho == 0 { def.ancho } else { ancho };
    let h = if alto == 0 { def.alto } else { alto };
    PhysicalSize::new((a as f64 * escala) as u32, (h as f64 * escala) as u32)
}

fn posicion_por_defecto(app: &AppHandle, indice: usize) -> Option<(PhysicalPosition<i32>, f64)> {
    let m = monitor_principal(app)?;
    let escala = m.scale_factor();
    let (x, y) = geometria::celda_por_defecto(area_de(&m), escala, indice);
    Some((PhysicalPosition::new(x, y), escala))
}

fn area_de(m: &Monitor) -> Rect {
    let a = m.work_area();
    Rect::nuevo(a.position.x, a.position.y, a.size.width as i32, a.size.height as i32)
}

fn posicion_guardada_valida(app: &AppHandle, id: &str) -> Option<PhysicalPosition<i32>> {
    let w = almacen(app).leer().widget(id);
    let (x, y) = (w.x?, w.y?);
    let m = monitor_de_punto(app, x + 20, y + 20)?;
    if let Some(nombre) = &w.monitor {
        if &nombre_monitor(&m) != nombre {
            return None;
        }
    }
    Some(PhysicalPosition::new(x, y))
}

pub fn abrir_widget(app: &AppHandle, id: &str) -> tauri::Result<()> {
    let Some(def) = catalogo::buscar(id) else { return Ok(()) };
    if let Some(v) = ventana_widget(app, id) {
        mostrar_ventana(&v)?;
        aplicar_capa(app, id);
        return Ok(());
    }
    let cfg = almacen(app).leer();
    let wcfg = cfg.widget(id);
    let indice = catalogo::WIDGETS.iter().position(|w| w.id == id).unwrap_or(0);
    let (pos, desplazado) = match posicion_guardada_valida(app, id) {
        Some(p) => (Some(p), false),
        None => (posicion_por_defecto(app, indice).map(|(p, _)| p), wcfg.x.is_some()),
    };
    let escala = pos
        .and_then(|p| monitor_de_punto(app, p.x + 20, p.y + 20))
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let tam = if wcfg.estado == EstadoWidget::Pildora {
        PhysicalSize::new((ANCHO_PILDORA as f64 * escala) as u32, (ALTO_PILDORA as f64 * escala) as u32)
    } else {
        tamano_fisico(def, wcfg.ancho, wcfg.alto, escala)
    };

    let url = WebviewUrl::App(format!("index.html?vista={id}").into());
    let ventana = WebviewWindowBuilder::new(app, catalogo::etiqueta(id), url)
        .title(format!("Glassdeck · {}", def.titulo))
        .additional_browser_args(ARGUMENTOS_WEBVIEW)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    ventana.set_size(tam)?;
    if let Some(p) = pos {
        gestor(app).marcar_movimiento_propio(id);
        ventana.set_position(p)?;
    }
    gestor(app).con_runtime(id, |r| r.desplazado = desplazado);
    aplicar_efecto(app, &ventana, false);
    redondear(&ventana);
    #[cfg(windows)]
    memoria_baja(app, &catalogo::etiqueta(id));
    let app2 = app.clone();
    let id2 = id.to_string();
    ventana.on_window_event(move |e| match e {
        WindowEvent::Moved(_) => al_mover(&app2, &id2),
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            if let Some(v) = ventana_widget(&app2, &id2) {
                redondear(&v);
            }
        }
        _ => {}
    });
    if gestor(app).todos_visibles.load(Ordering::Relaxed) {
        mostrar_ventana(&ventana)?;
    }
    aplicar_capa(app, id);
    Ok(())
}

pub fn cerrar_widget(app: &AppHandle, id: &str) -> Result<(), String> {
    almacen(app).modificar(|c| {
        if let Some(w) = c.widgets.get_mut(id) {
            w.activo = false;
        }
    })?;
    if let Some(v) = ventana_widget(app, id) {
        let _ = v.destroy();
    }
    gestor(app).runtime.lock().remove(id);
    let _ = app.emit("gd://config", ());
    Ok(())
}

pub fn activar_widget(app: &AppHandle, id: &str, activo: bool) -> Result<(), String> {
    if !activo {
        return cerrar_widget(app, id);
    }
    almacen(app).modificar(|c| {
        if let Some(w) = c.widgets.get_mut(id) {
            w.activo = true;
        }
    })?;
    abrir_widget(app, id).map_err(|e| e.to_string())?;
    let _ = app.emit("gd://config", ());
    Ok(())
}

fn al_mover(app: &AppHandle, id: &str) {
    let g = gestor(app);
    let propio = g.con_runtime(id, |r| r.ignorar_hasta.map(|t| Instant::now() < t).unwrap_or(false));
    if propio {
        return;
    }
    let empezo = g.con_runtime(id, |r| {
        r.ultimo_movimiento = Some(Instant::now());
        let antes = r.arrastrando;
        r.arrastrando = true;
        !antes
    });
    if empezo {
        if let Some(v) = ventana_widget(app, id) {
            aplicar_efecto(app, &v, true);
        }
    }
}

pub fn revisar_arrastres(app: &AppHandle) {
    let terminados: Vec<String> = {
        let g = gestor(app);
        let mut m = g.runtime.lock();
        m.iter_mut()
            .filter(|(_, r)| r.arrastrando && r.ultimo_movimiento.map(|t| t.elapsed() >= ESPERA_FIN_ARRASTRE).unwrap_or(false))
            .map(|(id, r)| {
                r.arrastrando = false;
                id.clone()
            })
            .collect()
    };
    for id in terminados {
        fin_de_arrastre(app, &id);
    }
}

fn fin_de_arrastre(app: &AppHandle, id: &str) {
    let Some(v) = ventana_widget(app, id) else { return };
    aplicar_efecto(app, &v, false);
    if gestor(app).maximizado(id) {
        return;
    }
    let Ok(mut pos) = v.outer_position() else { return };
    if almacen(app).leer().iman {
        if let Some(p) = imantar(app, id, &v, pos) {
            gestor(app).marcar_movimiento_propio(id);
            let _ = v.set_position(p);
            pos = p;
        }
    }
    let monitor = monitor_de_punto(app, pos.x + 20, pos.y + 20).map(|m| nombre_monitor(&m));
    gestor(app).con_runtime(id, |r| r.desplazado = false);
    let _ = almacen(app).modificar(|c| {
        if let Some(w) = c.widgets.get_mut(id) {
            w.x = Some(pos.x);
            w.y = Some(pos.y);
            w.monitor = monitor;
        }
    });
    aplicar_capa(app, id);
}

fn imantar(app: &AppHandle, id: &str, v: &WebviewWindow, pos: PhysicalPosition<i32>) -> Option<PhysicalPosition<i32>> {
    let tam = v.outer_size().ok()?;
    let m = monitor_de_punto(app, pos.x + 20, pos.y + 20)?;
    let sep = (SEPARACION_IMAN as f64 * m.scale_factor()) as i32;
    let dist = (DISTANCIA_IMAN as f64 * m.scale_factor()) as i32;
    let otros: Vec<Rect> = catalogo::WIDGETS
        .iter()
        .filter(|o| o.id != id)
        .filter_map(|o| ventana_widget(app, o.id))
        .filter_map(|ov| {
            let (op, os) = (ov.outer_position().ok()?, ov.outer_size().ok()?);
            Some(Rect::nuevo(op.x, op.y, os.width as i32, os.height as i32))
        })
        .collect();
    let actual = Rect::nuevo(pos.x, pos.y, tam.width as i32, tam.height as i32);
    geometria::ajustar_iman(actual, area_de(&m), &otros, sep, dist).map(|(x, y)| PhysicalPosition::new(x, y))
}

pub fn cambiar_estado(app: &AppHandle, id: &str, estado: EstadoWidget) -> Result<(), String> {
    let def = catalogo::buscar(id).ok_or("Widget desconocido")?;
    let v = ventana_widget(app, id).ok_or("El widget no está abierto")?;
    let escala = v.scale_factor().map_err(|e| e.to_string())?;
    let wcfg = almacen(app).leer().widget(id);
    let tam = match estado {
        EstadoWidget::Pildora => {
            PhysicalSize::new((ANCHO_PILDORA as f64 * escala) as u32, (ALTO_PILDORA as f64 * escala) as u32)
        }
        _ => tamano_fisico(def, wcfg.ancho, wcfg.alto, escala),
    };
    v.set_size(tam).map_err(|e| e.to_string())?;
    almacen(app).modificar(|c| {
        if let Some(w) = c.widgets.get_mut(id) {
            w.estado = estado;
        }
    })?;
    Ok(())
}

pub fn maximizar(app: &AppHandle, id: &str, activo: bool) -> Result<(), String> {
    let def = catalogo::buscar(id).ok_or("Widget desconocido")?;
    let v = ventana_widget(app, id).ok_or("El widget no está abierto")?;
    let g = gestor(app);
    if activo {
        if g.maximizado(id) {
            return Ok(());
        }
        let pos = v.outer_position().map_err(|e| e.to_string())?;
        let tam = v.inner_size().map_err(|e| e.to_string())?;
        let m = monitor_de_punto(app, pos.x + 20, pos.y + 20).or_else(|| monitor_principal(app)).ok_or("Sin monitor")?;
        let escala = m.scale_factor();
        let area = m.work_area();
        let ancho = ((def.ancho_max as f64 * escala) as u32).min(area.size.width.saturating_sub(48));
        let alto = ((def.alto_max as f64 * escala) as u32).min(area.size.height.saturating_sub(48));
        let x = area.position.x + (area.size.width as i32 - ancho as i32) / 2;
        let y = area.position.y + (area.size.height as i32 - alto as i32) / 2;
        g.con_runtime(id, |r| r.antes_de_maximizar = Some((pos, tam)));
        g.marcar_movimiento_propio(id);
        v.set_size(PhysicalSize::new(ancho, alto)).map_err(|e| e.to_string())?;
        v.set_position(PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        if let Some(hwnd) = win32::hwnd_de(&v) {
            win32::siempre_encima(hwnd, true);
        }
        let _ = v.set_focus();
    } else {
        let Some((pos, tam)) = g.con_runtime(id, |r| r.antes_de_maximizar.take()) else { return Ok(()) };
        g.marcar_movimiento_propio(id);
        v.set_size(tam).map_err(|e| e.to_string())?;
        v.set_position(pos).map_err(|e| e.to_string())?;
        aplicar_capa(app, id);
    }
    // `emit` llega a todas las ventanas: la carga lleva el id para que solo reaccione este widget.
    let _ = v.emit("gd://maximizado", serde_json::json!({ "id": id, "activo": activo }));
    Ok(())
}

pub fn fijar(app: &AppHandle, id: &str, fijado: bool) -> Result<(), String> {
    almacen(app).modificar(|c| {
        if let Some(w) = c.widgets.get_mut(id) {
            w.fijado = fijado;
        }
    })?;
    aplicar_capa(app, id);
    Ok(())
}

pub fn abrir_activos(app: &AppHandle) {
    let cfg = almacen(app).leer();
    for w in catalogo::WIDGETS {
        if cfg.widget(w.id).activo {
            if let Err(e) = abrir_widget(app, w.id) {
                log::error!("No se pudo abrir el widget {}: {e}", w.id);
            }
        }
    }
}

pub fn mostrar_ocultar_todo(app: &AppHandle) {
    let g = gestor(app);
    let visibles = !g.todos_visibles.load(Ordering::Relaxed);
    g.todos_visibles.store(visibles, Ordering::Relaxed);
    for w in catalogo::WIDGETS {
        if let Some(v) = ventana_widget(app, w.id) {
            let _ = if visibles { mostrar_ventana(&v) } else { v.hide() };
            if visibles {
                aplicar_capa(app, w.id);
            }
        }
    }
    let _ = app.emit("gd://visibilidad", visibles);
}

pub fn restablecer_posiciones(app: &AppHandle) -> Result<(), String> {
    almacen(app).modificar(|c| {
        for (id, w) in c.widgets.iter_mut() {
            w.x = None;
            w.y = None;
            w.monitor = None;
            w.estado = EstadoWidget::Normal;
            if let Some(def) = catalogo::buscar(id) {
                w.ancho = def.ancho;
                w.alto = def.alto;
            }
        }
    })?;
    for (i, w) in catalogo::WIDGETS.iter().enumerate() {
        let Some(v) = ventana_widget(app, w.id) else { continue };
        let Some((p, escala)) = posicion_por_defecto(app, i) else { continue };
        gestor(app).marcar_movimiento_propio(w.id);
        let _ = v.set_size(tamano_fisico(w, w.ancho, w.alto, escala));
        let _ = v.set_position(p);
        let _ = v.emit("gd://estado", EstadoWidget::Normal);
    }
    Ok(())
}

fn firma(ms: &[Monitor]) -> String {
    let mut partes: Vec<String> = ms
        .iter()
        .map(|m| format!("{}:{}x{}@{},{}:{}", nombre_monitor(m), m.size().width, m.size().height, m.position().x, m.position().y, m.scale_factor()))
        .collect();
    partes.sort();
    partes.join("|")
}

pub fn revisar_monitores(app: &AppHandle) {
    let ms = monitores(app);
    let nueva = firma(&ms);
    {
        let g = gestor(app);
        let mut actual = g.firma_monitores.lock();
        if *actual == nueva {
            return;
        }
        let primera = actual.is_empty();
        *actual = nueva;
        if primera {
            return;
        }
    }
    log::info!("Cambio de monitores detectado ({} conectados)", ms.len());
    let cfg = almacen(app).leer();
    let mut desplazados = 0usize;
    for (i, w) in catalogo::WIDGETS.iter().enumerate() {
        let Some(v) = ventana_widget(app, w.id) else { continue };
        let wcfg = cfg.widget(w.id);
        match posicion_guardada_valida(app, w.id) {
            Some(p) => {
                gestor(app).marcar_movimiento_propio(w.id);
                let _ = v.set_position(p);
                gestor(app).con_runtime(w.id, |r| r.desplazado = false);
            }
            None if wcfg.x.is_some() => {
                let actual_ok = v
                    .outer_position()
                    .ok()
                    .and_then(|p| monitor_de_punto(app, p.x + 20, p.y + 20))
                    .is_some();
                if !actual_ok || !gestor(app).con_runtime(w.id, |r| r.desplazado) {
                    if let Some((p, _)) = posicion_por_defecto(app, i + desplazados) {
                        gestor(app).marcar_movimiento_propio(w.id);
                        let _ = v.set_position(p);
                    }
                    gestor(app).con_runtime(w.id, |r| r.desplazado = true);
                    desplazados += 1;
                }
            }
            None => {}
        }
        aplicar_capa(app, w.id);
    }
}

pub fn aplicar_tema_a_todos(app: &AppHandle) {
    for w in catalogo::WIDGETS {
        if let Some(v) = ventana_widget(app, w.id) {
            aplicar_efecto(app, &v, false);
        }
    }
    for etiqueta in [paneles::DOCK, paneles::CUSTOMIZE, paneles::LIMPIEZA] {
        if let Some(v) = app.get_webview_window(etiqueta) {
            aplicar_efecto(app, &v, false);
        }
    }
}

pub fn iniciar_vigilancia(app: &AppHandle) {
    #[cfg(windows)]
    {
        let app2 = app.clone();
        win32::vigilar_primer_plano(move |es_escritorio| al_cambiar_primer_plano(&app2, es_escritorio));
    }
    let app3 = app.clone();
    std::thread::Builder::new()
        .name("glassdeck-ventanas".into())
        .spawn(move || {
            let mut ciclo: u32 = 0;
            loop {
                std::thread::sleep(Duration::from_millis(100));
                revisar_arrastres(&app3);
                paneles::revisar_dock(&app3);
                ciclo = ciclo.wrapping_add(1);
                if ciclo % 15 == 0 {
                    revisar_monitores(&app3);
                }
            }
        })
        .ok();
}

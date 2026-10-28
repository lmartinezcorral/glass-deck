//! Conexión de Cursor, claude.ai y ChatGPT (GD-CUR-01, GD-CLA-04, GD-OAI-04, GD-NFR-SEC-04).
//! La ventana de login carga la página del servicio en un perfil de WebView2 propio (`perfiles/<servicio>`), sin scripts
//! de Glassdeck y con una etiqueta que ninguna capacidad cubre: aunque Tauri inyecte su puente, cualquier comando se rechaza.
//! Las cookies se leen desde un hilo aparte (en el hilo principal WebView2 se bloquea) y solo pasan a la bóveda.

use crate::conexion::{interpretar, validar_peticion, ErrorRed, Metodo, Peticion, Respuesta, TransporteWeb};
use crate::recolectores::servicios::EstadoServicios;
use crate::recolectores::web::{seleccionar_cookies, ServicioWeb, EVENTO_CUENTAS};
use crate::seguridad::boveda;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const PREFIJO_ETIQUETA: &str = "login-";
pub const PREFIJO_LECTURA: &str = "lectura-";

const SONDEO_COOKIES: Duration = Duration::from_secs(1);
const ESPERA_LOGIN_MAX: Duration = Duration::from_secs(20 * 60);
const INTENTOS_BORRAR_PERFIL: u32 = 10;
const PAUSA_BORRAR_PERFIL: Duration = Duration::from_millis(500);
const ESPERA_CARGA: Duration = Duration::from_secs(30);
const ESPERA_PETICION: Duration = Duration::from_secs(30);
/// Encabezados que el navegador pone por su cuenta (y no deja tocar desde `fetch`).
const ENCABEZADOS_DEL_NAVEGADOR: &[&str] = &["cookie", "user-agent", "referer", "origin", "accept-language"];
/// `interpretar` aplica el límite real de 4 MiB; un carácter de más basta para que lo detecte.
const CARACTERES_MAX: usize = 4 * 1024 * 1024 + 1;
const PAGINA_ATERRIZAJE: &str = "favicon.ico";

pub fn etiqueta(s: ServicioWeb) -> String {
    format!("{PREFIJO_ETIQUETA}{}", s.id())
}

fn etiqueta_lectura(s: ServicioWeb) -> String {
    format!("{PREFIJO_LECTURA}{}", s.id())
}

fn dir_perfil(app: &AppHandle, s: ServicioWeb) -> Option<PathBuf> {
    app.path().local_data_dir().ok().map(|d| d.join("Glassdeck").join("perfiles").join(s.id()))
}

fn borrar_perfil(dir: &Path) -> bool {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => true,
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

fn avisar(app: &AppHandle) {
    let _ = app.emit(EVENTO_CUENTAS, ());
}

fn cerrar_lectura(app: &AppHandle, s: ServicioWeb) {
    if let Some(v) = app.get_webview_window(&etiqueta_lectura(s)) {
        let _ = v.destroy();
    }
}

/// Solo https dentro de la ventana de login; las ventanas emergentes se niegan.
fn navegacion_permitida(u: &Url) -> bool {
    u.scheme() == "https" || u.as_str() == "about:blank"
}

pub fn conectar(app: &AppHandle, s: ServicioWeb) -> Result<(), String> {
    if let Some(v) = app.get_webview_window(&etiqueta(s)) {
        let _ = v.show();
        let _ = v.set_focus();
        return Ok(());
    }
    let dir = dir_perfil(app, s).ok_or("Sin carpeta de datos local")?;
    cerrar_lectura(app, s);
    let perfil_limpio = borrar_perfil(&dir);
    let url = Url::parse(s.url_login()).map_err(|_| "Dirección de login inválida".to_string())?;
    WebviewWindowBuilder::new(app, etiqueta(s), WebviewUrl::External(url))
        .title(format!("Glassdeck · Conectar {}", s.nombre()))
        .inner_size(520.0, 760.0)
        .center()
        .data_directory(dir)
        .on_navigation(navegacion_permitida)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build()
        .map_err(|e| format!("No se pudo abrir la ventana de conexión: {e}"))?;
    log::info!("Conexión: ventana de login de {} abierta", s.nombre());
    avisar(app);
    let app = app.clone();
    std::thread::Builder::new()
        .name(format!("glassdeck-login-{}", s.id()))
        .spawn(move || vigilar_login(&app, s, perfil_limpio))
        .map_err(|_| "No se pudo vigilar el login".to_string())?;
    Ok(())
}

/// Si el perfil viejo no se pudo borrar, su cookie de sesión no cuenta: hay que esperar una nueva.
fn vigilar_login(app: &AppHandle, s: ServicioWeb, perfil_limpio: bool) {
    let inicio = Instant::now();
    let Ok(url_cookies) = Url::parse(s.url_cookies()) else { return };
    let mut previa: Option<String> = None;
    let mut primera = true;
    loop {
        std::thread::sleep(SONDEO_COOKIES);
        let Some(v) = app.get_webview_window(&etiqueta(s)) else {
            log::info!("Conexión: el login de {} se cerró sin terminar", s.nombre());
            avisar(app);
            return;
        };
        if inicio.elapsed() > ESPERA_LOGIN_MAX {
            log::info!("Conexión: el login de {} expiró", s.nombre());
            let _ = v.destroy();
            avisar(app);
            return;
        }
        let pares: Vec<(String, String)> = match v.cookies_for_url(url_cookies.clone()) {
            Ok(c) => c.iter().map(|c| (c.name().to_string(), c.value().to_string())).collect(),
            Err(_) => {
                log::warn!("Conexión: no se pudieron leer las cookies de {}", s.nombre());
                continue;
            }
        };
        let actual = seleccionar_cookies(s.cookies_sesion(), &pares);
        drop(pares);
        if primera {
            primera = false;
            if !perfil_limpio {
                previa = actual;
                continue;
            }
        }
        let Some(valor) = actual.filter(|a| Some(a) != previa.as_ref()) else { continue };
        if let Err(e) = boveda::guardar_largo(s.clave().nombre(), &valor) {
            log::warn!("Conexión: no se pudo guardar la sesión de {}: {e}", s.nombre());
            continue;
        }
        drop(valor);
        log::info!("Conexión: {} conectado", s.nombre());
        let _ = v.destroy();
        app.state::<EstadoServicios>().forzar(s.id());
        avisar(app);
        return;
    }
}

pub fn desconectar(app: &AppHandle, s: ServicioWeb) -> Result<(), String> {
    if let Some(v) = app.get_webview_window(&etiqueta(s)) {
        let _ = v.destroy();
    }
    cerrar_lectura(app, s);
    boveda::borrar_largo(s.clave().nombre())?;
    if let Some(dir) = dir_perfil(app, s) {
        let nombre = s.nombre();
        std::thread::spawn(move || {
            for _ in 0..INTENTOS_BORRAR_PERFIL {
                if borrar_perfil(&dir) {
                    return;
                }
                std::thread::sleep(PAUSA_BORRAR_PERFIL);
            }
            log::warn!("Conexión: no se pudo borrar el perfil de login de {nombre}; se borrará al volver a conectar");
        });
    }
    log::info!("Conexión: {} desconectado", s.nombre());
    app.state::<EstadoServicios>().forzar(s.id());
    avisar(app);
    Ok(())
}

/// Respaldo ante Cloudflare: un WebView invisible con el perfil del login hace el mismo `fetch` desde el propio origen.
/// Aterriza en una página sin scripts del sitio (el ícono) para que nadie haya reemplazado `fetch`, no tiene comandos de
/// Glassdeck (su etiqueta no está en ninguna capacidad) y el resultado vuelve solo por DevTools; se destruye al final de cada ronda.
pub struct NavegadorOculto {
    app: AppHandle,
    servicio: ServicioWeb,
}

impl NavegadorOculto {
    pub fn nuevo(app: AppHandle, servicio: ServicioWeb) -> Self {
        Self { app, servicio }
    }

    fn origen(&self) -> Result<Url, ErrorRed> {
        Url::parse(self.servicio.url_cookies()).map_err(|_| ErrorRed::DominioNoPermitido)
    }

    fn ventana(&self) -> Result<WebviewWindow, ErrorRed> {
        let etiqueta = etiqueta_lectura(self.servicio);
        if let Some(v) = self.app.get_webview_window(&etiqueta) {
            return Ok(v);
        }
        while !super::ARRANQUE_LISTO.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(50));
        }
        let dir = dir_perfil(&self.app, self.servicio).ok_or(ErrorRed::Cloudflare)?;
        let origen = self.origen()?;
        let aterrizaje = origen.join(PAGINA_ATERRIZAJE).map_err(|_| ErrorRed::DominioNoPermitido)?;
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let tx = parking_lot::Mutex::new(tx);
        let mismo_origen = origen.clone();
        let v = WebviewWindowBuilder::new(&self.app, etiqueta, WebviewUrl::External(aterrizaje))
            .title("Glassdeck · Lectura")
            .inner_size(320.0, 240.0)
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .data_directory(dir)
            .on_navigation(move |u| u.origin() == mismo_origen.origin())
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .on_page_load(move |_, carga| {
                if carga.event() == PageLoadEvent::Finished {
                    let _ = tx.lock().send(());
                }
            })
            .build()
            .map_err(|_| ErrorRed::Cloudflare)?;
        if rx.recv_timeout(ESPERA_CARGA).is_err() {
            let _ = v.destroy();
            return Err(ErrorRed::TiempoAgotado);
        }
        Ok(v)
    }
}

impl TransporteWeb for NavegadorOculto {
    fn enviar(&self, p: &Peticion) -> Result<Respuesta, ErrorRed> {
        let u = validar_peticion(p)?;
        if u.origin() != self.origen()?.origin() {
            return Err(ErrorRed::DominioNoPermitido);
        }
        let v = self.ventana()?;
        let texto = evaluar(&v, guion_fetch(p, &u))?;
        let (estado, encabezados, cuerpo) = leer_resultado(&texto)?;
        let r = interpretar(estado, &encabezados, cuerpo.as_bytes());
        if let Err(e) = &r {
            log::warn!("Red (WebView oculto): {} respondió {estado} ({e:?})", u.host_str().unwrap_or_default());
        }
        Ok(Respuesta::sin_cookies(estado, r?, encabezados))
    }

    fn terminar(&self) {
        cerrar_lectura(&self.app, self.servicio);
    }
}

/// `fetch` del propio origen con las cookies del perfil; una redirección (a un login) vuelve como 302.
fn guion_fetch(p: &Peticion, u: &Url) -> String {
    let mut encabezados: serde_json::Map<String, Value> = p
        .encabezados
        .iter()
        .filter(|(n, _)| !ENCABEZADOS_DEL_NAVEGADOR.iter().any(|x| n.eq_ignore_ascii_case(x)))
        .map(|(n, v)| (n.to_string(), Value::String(v.clone())))
        .collect();
    let metodo = if p.metodo == Metodo::Get { "GET" } else { "POST" };
    let mut opciones = json!({ "method": metodo, "credentials": "include", "redirect": "manual", "cache": "no-store" });
    if let Some(c) = &p.cuerpo {
        encabezados.insert("content-type".into(), Value::String("application/json".into()));
        opciones["body"] = Value::String(c.to_string());
    }
    opciones["headers"] = Value::Object(encabezados);
    format!(
        "(async () => {{ const r = await fetch({url}, {opciones}); if (r.type === 'opaqueredirect') return {{ estado: 302 }}; \
         const h = (n) => r.headers.get(n) || ''; \
         return {{ estado: r.status, tipo: h('content-type'), cf: h('cf-mitigated'), servidor: h('server'), cuerpo: (await r.text()).slice(0, {CARACTERES_MAX}) }}; }})()",
        url = Value::String(u.to_string()),
    )
}

/// Resultado de `Runtime.evaluate`; si `fetch` lanzó (red caída, CORS) no hay respuesta que interpretar.
fn leer_resultado(texto: &str) -> Result<(u16, Vec<(String, String)>, String), ErrorRed> {
    let v: Value = serde_json::from_str(texto).map_err(|_| ErrorRed::Formato)?;
    if v.get("exceptionDetails").is_some() {
        return Err(ErrorRed::SinConexion);
    }
    let r = v.pointer("/result/value").ok_or(ErrorRed::Formato)?;
    let estado = r.get("estado").and_then(Value::as_u64).and_then(|e| u16::try_from(e).ok()).ok_or(ErrorRed::Formato)?;
    let campo = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let encabezados = vec![
        ("content-type".to_string(), campo("tipo")),
        ("cf-mitigated".to_string(), campo("cf")),
        ("server".to_string(), campo("servidor")),
    ];
    Ok((estado, encabezados, campo("cuerpo")))
}

#[cfg(windows)]
fn evaluar(v: &WebviewWindow, guion: String) -> Result<String, ErrorRed> {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows_core::HSTRING;
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, ()>>();
    let parametros = json!({ "expression": guion, "awaitPromise": true, "returnByValue": true }).to_string();
    v.with_webview(move |w| {
        let tx_error = tx.clone();
        let manejador = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |r, texto| {
            let _ = tx.send(r.map(|_| texto).map_err(|_| ()));
            Ok(())
        }));
        let r = unsafe {
            w.controller().CoreWebView2().and_then(|c| {
                c.CallDevToolsProtocolMethod(&HSTRING::from("Runtime.evaluate"), &HSTRING::from(parametros), &manejador)
            })
        };
        if r.is_err() {
            let _ = tx_error.send(Err(()));
        }
    })
    .map_err(|_| ErrorRed::Cloudflare)?;
    match rx.recv_timeout(ESPERA_PETICION) {
        Ok(Ok(texto)) => Ok(texto),
        Ok(Err(())) => Err(ErrorRed::Cloudflare),
        Err(_) => Err(ErrorRed::TiempoAgotado),
    }
}

#[cfg(not(windows))]
fn evaluar(_: &WebviewWindow, _: String) -> Result<String, ErrorRed> {
    Err(ErrorRed::Cloudflare)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_etiquetas_de_login_y_lectura_no_son_de_widgets_ni_paneles() {
        for s in ServicioWeb::TODOS {
            for e in [etiqueta(s), etiqueta_lectura(s)] {
                assert!(!e.starts_with("w-") && !["dock", "customize", "limpieza"].contains(&e.as_str()), "{e}");
                assert!(e.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{e}");
            }
        }
    }

    fn coincide(patron: &str, etiqueta: &str) -> bool {
        match patron.split_once('*') {
            Some((antes, despues)) => etiqueta.starts_with(antes) && etiqueta.ends_with(despues),
            None => patron == etiqueta,
        }
    }

    #[test]
    fn ninguna_capacidad_cubre_las_ventanas_de_login_ni_direcciones_remotas() {
        let capacidades = [include_str!("../../capabilities/widgets.json"), include_str!("../../capabilities/paneles.json")];
        for texto in capacidades {
            let c: Value = serde_json::from_str(texto).unwrap();
            assert!(c.get("remote").is_none() && c.get("webviews").is_none(), "{}", c["identifier"]);
            let ventanas: Vec<&str> = c["windows"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
            for s in ServicioWeb::TODOS {
                for e in [etiqueta(s), etiqueta_lectura(s)] {
                    assert!(!ventanas.iter().any(|p| coincide(p, &e)), "{} cubre {e}", c["identifier"]);
                }
            }
        }
        assert!(coincide("w-*", "w-spending") && !coincide("w-*", "login-cursor"));
    }

    #[test]
    fn el_prototipo_se_congela_solo_en_las_ventanas_propias() {
        let conf: Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(conf["app"]["security"]["freezePrototype"], json!(false));
        assert_eq!(super::super::CONGELAR_PROTOTIPO, "Object.freeze(Object.prototype);");
        assert!(include_str!("mod.rs").contains(".initialization_script(CONGELAR_PROTOTIPO)"), "widgets");
        assert!(include_str!("paneles.rs").contains(".initialization_script(super::CONGELAR_PROTOTIPO)"), "paneles");
        assert!(!include_str!("login.rs").split("#[cfg(test)]").next().unwrap().contains(".initialization_script("));
    }

    #[test]
    fn la_ventana_de_login_solo_navega_por_https() {
        for (u, ok) in [
            ("https://claude.ai/login", true),
            ("https://accounts.google.com/o/oauth2", true),
            ("about:blank", true),
            ("http://claude.ai/", false),
            ("file:///C:/Windows/win.ini", false),
            ("tauri://localhost/index.html", false),
            ("javascript:alert(1)", false),
        ] {
            assert_eq!(navegacion_permitida(&Url::parse(u).unwrap()), ok, "{u}");
        }
    }

    #[test]
    fn el_fetch_del_webview_oculto_no_manda_cookies_ni_encabezados_del_navegador() {
        let p = crate::recolectores::web::como_navegador(
            Peticion::get("https://chatgpt.com/backend-api/wham/usage"),
            "https://chatgpt.com",
            &boveda::Secreto::de_prueba("x=cookie-de-prueba"),
        )
        .encabezado("authorization", "Bearer token-de-prueba");
        let u = validar_peticion(&p).unwrap();
        let g = guion_fetch(&p, &u);
        assert!(!g.contains("cookie-de-prueba") && !g.contains("user-agent") && !g.contains("referer"));
        assert!(g.contains("\"authorization\":\"Bearer token-de-prueba\"") && g.contains("\"credentials\":\"include\""));
        assert!(g.contains("\"redirect\":\"manual\"") && g.contains("\"method\":\"GET\""));
        let post = Peticion::post_lectura("https://cursor.com/api/dashboard/get-hard-limit", json!({ "a": 1 }));
        let g = guion_fetch(&post, &validar_peticion(&post).unwrap());
        assert!(g.contains("\"method\":\"POST\"") && g.contains("\"body\":\"{\\\"a\\\":1}\"") && g.contains("application/json"));
    }

    #[test]
    fn el_resultado_de_devtools_se_interpreta_como_una_respuesta_http() {
        let ok = json!({ "result": { "type": "object", "value": { "estado": 200, "tipo": "application/json", "cf": "", "servidor": "cloudflare", "cuerpo": "{\"x\":1}" } } });
        let (estado, enc, cuerpo) = leer_resultado(&ok.to_string()).unwrap();
        assert_eq!(interpretar(estado, &enc, cuerpo.as_bytes()), Ok(json!({ "x": 1 })));
        let reto = json!({ "result": { "value": { "estado": 403, "tipo": "text/html", "cf": "challenge", "cuerpo": "<html>" } } });
        let (estado, enc, cuerpo) = leer_resultado(&reto.to_string()).unwrap();
        assert_eq!(interpretar(estado, &enc, cuerpo.as_bytes()), Err(ErrorRed::Cloudflare));
        let redir = json!({ "result": { "value": { "estado": 302 } } });
        let (estado, enc, cuerpo) = leer_resultado(&redir.to_string()).unwrap();
        assert_eq!(interpretar(estado, &enc, cuerpo.as_bytes()), Err(ErrorRed::SesionExpirada));
        let fallo = json!({ "result": { "type": "object" }, "exceptionDetails": { "text": "TypeError: Failed to fetch" } });
        assert_eq!(leer_resultado(&fallo.to_string()).err(), Some(ErrorRed::SinConexion));
        assert_eq!(leer_resultado("no es json").err(), Some(ErrorRed::Formato));
    }
}

pub mod bucle;
pub mod calendario;
pub mod comandos;
pub mod gmail;
pub mod navegador;
pub mod oauth;
pub mod rotacion;

use crate::conexion::{self, ErrorRed};
use crate::config::Config;
use crate::recolectores::servicios::{Fuente, Motivo};
use crate::seguridad::boveda::{self, Clave};
use crate::seguridad::google_cliente::ClienteOAuth;
use bucle::{FinBucle, ServidorBucle};
use calendario::EventoAgenda;
use chrono::{DateTime, Local};
use gmail::CorreoNoLeido;
use parking_lot::Mutex;
use rotacion::{ElementoRotable, Rotador};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Margen para refrescar el token de acceso antes de que Google lo dé por vencido.
const MARGEN_ACCESO: Duration = Duration::from_secs(60);
const TIC_ALERTA: Duration = Duration::from_secs(30);

/// Las llamadas a Google pasan por aquí para probar todo el flujo con respuestas sintéticas, sin red.
pub trait ApiGoogle: Send + Sync {
    fn obtener(&self, url: &str, consulta: &[(&str, String)], token: &str) -> Result<serde_json::Value, ErrorRed>;
    fn formulario(&self, url: &str, campos: &[(&str, String)]) -> Result<serde_json::Value, ErrorRed>;
}

pub struct InternetGoogle;

impl ApiGoogle for InternetGoogle {
    fn obtener(&self, url: &str, consulta: &[(&str, String)], token: &str) -> Result<serde_json::Value, ErrorRed> {
        conexion::cliente().obtener_json_autorizado(url, consulta, token)
    }
    fn formulario(&self, url: &str, campos: &[(&str, String)]) -> Result<serde_json::Value, ErrorRed> {
        conexion::cliente().enviar_formulario(url, campos)
    }
}

/// La lista de cuentas vive en la bóveda (`Glassdeck/google/cuentas`), no en `settings.json`: el correo de
/// la cuenta no se escribe en disco en claro ni llega a logs. Sin `Debug` por la misma razón.
#[derive(Clone, Serialize, Deserialize)]
pub struct Cuenta {
    pub id: String,
    pub correo: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoCuenta {
    Conectada,
    Reconectar,
}

#[derive(Clone, Serialize)]
pub struct VistaCuenta {
    pub id: String,
    pub correo: String,
    pub estado: EstadoCuenta,
}

struct Acceso {
    token: String,
    caduca: Instant,
}

enum Fallo {
    Reconectar,
    Red(ErrorRed),
}

#[derive(Clone, Serialize)]
pub struct BuzonCuenta {
    pub cuenta: String,
    pub correo: String,
    pub estado: EstadoCuenta,
    pub error: Option<&'static str>,
    pub no_leidos: u32,
    pub mensajes: Vec<CorreoNoLeido>,
}

#[derive(Clone, Serialize)]
pub struct AgendaCuenta {
    pub cuenta: String,
    pub correo: String,
    pub estado: EstadoCuenta,
    pub error: Option<&'static str>,
    pub eventos: Vec<EventoAgenda>,
}

const TEXTO_RECONECTAR: &str = "Reconecta la cuenta";

type Aviso = Box<dyn Fn() + Send + Sync>;

/// Cuentas de Google conectadas: refresh token en la bóveda (uno por cuenta), token de acceso y encabezados
/// solo en memoria (GD-MAIL-05).
pub struct Google {
    api: Arc<dyn ApiGoogle>,
    prefijo: String,
    cuentas: Mutex<Vec<Cuenta>>,
    estados: Mutex<HashMap<String, EstadoCuenta>>,
    accesos: Mutex<HashMap<String, Acceso>>,
    cache_correo: Mutex<HashMap<String, HashMap<String, CorreoNoLeido>>>,
    ultimo_correo: Mutex<Vec<BuzonCuenta>>,
    ultima_agenda: Mutex<Vec<AgendaCuenta>>,
    conectando: AtomicBool,
    cancelar: AtomicBool,
    aviso: Mutex<Option<Aviso>>,
}

impl Google {
    pub fn nuevo(api: Arc<dyn ApiGoogle>) -> Self {
        let prefijo = Clave::GoogleCliente.nombre().split('/').next().unwrap_or("google");
        Self::con_prefijo(api, prefijo)
    }

    fn con_prefijo(api: Arc<dyn ApiGoogle>, prefijo: &str) -> Self {
        let g = Self {
            api,
            prefijo: prefijo.to_string(),
            cuentas: Mutex::new(Vec::new()),
            estados: Mutex::new(HashMap::new()),
            accesos: Mutex::new(HashMap::new()),
            cache_correo: Mutex::new(HashMap::new()),
            ultimo_correo: Mutex::new(Vec::new()),
            ultima_agenda: Mutex::new(Vec::new()),
            conectando: AtomicBool::new(false),
            cancelar: AtomicBool::new(false),
            aviso: Mutex::new(None),
        };
        *g.cuentas.lock() = g.cargar_cuentas();
        g
    }

    pub fn al_cambiar(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.aviso.lock() = Some(Box::new(f));
    }

    fn avisar(&self) {
        if let Some(f) = self.aviso.lock().as_ref() {
            f();
        }
    }

    fn nombre_lista(&self) -> String {
        format!("{}/cuentas", self.prefijo)
    }

    fn nombre_token(&self, id: &str) -> String {
        format!("{}/{id}", self.prefijo)
    }

    fn cargar_cuentas(&self) -> Vec<Cuenta> {
        let Ok(Some(s)) = boveda::leer(&self.nombre_lista()) else { return Vec::new() };
        serde_json::from_str::<Vec<Cuenta>>(s.exponer())
            .unwrap_or_default()
            .into_iter()
            .filter(|c| oauth::id_cuenta_valido(&c.id) && oauth::correo_valido(&c.correo))
            .collect()
    }

    fn guardar_cuentas(&self, cuentas: &[Cuenta]) -> Result<(), String> {
        if cuentas.is_empty() {
            return boveda::borrar(&self.nombre_lista());
        }
        let texto = serde_json::to_string(cuentas).map_err(|_| "No se pudo preparar la lista de cuentas".to_string())?;
        boveda::guardar(&self.nombre_lista(), &texto)
    }

    fn cliente(&self) -> Option<ClienteOAuth> {
        let s = boveda::leer(&format!("{}/cliente", self.prefijo)).ok().flatten()?;
        serde_json::from_str(s.exponer()).ok()
    }

    pub fn cuentas(&self) -> Vec<VistaCuenta> {
        let estados = self.estados.lock();
        self.cuentas
            .lock()
            .iter()
            .map(|c| VistaCuenta {
                id: c.id.clone(),
                correo: c.correo.clone(),
                estado: estados.get(&c.id).copied().unwrap_or(EstadoCuenta::Conectada),
            })
            .collect()
    }

    fn marcar(&self, id: &str, estado: EstadoCuenta) {
        let anterior = self.estados.lock().insert(id.to_string(), estado);
        if anterior.unwrap_or(EstadoCuenta::Conectada) != estado {
            log::info!("Google: la cuenta {id} pasó a {estado:?}");
            self.avisar();
        }
    }

    fn acceso(&self, id: &str, renovar: bool) -> Result<String, Fallo> {
        if !renovar {
            if let Some(a) = self.accesos.lock().get(id).filter(|a| a.caduca > Instant::now()) {
                return Ok(a.token.clone());
            }
        }
        self.accesos.lock().remove(id);
        let cliente = self.cliente().ok_or(Fallo::Reconectar)?;
        let refresh = boveda::leer(&self.nombre_token(id)).ok().flatten().ok_or(Fallo::Reconectar)?;
        match oauth::refrescar(&*self.api, &cliente, refresh.exponer()) {
            Ok(t) => {
                if let Some(nuevo) = t.refresh_token.as_deref().filter(|n| *n != refresh.exponer()) {
                    let _ = boveda::guardar(&self.nombre_token(id), nuevo);
                }
                let vida = Duration::from_secs(t.expires_in.unwrap_or(3_600)).saturating_sub(MARGEN_ACCESO);
                let token = t.access_token;
                self.accesos.lock().insert(id.to_string(), Acceso { token: token.clone(), caduca: Instant::now() + vida });
                Ok(token)
            }
            Err(ErrorRed::SesionExpirada | ErrorRed::Http(400)) => Err(Fallo::Reconectar),
            Err(e) => Err(Fallo::Red(e)),
        }
    }

    /// Un 401/403 de la API se reintenta una vez con un token recién refrescado; si sigue, la cuenta pide
    /// reconectar sin tocar a las demás (GD-MAIL-06).
    fn con_token<T>(&self, id: &str, mut f: impl FnMut(&str) -> Result<T, ErrorRed>) -> Result<T, Fallo> {
        let token = self.acceso(id, false)?;
        match f(&token) {
            Err(ErrorRed::SesionExpirada) => {
                let token = self.acceso(id, true)?;
                f(&token).map_err(|e| match e {
                    ErrorRed::SesionExpirada => Fallo::Reconectar,
                    e => Fallo::Red(e),
                })
            }
            r => r.map_err(Fallo::Red),
        }
    }

    fn lista(&self) -> Vec<Cuenta> {
        self.cuentas.lock().clone()
    }

    /// `Err` solo si todas las cuentas fallaron por la red; una cuenta caída no oculta a las otras.
    pub fn leer_correo(&self) -> Result<Vec<BuzonCuenta>, Motivo> {
        let cuentas = self.lista();
        let previos = self.ultimo_correo.lock().clone();
        let mut salida = Vec::with_capacity(cuentas.len());
        let mut fallo_red = None;
        let mut fallos = 0;
        for c in &cuentas {
            let mut cache = self.cache_correo.lock().remove(&c.id).unwrap_or_default();
            let r = self.con_token(&c.id, |t| gmail::leer_buzon(&*self.api, t, &mut cache));
            let base = BuzonCuenta {
                cuenta: c.id.clone(),
                correo: c.correo.clone(),
                estado: EstadoCuenta::Conectada,
                error: None,
                no_leidos: 0,
                mensajes: Vec::new(),
            };
            match r {
                Ok((no_leidos, mensajes)) => {
                    self.cache_correo.lock().insert(c.id.clone(), cache);
                    self.marcar(&c.id, EstadoCuenta::Conectada);
                    salida.push(BuzonCuenta { no_leidos, mensajes, ..base });
                }
                Err(Fallo::Reconectar) => {
                    self.marcar(&c.id, EstadoCuenta::Reconectar);
                    salida.push(BuzonCuenta { estado: EstadoCuenta::Reconectar, error: Some(TEXTO_RECONECTAR), ..base });
                }
                Err(Fallo::Red(e)) => {
                    self.cache_correo.lock().insert(c.id.clone(), cache);
                    let motivo = Motivo::from(e);
                    fallos += 1;
                    fallo_red = Some(motivo);
                    let previo = previos.iter().find(|b| b.cuenta == c.id).cloned().unwrap_or(base);
                    salida.push(BuzonCuenta { error: Some(motivo.texto()), ..previo });
                }
            }
        }
        *self.ultimo_correo.lock() = salida.clone();
        match fallo_red {
            Some(m) if fallos == cuentas.len() => Err(m),
            _ => Ok(salida),
        }
    }

    pub fn leer_agenda(&self, ahora: DateTime<Local>) -> Result<Vec<AgendaCuenta>, Motivo> {
        let (desde, hasta) = calendario::ventana(ahora);
        let cuentas = self.lista();
        let previos = self.ultima_agenda.lock().clone();
        let mut salida = Vec::with_capacity(cuentas.len());
        let mut fallo_red = None;
        let mut fallos = 0;
        for c in &cuentas {
            let base = AgendaCuenta {
                cuenta: c.id.clone(),
                correo: c.correo.clone(),
                estado: EstadoCuenta::Conectada,
                error: None,
                eventos: Vec::new(),
            };
            match self.con_token(&c.id, |t| calendario::leer_agenda(&*self.api, t, &desde, &hasta)) {
                Ok(eventos) => {
                    self.marcar(&c.id, EstadoCuenta::Conectada);
                    salida.push(AgendaCuenta { eventos, ..base });
                }
                Err(Fallo::Reconectar) => {
                    self.marcar(&c.id, EstadoCuenta::Reconectar);
                    salida.push(AgendaCuenta { estado: EstadoCuenta::Reconectar, error: Some(TEXTO_RECONECTAR), ..base });
                }
                Err(Fallo::Red(e)) => {
                    let motivo = Motivo::from(e);
                    fallos += 1;
                    fallo_red = Some(motivo);
                    let previo = previos.iter().find(|a| a.cuenta == c.id).cloned().unwrap_or(base);
                    salida.push(AgendaCuenta { error: Some(motivo.texto()), ..previo });
                }
            }
        }
        *self.ultima_agenda.lock() = salida.clone();
        match fallo_red {
            Some(m) if fallos == cuentas.len() => Err(m),
            _ => Ok(salida),
        }
    }

    /// Correos y eventos que aún no terminan; las claves coinciden con las que arma el widget.
    pub fn elementos_rotables(&self, ahora_ms: i64) -> Vec<ElementoRotable> {
        let mut v: Vec<ElementoRotable> = self
            .ultimo_correo
            .lock()
            .iter()
            .flat_map(|b| {
                b.mensajes.iter().map(|m| ElementoRotable { clave: format!("correo:{}:{}", b.cuenta, m.id), inicio_ms: None })
            })
            .collect();
        for a in self.ultima_agenda.lock().iter() {
            for e in a.eventos.iter().filter(|e| e.fin_ms > ahora_ms) {
                v.push(ElementoRotable {
                    clave: format!("evento:{}:{}", a.cuenta, e.id),
                    inicio_ms: (!e.todo_el_dia).then_some(e.inicio_ms),
                });
            }
        }
        v
    }

    pub fn cancelar_conexion(&self) {
        self.cancelar.store(true, Ordering::SeqCst);
    }

    pub fn conectar(&self) -> Result<VistaCuenta, String> {
        self.conectar_con(navegador::abrir_autorizacion, bucle::TIEMPO_LIMITE)
    }

    /// GD-MAIL-01: PKCE S256, `state` aleatorio y servidor de un solo uso en 127.0.0.1; `abrir` lleva la URL
    /// al navegador predeterminado.
    pub fn conectar_con(&self, abrir: impl FnOnce(&str) -> Result<(), String>, limite: Duration) -> Result<VistaCuenta, String> {
        if self.conectando.swap(true, Ordering::SeqCst) {
            return Err("Ya hay una conexión con Google en curso".into());
        }
        self.cancelar.store(false, Ordering::SeqCst);
        let r = self.conectar_interno(abrir, limite);
        self.conectando.store(false, Ordering::SeqCst);
        r
    }

    fn conectar_interno(&self, abrir: impl FnOnce(&str) -> Result<(), String>, limite: Duration) -> Result<VistaCuenta, String> {
        let cliente = self.cliente().ok_or("Primero importa el cliente de Google (sigue la guía)")?;
        let pkce = oauth::Pkce::nuevo()?;
        let estado = oauth::estado_aleatorio()?;
        let servidor = ServidorBucle::abrir().map_err(|_| "No se pudo abrir el puerto local para Google".to_string())?;
        let redireccion = servidor.redireccion();
        abrir(&oauth::url_autorizacion(&cliente.client_id, &redireccion, &pkce.reto, &estado))?;
        let codigo = match servidor.esperar(&estado, limite, &self.cancelar) {
            FinBucle::Codigo(c) => c,
            FinBucle::Denegado(_) => return Err("Google no autorizó la conexión".into()),
            FinBucle::TiempoAgotado => return Err("Se agotaron los 5 minutos para autorizar; vuelve a intentarlo".into()),
            FinBucle::Cancelado => return Err("Conexión cancelada".into()),
            FinBucle::Error => return Err("Falló el puerto local de la conexión".into()),
        };
        let t = oauth::intercambiar_codigo(&*self.api, &cliente, &codigo, &pkce.verificador, &redireccion)
            .map_err(|e| format!("Google rechazó la autorización ({})", Motivo::from(e).texto()))?;
        if !oauth::permisos_suficientes(t.scope.as_deref()) {
            let _ = oauth::revocar(&*self.api, t.refresh_token.as_deref().unwrap_or(&t.access_token));
            return Err("Deja marcados los permisos de Gmail y de Calendar en la pantalla de Google".into());
        }
        let identidad = oauth::identidad(t.id_token.as_deref().unwrap_or_default(), &cliente.client_id)?;
        let Some(refresh) = t.refresh_token else {
            return Err("Google no entregó acceso permanente; quita Glassdeck en tu cuenta de Google y vuelve a conectar".into());
        };
        let id = oauth::id_de_cuenta(&identidad.sub);
        boveda::guardar(&self.nombre_token(&id), &refresh)?;
        let mut cuentas = self.lista();
        cuentas.retain(|c| c.id != id);
        cuentas.push(Cuenta { id: id.clone(), correo: identidad.correo.clone() });
        if let Err(e) = self.guardar_cuentas(&cuentas) {
            let _ = boveda::borrar(&self.nombre_token(&id));
            return Err(e);
        }
        *self.cuentas.lock() = cuentas;
        let vida = Duration::from_secs(t.expires_in.unwrap_or(3_600)).saturating_sub(MARGEN_ACCESO);
        self.accesos.lock().insert(id.clone(), Acceso { token: t.access_token, caduca: Instant::now() + vida });
        self.estados.lock().insert(id.clone(), EstadoCuenta::Conectada);
        log::info!("Google: cuenta {id} conectada");
        self.avisar();
        Ok(VistaCuenta { id, correo: identidad.correo, estado: EstadoCuenta::Conectada })
    }

    /// Revoca en Google (si hay red) y borra el token de la bóveda aunque la revocación falle.
    pub fn desconectar(&self, id: &str) -> Result<(), String> {
        if !oauth::id_cuenta_valido(id) || !self.lista().iter().any(|c| c.id == id) {
            return Err("Esa cuenta no está conectada".into());
        }
        if let Ok(Some(t)) = boveda::leer(&self.nombre_token(id)) {
            if let Err(e) = oauth::revocar(&*self.api, t.exponer()) {
                log::warn!("Google: no se pudo revocar la cuenta {id} ({e:?}); se borra de todos modos");
            }
        }
        boveda::borrar(&self.nombre_token(id))?;
        let mut cuentas = self.lista();
        cuentas.retain(|c| c.id != id);
        self.guardar_cuentas(&cuentas)?;
        *self.cuentas.lock() = cuentas;
        self.accesos.lock().remove(id);
        self.estados.lock().remove(id);
        self.cache_correo.lock().remove(id);
        self.ultimo_correo.lock().retain(|b| b.cuenta != id);
        self.ultima_agenda.lock().retain(|a| a.cuenta != id);
        log::info!("Google: cuenta {id} desconectada");
        self.avisar();
        Ok(())
    }
}

/// Sin persistencia: los encabezados y eventos nunca llegan a `state.json` (GD-MAIL-05).
pub struct FuenteCorreo(pub Arc<Google>);

impl Fuente for FuenteCorreo {
    fn id(&self) -> &'static str {
        "correo"
    }
    fn widget(&self) -> &'static str {
        "correo"
    }
    fn intervalo(&self, cfg: &Config) -> Duration {
        Duration::from_millis(cfg.intervalos.correo_ms)
    }
    fn recolectar(&mut self, _: &Config) -> Result<serde_json::Value, Motivo> {
        Ok(serde_json::json!({ "cuentas": self.0.leer_correo()? }))
    }
}

pub struct FuenteAgenda(pub Arc<Google>);

impl Fuente for FuenteAgenda {
    fn id(&self) -> &'static str {
        "calendario"
    }
    fn widget(&self) -> &'static str {
        "correo"
    }
    fn intervalo(&self, cfg: &Config) -> Duration {
        Duration::from_millis(cfg.intervalos.calendario_ms)
    }
    fn recolectar(&mut self, _: &Config) -> Result<serde_json::Value, Motivo> {
        Ok(serde_json::json!({ "cuentas": self.0.leer_agenda(Local::now())? }))
    }
}

/// Decide qué se muestra en la alerta; no llama a Google, solo usa lo último leído.
pub struct FuenteAlerta {
    google: Arc<Google>,
    rotador: Rotador,
}

impl FuenteAlerta {
    pub fn nueva(google: Arc<Google>) -> Self {
        let semilla = crate::config::estado::ahora_ms() as u64;
        Self { google, rotador: Rotador::nuevo(semilla) }
    }
}

impl Fuente for FuenteAlerta {
    fn id(&self) -> &'static str {
        "alerta"
    }
    fn widget(&self) -> &'static str {
        "correo"
    }
    fn intervalo(&self, _: &Config) -> Duration {
        TIC_ALERTA
    }
    fn recolectar(&mut self, cfg: &Config) -> Result<serde_json::Value, Motivo> {
        let ahora = crate::config::estado::ahora_ms();
        let elementos = self.google.elementos_rotables(ahora);
        let alerta = self.rotador.avanzar(&elementos, ahora, cfg.intervalos.rotacion_ms as i64);
        serde_json::to_value(alerta).map_err(|_| Motivo::Interno)
    }
}

#[cfg(test)]
mod pruebas {
    use super::oauth::pruebas::{id_token_sintetico, CLIENT_ID};
    use super::*;
    use std::collections::HashSet;
    use std::io::Write;
    use std::sync::atomic::AtomicUsize;

    /// Google simulado: los tokens llevan el sufijo de la cuenta (`uno`, `dos`) y `revocadas` imita a
    /// Luis quitando el acceso desde su cuenta de Google.
    #[derive(Default)]
    struct Falsa {
        revocadas: Mutex<HashSet<String>>,
        revocaciones: Mutex<Vec<String>>,
        refrescos: AtomicUsize,
    }

    fn sufijo(token: &str) -> String {
        token.rsplit('-').next().unwrap_or_default().to_string()
    }

    impl ApiGoogle for Falsa {
        fn obtener(&self, url: &str, consulta: &[(&str, String)], token: &str) -> Result<serde_json::Value, ErrorRed> {
            assert!(conexion::validar_url(url, consulta).is_ok(), "{url}");
            if !token.starts_with("ya29.acceso-ficticio-") || self.revocadas.lock().contains(&sufijo(token)) {
                return Err(ErrorRed::SesionExpirada);
            }
            assert!(!consulta.iter().any(|(_, v)| v == "full" || v == "raw"));
            let json = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
            Ok(match url {
                gmail::URL_MENSAJES => json(include_str!("../grabaciones/google_gmail_lista_sintetica.json")),
                u if u.starts_with(gmail::URL_MENSAJES) => {
                    let mut m = json(include_str!("../grabaciones/google_gmail_mensaje_sintetico.json"));
                    m["id"] = u.rsplit('/').next().unwrap().into();
                    m
                }
                u if u.ends_with("/labels/INBOX") => serde_json::json!({ "messagesUnread": 7 }),
                calendario::URL_CALENDARIOS => json(include_str!("../grabaciones/google_calendarios_sinteticos.json")),
                u if u.ends_with("/events") => json(include_str!("../grabaciones/google_eventos_sinteticos.json")),
                otro => panic!("URL inesperada {otro}"),
            })
        }

        fn formulario(&self, url: &str, campos: &[(&str, String)]) -> Result<serde_json::Value, ErrorRed> {
            let campo = |k: &str| campos.iter().find(|(c, _)| *c == k).map(|(_, v)| v.clone()).unwrap_or_default();
            match url {
                oauth::URL_REVOCAR => {
                    self.revocaciones.lock().push(campo("token"));
                    Ok(serde_json::Value::Null)
                }
                oauth::URL_TOKEN if campo("grant_type") == "authorization_code" => {
                    assert_eq!(campo("code_verifier").len(), 43);
                    assert!(campo("redirect_uri").starts_with("http://127.0.0.1:"));
                    let s = campo("code").trim_start_matches("codigo-").to_string();
                    let scope = if s == "sinpermiso" {
                        format!("openid {}", oauth::SCOPE_EVENTOS)
                    } else {
                        oauth::SCOPES[2..].join(" ")
                    };
                    Ok(serde_json::json!({
                        "access_token": format!("ya29.acceso-ficticio-{s}"),
                        "refresh_token": format!("1//refresh-ficticio-{s}"),
                        "expires_in": 3599,
                        "scope": scope,
                        "id_token": id_token_sintetico(&format!("sub{s}"), &format!("cuenta.{s}@example.com")),
                    }))
                }
                oauth::URL_TOKEN => {
                    self.refrescos.fetch_add(1, Ordering::SeqCst);
                    let s = sufijo(&campo("refresh_token"));
                    if self.revocadas.lock().contains(&s) {
                        return Err(ErrorRed::Http(400));
                    }
                    Ok(serde_json::json!({ "access_token": format!("ya29.acceso-ficticio-{s}"), "expires_in": 3599 }))
                }
                otro => panic!("URL inesperada {otro}"),
            }
        }
    }

    /// Hace de navegador: manda a 127.0.0.1 la redirección que Google haría con `codigo`.
    fn navegador_falso(codigo: &'static str) -> impl FnOnce(&str) -> Result<(), String> {
        move |url: &str| {
            let u = reqwest::Url::parse(url).unwrap();
            assert_eq!(u.host_str(), Some("accounts.google.com"));
            let p: HashMap<String, String> = u.query_pairs().into_owned().collect();
            let destino = p["redirect_uri"].trim_start_matches("http://").to_string();
            let peticion = format!("GET /?state={}&code={codigo} HTTP/1.1\r\nHost: {destino}\r\n\r\n", p["state"]);
            std::thread::spawn(move || {
                let mut s = std::net::TcpStream::connect(destino).unwrap();
                s.write_all(peticion.as_bytes()).unwrap();
            });
            Ok(())
        }
    }

    fn preparar(prefijo: &str) -> (Arc<Falsa>, Google) {
        let cliente = serde_json::json!({ "client_id": CLIENT_ID, "client_secret": "secreto-ficticio-cliente" });
        boveda::guardar(&format!("{prefijo}/cliente"), &cliente.to_string()).unwrap();
        let falsa = Arc::new(Falsa::default());
        let g = Google::con_prefijo(falsa.clone(), prefijo);
        (falsa, g)
    }

    fn conectar(g: &Google, codigo: &'static str) -> VistaCuenta {
        g.conectar_con(navegador_falso(codigo), Duration::from_secs(20)).unwrap()
    }

    #[test]
    fn conectar_guarda_solo_el_refresh_en_la_boveda_y_la_lista_sin_tokens() {
        let prefijo = "prueba-google-conectar";
        let (_, g) = preparar(prefijo);
        let uno = conectar(&g, "codigo-uno");
        let dos = conectar(&g, "codigo-dos");
        assert_eq!(uno.correo, "cuenta.uno@example.com");
        assert!(oauth::id_cuenta_valido(&uno.id) && uno.id != dos.id);
        assert_eq!(boveda::leer(&format!("{prefijo}/{}", uno.id)).unwrap().unwrap().exponer(), "1//refresh-ficticio-uno");
        let lista = boveda::leer(&format!("{prefijo}/cuentas")).unwrap().unwrap();
        assert!(lista.exponer().contains("cuenta.dos@example.com"));
        assert!(!lista.exponer().contains("ficticio"), "la lista no lleva tokens");
        assert!(!boveda::mock::objetivos().iter().any(|o| o.contains("example.com")), "el correo no va en el nombre");
        let otra = Google::con_prefijo(Arc::new(Falsa::default()), prefijo);
        assert_eq!(otra.cuentas().len(), 2, "la lista sobrevive a un reinicio");
        assert_eq!(conectar(&g, "codigo-uno").id, uno.id, "reconectar la misma cuenta no la duplica");
        assert_eq!(g.cuentas().len(), 2);
    }

    #[test]
    fn sin_cliente_o_sin_permisos_no_se_conecta() {
        let g = Google::con_prefijo(Arc::new(Falsa::default()), "prueba-google-sin-cliente");
        let e = g.conectar_con(|_| panic!("no debe abrir el navegador"), Duration::from_secs(1)).err().unwrap();
        assert!(e.contains("importa el cliente"));
        let (falsa, g) = preparar("prueba-google-sin-permiso");
        let e = g.conectar_con(navegador_falso("codigo-sinpermiso"), Duration::from_secs(20)).err().unwrap();
        assert!(e.contains("Gmail"));
        assert!(g.cuentas().is_empty());
        assert_eq!(falsa.revocaciones.lock().as_slice(), ["1//refresh-ficticio-sinpermiso"]);
        let e = g.conectar_con(|_| Ok(()), Duration::from_millis(300)).err().unwrap();
        assert!(e.contains("5 minutos"), "{e}");
    }

    #[test]
    fn un_401_pone_en_ambar_solo_esa_cuenta() {
        let (falsa, g) = preparar("prueba-google-ambar");
        let uno = conectar(&g, "codigo-uno");
        let dos = conectar(&g, "codigo-dos");
        let antes = g.leer_correo().unwrap();
        assert!(antes.iter().all(|b| b.estado == EstadoCuenta::Conectada && b.mensajes.len() == 2 && b.no_leidos == 7));
        assert_eq!(falsa.refrescos.load(Ordering::SeqCst), 0, "el token de acceso se reutiliza mientras vive");
        falsa.revocadas.lock().insert("dos".into());
        let despues = g.leer_correo().expect("una cuenta caída no tumba la lectura");
        let b_uno = despues.iter().find(|b| b.cuenta == uno.id).unwrap();
        let b_dos = despues.iter().find(|b| b.cuenta == dos.id).unwrap();
        assert_eq!((b_uno.estado, b_uno.mensajes.len()), (EstadoCuenta::Conectada, 2));
        assert_eq!((b_dos.estado, b_dos.mensajes.len()), (EstadoCuenta::Reconectar, 0));
        assert_eq!(b_dos.error, Some(TEXTO_RECONECTAR));
        let estados: HashMap<String, EstadoCuenta> = g.cuentas().into_iter().map(|c| (c.id, c.estado)).collect();
        assert_eq!(estados[&uno.id], EstadoCuenta::Conectada);
        assert_eq!(estados[&dos.id], EstadoCuenta::Reconectar);
        let agenda = g.leer_agenda(Local::now()).unwrap();
        let a_uno = agenda.iter().find(|a| a.cuenta == uno.id).unwrap();
        assert_eq!(a_uno.eventos.len(), 3);
        assert_eq!(agenda.iter().find(|a| a.cuenta == dos.id).unwrap().estado, EstadoCuenta::Reconectar);
    }

    #[test]
    fn la_alerta_rota_entre_correos_y_eventos_de_varias_cuentas() {
        let (_, g) = preparar("prueba-google-rotar");
        let uno = conectar(&g, "codigo-uno");
        conectar(&g, "codigo-dos");
        g.leer_correo().unwrap();
        let ahora = DateTime::parse_from_rfc3339("2026-10-28T22:40:00Z").unwrap().with_timezone(&Local);
        g.leer_agenda(ahora).unwrap();
        let elementos = g.elementos_rotables(ahora.timestamp_millis());
        assert_eq!(elementos.iter().filter(|e| e.clave.starts_with("correo:")).count(), 4);
        assert!(elementos.iter().any(|e| e.clave == format!("correo:{}:18f0000000000001", uno.id)));
        let mut r = Rotador::nuevo(11);
        let a = r.avanzar(&elementos, ahora.timestamp_millis(), 600_000);
        assert!(a.clave.unwrap().starts_with("evento:"), "la junta de las 23:00 empieza en 20 min");
    }

    #[test]
    fn desconectar_revoca_y_borra_de_la_boveda() {
        let prefijo = "prueba-google-desconectar";
        let (falsa, g) = preparar(prefijo);
        let uno = conectar(&g, "codigo-uno");
        let dos = conectar(&g, "codigo-dos");
        g.leer_correo().unwrap();
        g.desconectar(&uno.id).unwrap();
        assert_eq!(falsa.revocaciones.lock().as_slice(), ["1//refresh-ficticio-uno"]);
        assert!(!boveda::existe(&format!("{prefijo}/{}", uno.id)));
        assert!(boveda::existe(&format!("{prefijo}/{}", dos.id)));
        assert_eq!(g.cuentas().iter().map(|c| c.id.clone()).collect::<Vec<_>>(), vec![dos.id.clone()]);
        assert!(g.elementos_rotables(0).iter().all(|e| !e.clave.contains(&uno.id)));
        assert!(g.desconectar(&uno.id).is_err());
        g.desconectar(&dos.id).unwrap();
        assert!(!boveda::existe(&format!("{prefijo}/cuentas")));
    }

    #[test]
    fn el_refresh_token_y_los_correos_no_llegan_a_los_logs() {
        let c = crate::seguridad::registro::captura_de_prueba();
        let (falsa, g) = preparar("prueba-google-logs");
        let uno = conectar(&g, "codigo-uno");
        conectar(&g, "codigo-dos");
        g.leer_correo().unwrap();
        g.leer_agenda(Local::now()).unwrap();
        falsa.revocadas.lock().insert("dos".into());
        g.leer_correo().unwrap();
        g.desconectar(&uno.id).unwrap();
        let texto = c.lock().clone();
        assert!(texto.contains("conectada"), "el registro de prueba debe estar activo");
        for prohibido in ["refresh-ficticio", "acceso-ficticio", "secreto-ficticio", "@example.com", "Banco Ejemplo", "Revisión semanal"] {
            assert!(!texto.contains(prohibido), "{prohibido} llegó a los logs");
        }
        let linea = "token=1//0gFicticioABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 refresh 1//0gFicticioABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let filtrada = crate::seguridad::filtro::enmascarar(linea);
        assert!(!filtrada.contains("0gFicticio"), "{filtrada}");
    }
}

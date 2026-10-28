use super::EstadoRecoleccion;
use crate::avisos::{self, Uso};
use crate::conexion::{ErrorRed, Reintentos};
use crate::config::estado::{ahora_ms, UltimoDato};
use crate::config::{AlmacenConfig, AlmacenEstado, Config};
use crate::ventanas::catalogo;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const TIC: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    SinConexion,
    FormatoCambio,
    SesionExpirada,
    ServicioNoDisponible,
    DominioNoPermitido,
    SinUbicacion,
    Interno,
    Cloudflare,
}

impl Motivo {
    pub fn texto(self) -> &'static str {
        match self {
            Motivo::SinConexion => "Sin conexión",
            Motivo::FormatoCambio => "Formato cambió",
            Motivo::SesionExpirada => "Sesión expirada",
            Motivo::ServicioNoDisponible => "Servicio no disponible",
            Motivo::DominioNoPermitido => "Dominio no permitido",
            Motivo::SinUbicacion => "Sin ubicación",
            Motivo::Interno => "Error interno",
            Motivo::Cloudflare => "Cloudflare bloqueó la lectura",
        }
    }
}

impl From<ErrorRed> for Motivo {
    fn from(e: ErrorRed) -> Self {
        match e {
            ErrorRed::SinConexion | ErrorRed::TiempoAgotado => Motivo::SinConexion,
            ErrorRed::SesionExpirada => Motivo::SesionExpirada,
            ErrorRed::ServicioNoDisponible | ErrorRed::Http(_) => Motivo::ServicioNoDisponible,
            ErrorRed::DominioNoPermitido => Motivo::DominioNoPermitido,
            ErrorRed::Formato => Motivo::FormatoCambio,
            ErrorRed::Cloudflare => Motivo::Cloudflare,
        }
    }
}

/// Último dato válido con su hora; si la última lectura falló, `error` dice por qué y `datos` se conserva.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct EstadoFuente {
    pub datos: Option<serde_json::Value>,
    pub hora_ms: Option<i64>,
    pub error: Option<Motivo>,
    pub texto_error: Option<&'static str>,
    pub proximo_intento_ms: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct LecturaServicio {
    pub fuente: &'static str,
    pub datos: EstadoFuente,
}

pub trait Fuente: Send {
    fn id(&self) -> &'static str;
    fn widget(&self) -> &'static str;
    fn intervalo(&self, cfg: &Config) -> Duration;
    /// Solo las fuentes sin datos personales guardan su último dato en `state.json`.
    fn persistente(&self) -> bool {
        false
    }
    fn recolectar(&mut self, cfg: &Config) -> Result<serde_json::Value, Motivo>;
    /// Las fuentes de gasto traducen su dato a porcentajes por ciclo para el motor de avisos.
    fn usos(&self, _datos: &serde_json::Value, _cfg: &Config) -> Vec<Uso> {
        Vec::new()
    }
}

pub struct Ejecutor {
    fuente: Box<dyn Fuente>,
    proximo: Option<Instant>,
    reintentos: Reintentos,
    pub estado: EstadoFuente,
}

impl Ejecutor {
    pub fn nuevo(fuente: Box<dyn Fuente>, previo: Option<UltimoDato>) -> Self {
        let estado = previo
            .map(|p| EstadoFuente { datos: Some(p.datos), hora_ms: Some(p.hora_ms), ..Default::default() })
            .unwrap_or_default();
        Self { fuente, proximo: None, reintentos: Reintentos::default(), estado }
    }

    pub fn vencido(&self, ahora: Instant) -> bool {
        self.proximo.map(|t| ahora >= t).unwrap_or(true)
    }

    pub fn forzar(&mut self) {
        self.proximo = None;
    }

    /// Devuelve el dato nuevo cuando la lectura salió bien y la fuente es persistente.
    pub fn ejecutar(&mut self, cfg: &Config, ahora: Instant, ahora_ms: i64) -> Option<UltimoDato> {
        let intervalo = self.fuente.intervalo(cfg);
        match self.fuente.recolectar(cfg) {
            Ok(datos) => {
                self.reintentos.al_tener_exito();
                self.proximo = Some(ahora + intervalo);
                self.estado = EstadoFuente { datos: Some(datos), hora_ms: Some(ahora_ms), ..Default::default() };
                self.fuente
                    .persistente()
                    .then(|| UltimoDato { hora_ms: ahora_ms, datos: self.estado.datos.clone().unwrap_or_default() })
            }
            Err(motivo) => {
                let espera = self.reintentos.al_fallar().min(intervalo);
                self.proximo = Some(ahora + espera);
                self.estado.error = Some(motivo);
                self.estado.texto_error = Some(motivo.texto());
                self.estado.proximo_intento_ms = Some(ahora_ms + espera.as_millis() as i64);
                log::warn!("Fuente {}: {motivo:?}; reintento en {} s", self.fuente.id(), espera.as_secs());
                None
            }
        }
    }

    /// Solo con la última lectura buena: un dato viejo no dispara avisos.
    pub fn usos(&self, cfg: &Config) -> Vec<Uso> {
        match (&self.estado.datos, self.estado.error) {
            (Some(d), None) => self.fuente.usos(d, cfg),
            _ => Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct EstadoServicios {
    ultimos: Mutex<HashMap<&'static str, EstadoFuente>>,
    forzados: Mutex<HashSet<String>>,
    widgets: Mutex<HashMap<&'static str, &'static str>>,
}

impl EstadoServicios {
    pub fn ultimo(&self, fuente: &str) -> Option<EstadoFuente> {
        self.ultimos.lock().get(fuente).cloned()
    }

    pub fn forzar(&self, fuente: &str) {
        self.forzados.lock().insert(fuente.to_string());
    }

    fn registrar(&self, fuente: &'static str, widget: &'static str) {
        self.widgets.lock().insert(fuente, widget);
    }

    /// Al encender un widget sus fuentes leen de inmediato en vez de esperar el intervalo completo.
    pub fn forzar_widget(&self, widget: &str) {
        let fuentes: Vec<&str> = self.widgets.lock().iter().filter(|(_, w)| **w == widget).map(|(f, _)| *f).collect();
        for f in fuentes {
            self.forzar(f);
        }
    }
}

/// Cada fuente corre en su propio hilo: una que falla o tarda no frena a las demás (GD-NFR-REL-01).
pub fn iniciar(app: &AppHandle, fuentes: Vec<Box<dyn Fuente>>) {
    for f in fuentes {
        let app = app.clone();
        let nombre = format!("glassdeck-{}", f.id());
        std::thread::Builder::new().name(nombre).spawn(move || correr(app, f)).ok();
    }
}

fn widget_activo(app: &AppHandle, widget: &str) -> bool {
    app.state::<AlmacenConfig>().actual.lock().widgets.get(widget).map(|w| w.activo).unwrap_or(false)
}

fn correr(app: AppHandle, fuente: Box<dyn Fuente>) {
    let (id, widget, persistente) = (fuente.id(), fuente.widget(), fuente.persistente());
    let previo = if persistente { app.state::<AlmacenEstado>().ultimo(id) } else { None };
    let mut e = Ejecutor::nuevo(fuente, previo);
    app.state::<EstadoServicios>().registrar(id, widget);
    app.state::<EstadoServicios>().ultimos.lock().insert(id, e.estado.clone());
    loop {
        std::thread::sleep(TIC);
        let servicios = app.state::<EstadoServicios>();
        if servicios.forzados.lock().remove(id) {
            e.forzar();
        }
        if app.state::<EstadoRecoleccion>().pausado.load(Ordering::Relaxed) || !widget_activo(&app, widget) {
            continue;
        }
        let ahora = Instant::now();
        if !e.vencido(ahora) {
            continue;
        }
        let cfg = app.state::<AlmacenConfig>().leer();
        let error_previo = e.estado.error;
        if let Some(dato) = e.ejecutar(&cfg, ahora, ahora_ms()) {
            if let Err(err) = app.state::<AlmacenEstado>().guardar_ultimo(id, dato) {
                log::warn!("{err}");
            }
        }
        avisos::procesar(&app, &e.usos(&cfg));
        servicios.ultimos.lock().insert(id, e.estado.clone());
        if e.estado.error != error_previo {
            let _ = app.emit(crate::recolectores::web::EVENTO_CUENTAS, ());
        }
        let _ = app.emit_to(catalogo::etiqueta(widget), "gd://datos", LecturaServicio { fuente: id, datos: e.estado.clone() });
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    pub struct Simulada {
        pub id: &'static str,
        pub respuestas: Vec<Result<serde_json::Value, Motivo>>,
        pub persistente: bool,
    }

    impl Fuente for Simulada {
        fn id(&self) -> &'static str {
            self.id
        }
        fn widget(&self) -> &'static str {
            "spending"
        }
        fn intervalo(&self, _: &Config) -> Duration {
            Duration::from_secs(1_800)
        }
        fn persistente(&self) -> bool {
            self.persistente
        }
        fn recolectar(&mut self, _: &Config) -> Result<serde_json::Value, Motivo> {
            if self.respuestas.is_empty() {
                Err(Motivo::Interno)
            } else {
                self.respuestas.remove(0)
            }
        }
        fn usos(&self, datos: &serde_json::Value, cfg: &Config) -> Vec<Uso> {
            let gastado = datos.get("gastado_usd").and_then(|v| v.as_f64()).unwrap_or_default();
            avisos::porcentaje_cursor(gastado, Some(20.0), cfg.umbrales.presupuesto_cursor_usd)
                .map(|porcentaje| vec![Uso { medidor: avisos::Medidor::Cursor, ciclo: "2026-10-01".into(), porcentaje }])
                .unwrap_or_default()
        }
    }

    #[test]
    fn las_lecturas_de_gasto_alimentan_al_motor_de_avisos_solo_si_son_frescas() {
        let mut cfg = Config::default();
        let mut e = ejecutor("cursor", vec![Ok(serde_json::json!({ "gastado_usd": 17.0 })), Err(Motivo::SinConexion)]);
        let t = Instant::now();
        e.ejecutar(&cfg, t, 0);
        let usos = e.usos(&cfg);
        assert_eq!(usos[0].porcentaje, 85.0);
        let avisos = avisos::evaluar(&usos, &cfg.umbrales, &Default::default());
        assert_eq!(avisos.iter().map(|a| a.umbral).collect::<Vec<_>>(), vec![80]);
        cfg.umbrales.presupuesto_cursor_usd = Some(34.0);
        assert_eq!(e.usos(&cfg)[0].porcentaje, 50.0);
        e.ejecutar(&cfg, t, 1);
        assert!(e.usos(&cfg).is_empty(), "con la lectura fallida no se reevalúa un dato viejo");
    }

    fn ejecutor(id: &'static str, respuestas: Vec<Result<serde_json::Value, Motivo>>) -> Ejecutor {
        Ejecutor::nuevo(Box::new(Simulada { id, respuestas, persistente: true }), None)
    }

    #[test]
    fn una_fuente_que_falla_no_detiene_a_las_demas() {
        let cfg = Config::default();
        let mut fuentes = vec![
            ejecutor("a", vec![Ok(serde_json::json!(1)), Ok(serde_json::json!(2))]),
            ejecutor("rota", vec![Err(Motivo::SinConexion), Err(Motivo::SinConexion)]),
            ejecutor("c", vec![Ok(serde_json::json!("x")), Ok(serde_json::json!("y"))]),
        ];
        let t0 = Instant::now();
        for e in fuentes.iter_mut() {
            e.ejecutar(&cfg, t0, 0);
        }
        assert_eq!(fuentes[0].estado.datos, Some(serde_json::json!(1)));
        assert_eq!(fuentes[1].estado.error, Some(Motivo::SinConexion));
        assert_eq!(fuentes[2].estado.datos, Some(serde_json::json!("x")));
        let t1 = t0 + Duration::from_secs(1_800);
        for e in fuentes.iter_mut().filter(|e| e.vencido(t1)) {
            e.ejecutar(&cfg, t1, 1);
        }
        assert_eq!(fuentes[0].estado.datos, Some(serde_json::json!(2)));
        assert_eq!(fuentes[2].estado.datos, Some(serde_json::json!("y")));
    }

    #[test]
    fn al_fallar_conserva_el_ultimo_dato_con_su_hora_y_programa_el_reintento() {
        let cfg = Config::default();
        let previo = UltimoDato { hora_ms: 500, datos: serde_json::json!({ "t": 20 }) };
        let fuente = Simulada { id: "clima", respuestas: vec![Err(Motivo::SinConexion)], persistente: true };
        let mut e = Ejecutor::nuevo(Box::new(fuente), Some(previo));
        let t0 = Instant::now();
        assert!(e.ejecutar(&cfg, t0, 10_000).is_none());
        assert_eq!(e.estado.datos, Some(serde_json::json!({ "t": 20 })));
        assert_eq!(e.estado.hora_ms, Some(500));
        assert_eq!(e.estado.texto_error, Some("Sin conexión"));
        assert_eq!(e.estado.proximo_intento_ms, Some(10_000 + 60_000));
        assert!(!e.vencido(t0 + Duration::from_secs(59)));
        assert!(e.vencido(t0 + Duration::from_secs(60)));
    }

    #[test]
    fn las_esperas_crecen_y_un_exito_las_reinicia_y_limpia_el_error() {
        let cfg = Config::default();
        let mut e = ejecutor(
            "x",
            vec![Err(Motivo::SinConexion), Err(Motivo::SinConexion), Err(Motivo::SinConexion), Ok(serde_json::json!(7)), Err(Motivo::SinConexion)],
        );
        let t = Instant::now();
        let mut esperas = Vec::new();
        for _ in 0..3 {
            e.ejecutar(&cfg, t, 0);
            esperas.push(e.estado.proximo_intento_ms.unwrap() / 60_000);
        }
        assert_eq!(esperas, vec![1, 2, 5]);
        assert!(e.ejecutar(&cfg, t, 0).is_some());
        assert_eq!(e.estado.error, None);
        e.ejecutar(&cfg, t, 0);
        assert_eq!(e.estado.proximo_intento_ms, Some(60_000));
    }

    #[test]
    fn encender_un_widget_fuerza_solo_sus_fuentes() {
        let s = EstadoServicios::default();
        s.registrar("claude_code", "spending");
        s.registrar("cursor", "spending");
        s.registrar("clima", "clima");
        s.forzar_widget("spending");
        let mut forzados: Vec<String> = s.forzados.lock().iter().cloned().collect();
        forzados.sort();
        assert_eq!(forzados, vec!["claude_code", "cursor"]);
    }

    #[test]
    fn forzar_adelanta_la_siguiente_lectura() {
        let cfg = Config::default();
        let mut e = ejecutor("x", vec![Ok(serde_json::json!(1))]);
        let t = Instant::now();
        e.ejecutar(&cfg, t, 0);
        assert!(!e.vencido(t));
        e.forzar();
        assert!(e.vencido(t));
    }
}

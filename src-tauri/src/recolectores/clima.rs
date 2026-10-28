use super::servicios::{Fuente, Motivo};
use crate::conexion::Transporte;
use crate::config::{Config, ModoUbicacion, Ubicacion};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

const URL_PRONOSTICO: &str = "https://api.open-meteo.com/v1/forecast";
const URL_GEOCODIFICACION: &str = "https://geocoding-api.open-meteo.com/v1/search";
const URL_IPAPI: &str = "https://ipapi.co/json/";
/// La ubicación por IP casi nunca cambia; se pide de nuevo cada 6 h para no gastar el cupo gratuito de ipapi.
const VIGENCIA_IP: Duration = Duration::from_secs(6 * 3600);
const HORAS: usize = 24;
const DIAS: usize = 7;
pub const MAX_RESULTADOS: usize = 8;
pub const LARGO_BUSQUEDA: std::ops::RangeInclusive<usize> = 2..=80;

#[derive(Deserialize)]
struct RespIpapi {
    #[serde(default)]
    error: bool,
    city: Option<String>,
    region: Option<String>,
    country_name: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Deserialize)]
struct RespGeo {
    #[serde(default)]
    results: Vec<ResultadoGeo>,
}

#[derive(Deserialize)]
struct ResultadoGeo {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

#[derive(Deserialize)]
struct RespPronostico {
    timezone: Option<String>,
    current: RespActual,
    hourly: RespHorario,
    daily: RespDiario,
}

#[derive(Deserialize)]
struct RespActual {
    time: String,
    temperature_2m: f64,
    apparent_temperature: Option<f64>,
    relative_humidity_2m: Option<f64>,
    weather_code: Option<u8>,
    is_day: Option<u8>,
    wind_speed_10m: Option<f64>,
}

#[derive(Deserialize)]
struct RespHorario {
    time: Vec<String>,
    temperature_2m: Vec<Option<f64>>,
    weather_code: Vec<Option<u8>>,
    #[serde(default)]
    precipitation_probability: Vec<Option<f64>>,
    #[serde(default)]
    is_day: Vec<Option<u8>>,
}

#[derive(Deserialize)]
struct RespDiario {
    time: Vec<String>,
    weather_code: Vec<Option<u8>>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<f64>>,
}

/// Lo que ve el widget: sin coordenadas, solo el nombre del lugar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lugar {
    pub nombre: String,
    pub region: Option<String>,
    pub pais: Option<String>,
    pub automatica: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClimaActual {
    pub hora: String,
    pub temperatura: f64,
    pub sensacion: Option<f64>,
    pub humedad: Option<f64>,
    pub viento_kmh: Option<f64>,
    pub codigo: Option<u8>,
    pub icono: &'static str,
    pub descripcion: &'static str,
    pub es_dia: bool,
    pub maxima: Option<f64>,
    pub minima: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoraClima {
    pub hora: String,
    pub temperatura: Option<f64>,
    pub codigo: Option<u8>,
    pub icono: &'static str,
    pub prob_lluvia: Option<f64>,
    pub es_dia: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiaClima {
    pub fecha: String,
    pub maxima: Option<f64>,
    pub minima: Option<f64>,
    pub codigo: Option<u8>,
    pub icono: &'static str,
    pub descripcion: &'static str,
    pub prob_lluvia: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Clima {
    pub lugar: Lugar,
    pub zona_horaria: Option<String>,
    pub actual: ClimaActual,
    pub horas: Vec<HoraClima>,
    pub dias: Vec<DiaClima>,
}

/// Códigos WMO de Open-Meteo → (clave de icono, descripción).
pub fn describir_codigo(codigo: Option<u8>) -> (&'static str, &'static str) {
    match codigo {
        Some(0) => ("despejado", "Despejado"),
        Some(1) => ("parcial", "Mayormente despejado"),
        Some(2) => ("parcial", "Parcialmente nublado"),
        Some(3) => ("nublado", "Nublado"),
        Some(45 | 48) => ("niebla", "Niebla"),
        Some(51 | 53 | 55) => ("llovizna", "Llovizna"),
        Some(56 | 57) => ("llovizna", "Llovizna helada"),
        Some(61) => ("lluvia", "Lluvia ligera"),
        Some(63) => ("lluvia", "Lluvia"),
        Some(65) => ("lluvia", "Lluvia fuerte"),
        Some(66 | 67) => ("lluvia", "Lluvia helada"),
        Some(71 | 73 | 75 | 77) => ("nieve", "Nieve"),
        Some(80 | 81 | 82) => ("chubascos", "Chubascos"),
        Some(85 | 86) => ("nieve", "Chubascos de nieve"),
        Some(95) => ("tormenta", "Tormenta"),
        Some(96 | 99) => ("tormenta", "Tormenta con granizo"),
        _ => ("desconocido", "Sin dato"),
    }
}

fn en<T: Copy>(v: &[Option<T>], i: usize) -> Option<T> {
    v.get(i).copied().flatten()
}

fn armar(r: RespPronostico, lugar: Lugar) -> Result<Clima, Motivo> {
    let desde = format!("{}:00", r.current.time.get(..13).unwrap_or(&r.current.time));
    let h = &r.hourly;
    let horas: Vec<HoraClima> = h
        .time
        .iter()
        .enumerate()
        .filter(|(_, t)| t.as_str() >= desde.as_str())
        .take(HORAS)
        .map(|(i, t)| {
            let codigo = en(&h.weather_code, i);
            HoraClima {
                hora: t.clone(),
                temperatura: en(&h.temperature_2m, i),
                codigo,
                icono: describir_codigo(codigo).0,
                prob_lluvia: en(&h.precipitation_probability, i),
                es_dia: en(&h.is_day, i).map(|d| d == 1).unwrap_or(true),
            }
        })
        .collect();
    let d = &r.daily;
    let dias: Vec<DiaClima> = d
        .time
        .iter()
        .take(DIAS)
        .enumerate()
        .map(|(i, f)| {
            let codigo = en(&d.weather_code, i);
            let (icono, descripcion) = describir_codigo(codigo);
            DiaClima {
                fecha: f.clone(),
                maxima: en(&d.temperature_2m_max, i),
                minima: en(&d.temperature_2m_min, i),
                codigo,
                icono,
                descripcion,
                prob_lluvia: en(&d.precipitation_probability_max, i),
            }
        })
        .collect();
    if horas.is_empty() || dias.is_empty() || !r.current.temperature_2m.is_finite() {
        return Err(Motivo::FormatoCambio);
    }
    let a = r.current;
    let (icono, descripcion) = describir_codigo(a.weather_code);
    let actual = ClimaActual {
        hora: a.time,
        temperatura: a.temperature_2m,
        sensacion: a.apparent_temperature,
        humedad: a.relative_humidity_2m,
        viento_kmh: a.wind_speed_10m,
        codigo: a.weather_code,
        icono,
        descripcion,
        es_dia: a.is_day.map(|d| d == 1).unwrap_or(true),
        maxima: dias[0].maxima,
        minima: dias[0].minima,
    };
    Ok(Clima { lugar, zona_horaria: r.timezone, actual, horas, dias })
}

fn de_ipapi(r: RespIpapi) -> Result<Ubicacion, Motivo> {
    if r.error {
        return Err(Motivo::ServicioNoDisponible);
    }
    let (Some(nombre), Some(latitud), Some(longitud)) = (r.city, r.latitude, r.longitude) else {
        return Err(Motivo::SinUbicacion);
    };
    let u = Ubicacion { nombre, region: r.region, pais: r.country_name, latitud, longitud };
    u.validar().map(|_| u).map_err(|_| Motivo::SinUbicacion)
}

fn de_geocodificacion(r: RespGeo) -> Vec<Ubicacion> {
    r.results
        .into_iter()
        .map(|g| Ubicacion { nombre: g.name, region: g.admin1, pais: g.country, latitud: g.latitude, longitud: g.longitude })
        .filter(|u| u.validar().is_ok())
        .take(MAX_RESULTADOS)
        .collect()
}

fn leer<T: for<'de> Deserialize<'de>>(v: serde_json::Value) -> Result<T, Motivo> {
    serde_json::from_value(v).map_err(|_| Motivo::FormatoCambio)
}

fn consulta_pronostico(u: &Ubicacion) -> Vec<(&'static str, String)> {
    vec![
        ("latitude", format!("{:.4}", u.latitud)),
        ("longitude", format!("{:.4}", u.longitud)),
        ("current", "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m".into()),
        ("hourly", "temperature_2m,weather_code,precipitation_probability,is_day".into()),
        ("daily", "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max".into()),
        ("timezone", "auto".into()),
        ("forecast_days", DIAS.to_string()),
        ("forecast_hours", HORAS.to_string()),
    ]
}

/// GD-CUST-08: el texto de búsqueda no se registra en los logs.
pub fn buscar_ciudad(red: &dyn Transporte, texto: &str) -> Result<Vec<Ubicacion>, Motivo> {
    let t = texto.trim();
    if !LARGO_BUSQUEDA.contains(&t.chars().count()) {
        return Ok(Vec::new());
    }
    let consulta =
        [("name", t.to_string()), ("count", MAX_RESULTADOS.to_string()), ("language", "es".into()), ("format", "json".into())];
    Ok(de_geocodificacion(leer(red.obtener(URL_GEOCODIFICACION, &consulta)?)?))
}

pub struct FuenteClima {
    red: Arc<dyn Transporte>,
    por_ip: Option<(Ubicacion, Instant)>,
}

impl FuenteClima {
    pub fn nueva(red: Arc<dyn Transporte>) -> Self {
        Self { red, por_ip: None }
    }

    /// ipapi solo se consulta en modo automático; si falla, sirve la última ubicación por IP aunque haya vencido.
    fn ubicacion(&mut self, cfg: &Config) -> Result<(Ubicacion, bool), Motivo> {
        if let (ModoUbicacion::Manual, Some(u)) = (cfg.clima.modo, &cfg.clima.ubicacion) {
            return Ok((u.clone(), false));
        }
        if let Some((u, t)) = &self.por_ip {
            if t.elapsed() < VIGENCIA_IP {
                return Ok((u.clone(), true));
            }
        }
        match self.red.obtener(URL_IPAPI, &[]).map_err(Motivo::from).and_then(leer::<RespIpapi>).and_then(de_ipapi) {
            Ok(u) => {
                self.por_ip = Some((u.clone(), Instant::now()));
                Ok((u, true))
            }
            Err(m) => self.por_ip.as_ref().map(|(u, _)| (u.clone(), true)).ok_or(m),
        }
    }
}

impl Fuente for FuenteClima {
    fn id(&self) -> &'static str {
        "clima"
    }
    fn widget(&self) -> &'static str {
        "clima"
    }
    fn intervalo(&self, cfg: &Config) -> Duration {
        Duration::from_millis(cfg.intervalos.clima_ms)
    }
    fn persistente(&self) -> bool {
        true
    }
    fn recolectar(&mut self, cfg: &Config) -> Result<serde_json::Value, Motivo> {
        let (u, automatica) = self.ubicacion(cfg)?;
        let r = leer(self.red.obtener(URL_PRONOSTICO, &consulta_pronostico(&u))?)?;
        let lugar = Lugar { nombre: u.nombre, region: u.region, pais: u.pais, automatica };
        serde_json::to_value(armar(r, lugar)?).map_err(|_| Motivo::Interno)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexion::{validar_url, ErrorRed};
    use crate::config::estado::UltimoDato;
    use crate::config::ConfigClima;
    use crate::recolectores::servicios::Ejecutor;
    use parking_lot::Mutex;
    use std::collections::HashMap;

    const PRONOSTICO: &str = include_str!("grabaciones/pronostico_chihuahua.json");
    const GEOCODIFICACION: &str = include_str!("grabaciones/geocodificacion_chihuahua.json");
    const IPAPI: &str = include_str!("grabaciones/ipapi_sintetica.json");

    /// Responde por dominio con JSON grabado y anota cada URL pedida; nunca abre la red.
    #[derive(Default)]
    struct Grabada {
        respuestas: HashMap<&'static str, Result<serde_json::Value, ErrorRed>>,
        llamadas: Mutex<Vec<reqwest::Url>>,
    }

    impl Grabada {
        fn con(mut self, host: &'static str, r: Result<&str, ErrorRed>) -> Self {
            self.respuestas.insert(host, r.map(|t| serde_json::from_str(t).unwrap()));
            self
        }
        fn hosts(&self) -> Vec<String> {
            self.llamadas.lock().iter().map(|u| u.host_str().unwrap().to_string()).collect()
        }
    }

    impl Transporte for Grabada {
        fn obtener(&self, url: &str, consulta: &[(&str, String)]) -> Result<serde_json::Value, ErrorRed> {
            let u = validar_url(url, consulta)?;
            self.llamadas.lock().push(u.clone());
            self.respuestas.get(u.host_str().unwrap()).cloned().unwrap_or(Err(ErrorRed::SinConexion))
        }
    }

    fn chihuahua() -> Ubicacion {
        Ubicacion { nombre: "Chihuahua".into(), region: Some("Estado de Chihuahua".into()), pais: Some("México".into()), latitud: 28.63528, longitud: -106.08889 }
    }

    fn manual() -> Config {
        Config { clima: ConfigClima { modo: ModoUbicacion::Manual, ubicacion: Some(chihuahua()) }, ..Config::default() }
    }

    fn lugar() -> Lugar {
        Lugar { nombre: "Chihuahua".into(), region: None, pais: None, automatica: false }
    }

    #[test]
    fn el_pronostico_grabado_da_actual_24_horas_y_7_dias() {
        let c = armar(serde_json::from_str(PRONOSTICO).unwrap(), lugar()).unwrap();
        assert_eq!(c.actual.temperatura, 20.5);
        assert_eq!((c.actual.icono, c.actual.descripcion), ("nublado", "Nublado"));
        assert_eq!((c.actual.maxima, c.actual.minima), (Some(24.7), Some(14.5)));
        assert_eq!(c.actual.humedad, Some(67.0));
        assert!(c.actual.es_dia);
        assert_eq!(c.horas.len(), 24);
        assert_eq!(c.horas[0].hora, "2026-10-28T10:00");
        assert_eq!(c.dias.len(), 7);
        assert_eq!(c.dias[1].prob_lluvia, Some(79.0));
        assert_eq!(c.dias[6].icono, "despejado");
        assert_eq!(c.zona_horaria.as_deref(), Some("America/Chihuahua"));
    }

    #[test]
    fn las_horas_pasadas_se_descartan() {
        let mut v: serde_json::Value = serde_json::from_str(PRONOSTICO).unwrap();
        v["current"]["time"] = "2026-10-28T15:45".into();
        let c = armar(serde_json::from_value(v).unwrap(), lugar()).unwrap();
        assert_eq!(c.horas[0].hora, "2026-10-28T15:00");
        assert_eq!(c.horas.len(), 19);
    }

    #[test]
    fn un_formato_distinto_se_reporta_como_formato_cambio() {
        let mut v: serde_json::Value = serde_json::from_str(PRONOSTICO).unwrap();
        v.as_object_mut().unwrap().remove("hourly");
        let red = Grabada::default().con("api.open-meteo.com", Ok(&v.to_string()));
        let mut f = FuenteClima::nueva(Arc::new(red));
        assert_eq!(f.recolectar(&manual()), Err(Motivo::FormatoCambio));

        let mut v: serde_json::Value = serde_json::from_str(PRONOSTICO).unwrap();
        v["daily"]["time"] = serde_json::json!([]);
        assert_eq!(armar(serde_json::from_value(v).unwrap(), lugar()), Err(Motivo::FormatoCambio));
    }

    #[test]
    fn la_geocodificacion_grabada_encuentra_chihuahua() {
        let red = Grabada::default().con("geocoding-api.open-meteo.com", Ok(GEOCODIFICACION));
        let r = buscar_ciudad(&red, "  Chihuahua ").unwrap();
        assert!(!r.is_empty() && r.len() <= MAX_RESULTADOS);
        assert_eq!(r[0].nombre, "Chihuahua");
        assert_eq!(r[0].pais.as_deref(), Some("México"));
        assert_eq!((r[0].latitud, r[0].longitud), (28.63528, -106.08889));
        let q = red.llamadas.lock()[0].query().unwrap().to_string();
        assert!(q.contains("name=Chihuahua") && q.contains("language=es"), "{q}");
    }

    #[test]
    fn una_busqueda_muy_corta_o_larga_no_sale_a_la_red() {
        let red = Grabada::default();
        assert!(buscar_ciudad(&red, " a ").unwrap().is_empty());
        assert!(buscar_ciudad(&red, &"x".repeat(81)).unwrap().is_empty());
        assert!(red.hosts().is_empty());
    }

    #[test]
    fn con_ciudad_fijada_el_clima_es_de_esa_ciudad_y_no_se_consulta_ipapi() {
        let red = Arc::new(Grabada::default().con("api.open-meteo.com", Ok(PRONOSTICO)).con("ipapi.co", Ok(IPAPI)));
        let mut f = FuenteClima::nueva(red.clone());
        let v = f.recolectar(&manual()).unwrap();
        assert_eq!(v["lugar"]["nombre"], "Chihuahua");
        assert_eq!(v["lugar"]["automatica"], false);
        assert!(v["lugar"].get("latitud").is_none(), "las coordenadas no van al widget ni a state.json");
        assert_eq!(red.hosts(), vec!["api.open-meteo.com"]);
        let q = red.llamadas.lock()[0].query().unwrap().to_string();
        assert!(q.contains("latitude=28.6353") && q.contains("longitude=-106.0889"), "{q}");
    }

    #[test]
    fn en_modo_automatico_se_ubica_por_ip_una_sola_vez() {
        let red = Arc::new(Grabada::default().con("api.open-meteo.com", Ok(PRONOSTICO)).con("ipapi.co", Ok(IPAPI)));
        let mut f = FuenteClima::nueva(red.clone());
        let cfg = Config::default();
        let v = f.recolectar(&cfg).unwrap();
        f.recolectar(&cfg).unwrap();
        assert_eq!(v["lugar"]["nombre"], "Chihuahua");
        assert_eq!(v["lugar"]["automatica"], true);
        assert_eq!(red.hosts(), vec!["ipapi.co", "api.open-meteo.com", "api.open-meteo.com"]);
    }

    #[test]
    fn los_errores_de_ipapi_se_traducen_a_motivos() {
        let error = Grabada::default().con("ipapi.co", Ok(r#"{"error": true, "reason": "RateLimited"}"#));
        assert_eq!(FuenteClima::nueva(Arc::new(error)).recolectar(&Config::default()), Err(Motivo::ServicioNoDisponible));
        let sin_coordenadas = Grabada::default().con("ipapi.co", Ok(r#"{"city": "Chihuahua"}"#));
        assert_eq!(FuenteClima::nueva(Arc::new(sin_coordenadas)).recolectar(&Config::default()), Err(Motivo::SinUbicacion));
        let sin_red = Grabada::default();
        assert_eq!(FuenteClima::nueva(Arc::new(sin_red)).recolectar(&Config::default()), Err(Motivo::SinConexion));
    }

    #[test]
    fn sin_red_queda_el_ultimo_dato_con_su_hora() {
        let previo = UltimoDato { hora_ms: 1_000, datos: serde_json::json!({ "actual": { "temperatura": 18.0 } }) };
        let mut e = Ejecutor::nuevo(Box::new(FuenteClima::nueva(Arc::new(Grabada::default()))), Some(previo));
        assert!(e.ejecutar(&manual(), Instant::now(), 9_000).is_none());
        assert_eq!(e.estado.datos.as_ref().unwrap()["actual"]["temperatura"], 18.0);
        assert_eq!(e.estado.hora_ms, Some(1_000));
        assert_eq!(e.estado.texto_error, Some("Sin conexión"));
    }

    #[test]
    fn una_lectura_buena_se_guarda_para_la_siguiente_vez_sin_red() {
        let red = Grabada::default().con("api.open-meteo.com", Ok(PRONOSTICO));
        let mut e = Ejecutor::nuevo(Box::new(FuenteClima::nueva(Arc::new(red))), None);
        let dato = e.ejecutar(&manual(), Instant::now(), 5_000).expect("el clima es persistente");
        assert_eq!(dato.hora_ms, 5_000);
        assert_eq!(dato.datos["actual"]["temperatura"], 20.5);
    }

    #[test]
    fn todos_los_codigos_wmo_tienen_icono() {
        for c in [0u8, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82, 85, 86, 95, 96, 99] {
            assert_ne!(describir_codigo(Some(c)).0, "desconocido", "{c}");
        }
        assert_eq!(describir_codigo(Some(42)).0, "desconocido");
        assert_eq!(describir_codigo(None).0, "desconocido");
    }
}

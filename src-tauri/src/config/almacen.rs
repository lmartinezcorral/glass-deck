use super::Config;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct AlmacenConfig {
    pub ruta: PathBuf,
    pub actual: Mutex<Config>,
}

impl AlmacenConfig {
    pub fn nuevo(dir: &Path) -> Self {
        let ruta = dir.join("settings.json");
        let actual = cargar(&ruta);
        Self { ruta, actual: Mutex::new(actual) }
    }

    pub fn leer(&self) -> Config {
        self.actual.lock().clone()
    }

    pub fn modificar<F: FnOnce(&mut Config)>(&self, f: F) -> Result<Config, String> {
        let mut c = self.actual.lock();
        f(&mut c);
        c.normalizar();
        guardar(&self.ruta, &*c).map_err(|e| format!("No se pudo guardar la configuración: {e}"))?;
        Ok(c.clone())
    }

    pub fn reemplazar(&self, nueva: Config) -> Result<Config, String> {
        nueva.intervalos.validar()?;
        self.modificar(|c| *c = nueva)
    }
}

fn ruta_respaldo(ruta: &Path) -> PathBuf {
    ruta.with_extension("json.bak")
}

fn leer_json<T: DeserializeOwned>(ruta: &Path) -> Option<T> {
    let texto = fs::read_to_string(ruta).ok()?;
    serde_json::from_str::<T>(texto.trim_start_matches('\u{feff}')).ok()
}

/// Si el archivo principal está dañado se guarda una copia `.corrupto` y se intenta el respaldo.
pub fn cargar_json<T: DeserializeOwned>(ruta: &Path) -> Option<T> {
    if let Some(v) = leer_json(ruta) {
        return Some(v);
    }
    if ruta.exists() {
        let nombre = ruta.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        log::warn!("{nombre} inválido; se guarda copia y se intenta el respaldo");
        let _ = fs::copy(ruta, ruta.with_extension("json.corrupto"));
    }
    leer_json(&ruta_respaldo(ruta))
}

pub fn cargar(ruta: &Path) -> Config {
    let mut c: Config = cargar_json(ruta).unwrap_or_default();
    c.normalizar();
    c
}

pub fn guardar<T: Serialize>(ruta: &Path, c: &T) -> std::io::Result<()> {
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir)?;
    }
    let datos = serde_json::to_vec_pretty(c).map_err(std::io::Error::other)?;
    let tmp = ruta.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&datos)?;
        f.sync_all()?;
    }
    if ruta.exists() {
        let _ = fs::copy(ruta, ruta_respaldo(ruta));
    }
    fs::rename(&tmp, ruta)
}

#[cfg(test)]
pub(crate) fn dir_temporal(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("glassdeck-prueba-{nombre}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn guardar_y_cargar_conserva_los_datos() {
        let d = dir_temporal("guardar");
        let ruta = d.join("settings.json");
        let mut c = Config::default();
        c.iman = true;
        guardar(&ruta, &c).unwrap();
        assert!(cargar(&ruta).iman);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn archivo_corrupto_usa_el_respaldo() {
        let d = dir_temporal("corrupto");
        let ruta = d.join("settings.json");
        let mut c = Config::default();
        c.iman = true;
        guardar(&ruta, &c).unwrap();
        guardar(&ruta, &c).unwrap();
        fs::write(&ruta, b"{ esto no es json").unwrap();
        assert!(cargar(&ruta).iman);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn opciones_generales_sobreviven_guardar_y_leer() {
        use crate::config::Capa;
        let d = dir_temporal("generales");
        let ruta = d.join("settings.json");
        let defecto = Config::default();
        assert!(defecto.arranque_windows);
        assert_eq!(defecto.capa_defecto, Capa::Escritorio);
        assert!(defecto.pausar_con_bateria);
        let mut c = defecto;
        c.arranque_windows = false;
        c.capa_defecto = Capa::SiempreEncima;
        c.pausar_con_bateria = false;
        guardar(&ruta, &c).unwrap();
        let leida = cargar(&ruta);
        assert!(!leida.arranque_windows);
        assert_eq!(leida.capa_defecto, Capa::SiempreEncima);
        assert!(!leida.pausar_con_bateria);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn widgets_conservan_posicion_tamano_estado_pin_y_monitor() {
        use crate::config::EstadoWidget;
        let d = dir_temporal("widgets");
        let ruta = d.join("settings.json");
        let mut c = Config::default();
        let w = c.widgets.get_mut("cpu").unwrap();
        w.x = Some(-1700);
        w.y = Some(40);
        w.ancho = 300;
        w.alto = 260;
        w.estado = EstadoWidget::Pildora;
        w.fijado = true;
        w.monitor = Some(r"\\.\DISPLAY3".into());
        let esperado = w.clone();
        guardar(&ruta, &c).unwrap();
        assert_eq!(cargar(&ruta).widget("cpu"), esperado);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn falla_entre_temporal_y_renombrado_deja_el_original() {
        let d = dir_temporal("falla");
        let ruta = d.join("settings.json");
        let mut c = Config::default();
        c.iman = true;
        guardar(&ruta, &c).unwrap();
        guardar(&ruta, &c).unwrap();
        fs::write(ruta.with_extension("json.tmp"), b"{ escritura a me").unwrap();
        assert!(cargar(&ruta).iman);
        assert!(leer_json::<Config>(&ruta_respaldo(&ruta)).is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn acepta_archivo_con_bom_y_guarda_copia_del_corrupto() {
        let d = dir_temporal("bom");
        let ruta = d.join("settings.json");
        let mut c = Config::default();
        c.arranque_windows = false;
        let mut datos = vec![0xEF, 0xBB, 0xBF];
        datos.extend(serde_json::to_vec(&c).unwrap());
        fs::write(&ruta, datos).unwrap();
        assert!(!cargar(&ruta).arranque_windows);
        fs::write(&ruta, b"{ roto").unwrap();
        cargar(&ruta);
        assert!(ruta.with_extension("json.corrupto").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn sin_archivos_usa_valores_por_defecto() {
        let d = dir_temporal("vacio");
        let c = cargar(&d.join("settings.json"));
        assert_eq!(c, {
            let mut x = Config::default();
            x.normalizar();
            x
        });
        let _ = fs::remove_dir_all(&d);
    }
}

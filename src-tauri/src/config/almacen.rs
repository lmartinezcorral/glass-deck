use super::Config;
use parking_lot::Mutex;
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
        guardar(&self.ruta, &c).map_err(|e| format!("No se pudo guardar la configuración: {e}"))?;
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

fn leer_json(ruta: &Path) -> Option<Config> {
    let texto = fs::read_to_string(ruta).ok()?;
    serde_json::from_str::<Config>(&texto).ok()
}

pub fn cargar(ruta: &Path) -> Config {
    let mut c = match leer_json(ruta) {
        Some(c) => c,
        None => {
            if ruta.exists() {
                log::warn!("settings.json inválido; se intenta el respaldo");
            }
            match leer_json(&ruta_respaldo(ruta)) {
                Some(c) => c,
                None => Config::default(),
            }
        }
    };
    c.normalizar();
    c
}

pub fn guardar(ruta: &Path, c: &Config) -> std::io::Result<()> {
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
mod pruebas {
    use super::*;

    fn dir_temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("glassdeck-prueba-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

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

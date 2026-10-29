use super::filtro::enmascarar;
use log::{LevelFilter, Log, Metadata, Record};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const TAMANO_MAX: u64 = 2 * 1024 * 1024;
const ARCHIVOS_MAX: usize = 5;

struct Registro {
    dir: PathBuf,
    tamano_max: u64,
    archivo: Mutex<Option<File>>,
}

impl Registro {
    fn nuevo(dir: &Path, tamano_max: u64) -> Self {
        let _ = fs::create_dir_all(dir);
        Self { dir: dir.to_path_buf(), tamano_max, archivo: Mutex::new(abrir(&dir.join("glassdeck.log"))) }
    }

    fn ruta(&self, n: usize) -> PathBuf {
        if n == 0 {
            self.dir.join("glassdeck.log")
        } else {
            self.dir.join(format!("glassdeck.{n}.log"))
        }
    }

    fn rotar_si_hace_falta(&self, f: &mut Option<File>) {
        let tam = f.as_ref().and_then(|a| a.metadata().ok()).map(|m| m.len()).unwrap_or(0);
        if tam < self.tamano_max {
            return;
        }
        *f = None;
        let _ = fs::remove_file(self.ruta(ARCHIVOS_MAX - 1));
        for n in (0..ARCHIVOS_MAX - 1).rev() {
            let _ = fs::rename(self.ruta(n), self.ruta(n + 1));
        }
        *f = abrir(&self.ruta(0));
    }
}

fn abrir(ruta: &Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(ruta).ok()
}

impl Log for Registro {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.target().starts_with("glassdeck") || metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut guarda = self.archivo.lock();
        self.rotar_si_hace_falta(&mut guarda);
        if let Some(f) = guarda.as_mut() {
            let ahora = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let linea = enmascarar(&format!("{}", record.args()));
            let _ = writeln!(f, "{ahora} {:<5} {}: {linea}", record.level(), record.target());
        }
    }

    fn flush(&self) {
        if let Some(f) = self.archivo.lock().as_mut() {
            let _ = f.flush();
        }
    }
}

pub fn iniciar(dir: &Path) {
    if log::set_boxed_logger(Box::new(Registro::nuevo(dir, TAMANO_MAX))).is_ok() {
        log::set_max_level(if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info });
    }
}

fn describir_panico(info: &std::panic::PanicHookInfo<'_>) -> String {
    let carga = info.payload();
    let mensaje = carga
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| carga.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("sin mensaje");
    let lugar = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "lugar desconocido".into());
    enmascarar(&format!("Pánico en {lugar}: {mensaje}"))
}

/// Un cierre inesperado deja rastro en el log antes del manejo por defecto; con `panic = "abort"` en release
/// el gancho corre igual, justo antes de abortar.
pub fn registrar_panicos() {
    static UNA: std::sync::Once = std::sync::Once::new();
    UNA.call_once(|| {
        let previo = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("{}", describir_panico(info));
            log::logger().flush();
            previo(info);
        }));
    });
}

#[cfg(test)]
struct Captura(Mutex<String>);

#[cfg(test)]
impl Log for Captura {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, r: &Record) {
        self.0.lock().push_str(&format!("{}\n", r.args()));
    }
    fn flush(&self) {}
}

/// `log` admite un solo registrador por proceso: todas las pruebas que revisan los logs comparten este.
#[cfg(test)]
pub(crate) fn captura_de_prueba() -> &'static Mutex<String> {
    static C: std::sync::OnceLock<&'static Captura> = std::sync::OnceLock::new();
    &C.get_or_init(|| {
        let c: &'static Captura = Box::leak(Box::new(Captura(Mutex::new(String::new()))));
        let _ = log::set_logger(c);
        log::set_max_level(LevelFilter::Trace);
        c
    })
    .0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn rota_y_conserva_como_maximo_cinco_archivos() {
        let d = std::env::temp_dir().join(format!("glassdeck-prueba-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let r = Registro::nuevo(&d, 200);
        for i in 0..200 {
            r.log(
                &Record::builder()
                    .args(format_args!("linea de prueba {i} en C:\\Users\\hulkmtz\\x"))
                    .target("glassdeck::prueba")
                    .level(log::Level::Info)
                    .build(),
            );
        }
        r.flush();
        drop(r);
        let archivos: Vec<_> = fs::read_dir(&d).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(archivos.len(), ARCHIVOS_MAX);
        for a in &archivos {
            let texto = fs::read_to_string(a.path()).unwrap();
            assert!(!texto.contains("hulkmtz"));
            assert!(a.metadata().unwrap().len() < 400);
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn un_panico_queda_en_el_log_con_su_lugar_y_sin_datos_sensibles() {
        let captura = captura_de_prueba();
        registrar_panicos();
        let r = std::thread::spawn(|| panic!("pánico de prueba al abrir C:\\Users\\hulkmtz\\x con token=valor-de-prueba")).join();
        assert!(r.is_err());
        let texto = captura.lock().clone();
        let linea = texto.lines().find(|l| l.contains("pánico de prueba")).expect("el pánico se registró");
        assert!(linea.starts_with("Pánico en src") && linea.contains("registro.rs:"), "{linea}");
        assert!(!linea.contains("hulkmtz") && !linea.contains("valor-de-prueba"), "{linea}");
    }
}

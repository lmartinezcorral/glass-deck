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
    archivo: Mutex<Option<File>>,
}

impl Registro {
    fn ruta(&self, n: usize) -> PathBuf {
        if n == 0 {
            self.dir.join("glassdeck.log")
        } else {
            self.dir.join(format!("glassdeck.{n}.log"))
        }
    }

    fn rotar_si_hace_falta(&self, f: &mut Option<File>) {
        let tam = f.as_ref().and_then(|a| a.metadata().ok()).map(|m| m.len()).unwrap_or(0);
        if tam < TAMANO_MAX {
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
    let _ = fs::create_dir_all(dir);
    let registro = Registro { dir: dir.to_path_buf(), archivo: Mutex::new(abrir(&dir.join("glassdeck.log"))) };
    if log::set_boxed_logger(Box::new(registro)).is_ok() {
        log::set_max_level(if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info });
    }
}

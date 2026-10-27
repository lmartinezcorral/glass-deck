#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const ACCION_LIMPIAR_WINDOWS_TEMP: &str = "limpiar-windows-temp";
const CODIGO_ERROR: i32 = -1;
const ANTIGUEDAD_MINIMA: Duration = Duration::from_secs(24 * 3600);
const ATRIBUTO_REPARSE: u32 = 0x400;

fn es_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & ATRIBUTO_REPARSE != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}

fn viejo(meta: &fs::Metadata, ahora: SystemTime) -> bool {
    meta.modified()
        .ok()
        .and_then(|m| ahora.duration_since(m).ok())
        .map(|d| d >= ANTIGUEDAD_MINIMA)
        .unwrap_or(false)
}

fn limpiar(dir: &Path, ahora: SystemTime, liberado: &mut u64) {
    let Ok(entradas) = fs::read_dir(dir) else { return };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let Ok(meta) = fs::symlink_metadata(&ruta) else { continue };
        if es_reparse(&meta) {
            continue;
        }
        if meta.is_dir() {
            limpiar(&ruta, ahora, liberado);
            if viejo(&meta, ahora) {
                let _ = fs::remove_dir(&ruta);
            }
        } else if viejo(&meta, ahora) && fs::remove_file(&ruta).is_ok() {
            *liberado += meta.len();
        }
    }
}

fn windows_temp() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Temp")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 || args[0] != ACCION_LIMPIAR_WINDOWS_TEMP {
        std::process::exit(CODIGO_ERROR);
    }
    let raiz = windows_temp();
    if !raiz.is_dir() {
        std::process::exit(CODIGO_ERROR);
    }
    let mut liberado = 0u64;
    limpiar(&raiz, SystemTime::now(), &mut liberado);
    let mb = (liberado / 1_048_576).min(i32::MAX as u64) as i32;
    std::process::exit(mb);
}

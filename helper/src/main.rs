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

fn accion_valida(args: &[String]) -> bool {
    matches!(args, [unica] if unica == ACCION_LIMPIAR_WINDOWS_TEMP)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !accion_valida(&args) {
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn solo_acepta_la_accion_cerrada() {
        assert!(accion_valida(&args(&["limpiar-windows-temp"])));
        assert!(!accion_valida(&args(&[])));
        assert!(!accion_valida(&args(&["limpiar-windows-temp", "C:\\"])));
        assert!(!accion_valida(&args(&["C:\\Windows\\Temp"])));
        assert!(!accion_valida(&args(&["LIMPIAR-WINDOWS-TEMP"])));
    }

    #[test]
    fn borra_solo_archivos_viejos() {
        let dir = std::env::temp_dir().join(format!("gd-helper-prueba-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let viejo_ruta = dir.join("viejo.tmp");
        let nuevo_ruta = dir.join("nuevo.tmp");
        fs::write(&viejo_ruta, vec![0u8; 2048]).unwrap();
        fs::write(&nuevo_ruta, b"reciente").unwrap();
        let hace_dos_dias = SystemTime::now() - Duration::from_secs(48 * 3600);
        fs::File::options().write(true).open(&viejo_ruta).unwrap().set_modified(hace_dos_dias).unwrap();

        let mut liberado = 0;
        limpiar(&dir, SystemTime::now(), &mut liberado);

        assert!(!viejo_ruta.exists());
        assert!(nuevo_ruta.exists());
        assert_eq!(liberado, 2048);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn no_entra_a_uniones() {
        let dir = std::env::temp_dir().join(format!("gd-helper-union-{}", std::process::id()));
        let destino = std::env::temp_dir().join(format!("gd-helper-destino-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&destino);
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&destino).unwrap();
        let importante = destino.join("importante.txt");
        fs::write(&importante, b"no borrar").unwrap();
        let hace_dos_dias = SystemTime::now() - Duration::from_secs(48 * 3600);
        fs::File::options().write(true).open(&importante).unwrap().set_modified(hace_dos_dias).unwrap();
        let creada = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(dir.join("union"))
            .arg(&destino)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(creada, "mklink /J no requiere permisos de administrador");

        let mut liberado = 0;
        limpiar(&dir, SystemTime::now(), &mut liberado);

        assert!(importante.exists());
        assert_eq!(liberado, 0);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&destino);
    }
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const ANTIGUEDAD_MINIMA: Duration = Duration::from_secs(24 * 3600);
const ATRIBUTO_REPARSE: u32 = 0x400;

#[derive(Debug, Clone, Serialize)]
pub struct Origen {
    pub id: &'static str,
    pub nombre: &'static str,
    pub bytes: u64,
    pub archivos: u64,
    pub requiere_admin: bool,
    pub estimado: bool,
    pub marcado_defecto: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Opciones {
    pub temp_usuario: bool,
    pub temp_windows: bool,
    pub papelera: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Resumen {
    pub liberado: u64,
    pub archivos_borrados: u64,
    pub omitidos: u64,
    pub windows_temp: Option<String>,
    pub papelera: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Conteo {
    pub bytes: u64,
    pub archivos: u64,
    pub omitidos: u64,
}

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

fn suficientemente_viejo(meta: &fs::Metadata, ahora: SystemTime) -> bool {
    meta.modified()
        .ok()
        .and_then(|m| ahora.duration_since(m).ok())
        .map(|d| d >= ANTIGUEDAD_MINIMA)
        .unwrap_or(false)
}

pub fn recorrer(raiz: &Path, borrar: bool, ahora: SystemTime) -> Conteo {
    let mut c = Conteo::default();
    recorrer_dir(raiz, borrar, ahora, &mut c, true);
    c
}

fn recorrer_dir(dir: &Path, borrar: bool, ahora: SystemTime, c: &mut Conteo, es_raiz: bool) {
    let Ok(entradas) = fs::read_dir(dir) else {
        if !es_raiz {
            c.omitidos += 1;
        }
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let Ok(meta) = fs::symlink_metadata(&ruta) else {
            c.omitidos += 1;
            continue;
        };
        if es_reparse(&meta) {
            continue;
        }
        if meta.is_dir() {
            recorrer_dir(&ruta, borrar, ahora, c, false);
            if borrar && suficientemente_viejo(&meta, ahora) {
                let _ = fs::remove_dir(&ruta);
            }
            continue;
        }
        if !suficientemente_viejo(&meta, ahora) {
            continue;
        }
        if borrar {
            match fs::remove_file(&ruta) {
                Ok(()) => {
                    c.bytes += meta.len();
                    c.archivos += 1;
                }
                Err(_) => c.omitidos += 1,
            }
        } else {
            c.bytes += meta.len();
            c.archivos += 1;
        }
    }
}

pub fn temp_usuario() -> PathBuf {
    std::env::temp_dir()
}

pub fn temp_windows() -> PathBuf {
    let raiz = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    raiz.join("Temp")
}

pub fn vista_previa() -> Vec<Origen> {
    let ahora = SystemTime::now();
    let usuario = recorrer(&temp_usuario(), false, ahora);
    let windows = recorrer(&temp_windows(), false, ahora);
    let (papelera_bytes, papelera_items) = papelera::tamano();
    vec![
        Origen {
            id: "temp_usuario",
            nombre: "Temporales del usuario (%TEMP%)",
            bytes: usuario.bytes,
            archivos: usuario.archivos,
            requiere_admin: false,
            estimado: false,
            marcado_defecto: true,
        },
        Origen {
            id: "temp_windows",
            nombre: "Temporales de Windows (C:\\Windows\\Temp)",
            bytes: windows.bytes,
            archivos: windows.archivos,
            requiere_admin: true,
            estimado: true,
            marcado_defecto: true,
        },
        Origen {
            id: "papelera",
            nombre: "Papelera de reciclaje",
            bytes: papelera_bytes,
            archivos: papelera_items,
            requiere_admin: false,
            estimado: false,
            marcado_defecto: false,
        },
    ]
}

pub fn ejecutar(op: Opciones, helper: Option<&Path>) -> Resumen {
    let mut r = Resumen::default();
    if op.temp_usuario {
        let c = recorrer(&temp_usuario(), true, SystemTime::now());
        r.liberado += c.bytes;
        r.archivos_borrados += c.archivos;
        r.omitidos += c.omitidos;
    }
    if op.temp_windows {
        r.windows_temp = Some(match helper {
            Some(h) => match elevado::limpiar_windows_temp(h) {
                Ok(mb) => {
                    r.liberado += mb * 1_048_576;
                    format!("Limpio ({mb} MB)")
                }
                Err(e) => e,
            },
            None => "No se encontró el ayudante de Glassdeck".into(),
        });
    }
    if op.papelera {
        let (bytes, _) = papelera::tamano();
        r.papelera = Some(match papelera::vaciar() {
            Ok(()) => {
                r.liberado += bytes;
                "Vaciada".into()
            }
            Err(e) => e,
        });
    }
    r
}

#[cfg(windows)]
mod papelera {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
        SHQUERYRBINFO,
    };

    pub fn tamano() -> (u64, u64) {
        let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
        match unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) } {
            Ok(()) => (info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64),
            Err(_) => (0, 0),
        }
    }

    pub fn vaciar() -> Result<(), String> {
        let (_, items) = tamano();
        if items == 0 {
            return Ok(());
        }
        unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) }
            .map_err(|e| format!("No se pudo vaciar la papelera ({})", e.code()))
    }
}

#[cfg(not(windows))]
mod papelera {
    pub fn tamano() -> (u64, u64) {
        (0, 0)
    }
    pub fn vaciar() -> Result<(), String> {
        Ok(())
    }
}

#[cfg(windows)]
pub mod elevado {
    use std::path::Path;
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    pub const ACCION_LIMPIAR_WINDOWS_TEMP: &str = "limpiar-windows-temp";
    pub const CODIGO_ERROR: u32 = u32::MAX;

    pub fn limpiar_windows_temp(helper: &Path) -> Result<u64, String> {
        let archivo = HSTRING::from(helper.as_os_str());
        let parametros = HSTRING::from(ACCION_LIMPIAR_WINDOWS_TEMP);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(archivo.as_ptr()),
            lpParameters: PCWSTR(parametros.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
            if e.code() == ERROR_CANCELLED.to_hresult() {
                return Err("Omitido: se canceló el permiso de administrador".into());
            }
            return Err(format!("No se pudo iniciar el ayudante ({})", e.code()));
        }
        if info.hProcess.is_invalid() {
            return Err("El ayudante no devolvió resultado".into());
        }
        let mut codigo = CODIGO_ERROR;
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            let _ = GetExitCodeProcess(info.hProcess, &mut codigo);
            let _ = CloseHandle(info.hProcess);
        }
        if codigo == CODIGO_ERROR {
            return Err("El ayudante terminó con error".into());
        }
        Ok(codigo as u64)
    }
}

#[cfg(not(windows))]
pub mod elevado {
    pub fn limpiar_windows_temp(_helper: &std::path::Path) -> Result<u64, String> {
        Err("Solo disponible en Windows".into())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::Duration;

    fn preparar(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("glassdeck-limpieza-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::write(d.join("viejo.tmp"), vec![0u8; 1000]).unwrap();
        fs::write(d.join("sub").join("viejo2.tmp"), vec![0u8; 500]).unwrap();
        fs::write(d.join("nuevo.tmp"), vec![0u8; 300]).unwrap();
        d
    }

    #[test]
    fn la_vista_previa_solo_cuenta_archivos_viejos_y_no_borra() {
        let d = preparar("vista");
        let futuro = SystemTime::now() + Duration::from_secs(25 * 3600);
        let c = recorrer(&d, false, futuro);
        assert_eq!(c.archivos, 3);
        assert_eq!(c.bytes, 1800);
        let ahora = recorrer(&d, false, SystemTime::now());
        assert_eq!(ahora.archivos, 0);
        assert!(d.join("viejo.tmp").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn borrar_respeta_la_antiguedad() {
        let d = preparar("borrar");
        let c = recorrer(&d, true, SystemTime::now());
        assert_eq!(c.archivos, 0);
        assert!(d.join("nuevo.tmp").exists());
        let futuro = SystemTime::now() + Duration::from_secs(25 * 3600);
        let c = recorrer(&d, true, futuro);
        assert_eq!(c.archivos, 3);
        assert!(!d.join("viejo.tmp").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(windows)]
    #[test]
    fn un_archivo_en_uso_se_omite_y_la_limpieza_sigue() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = preparar("en-uso");
        let en_uso = fs::OpenOptions::new().read(true).share_mode(0).open(d.join("viejo.tmp")).unwrap();
        let futuro = SystemTime::now() + Duration::from_secs(25 * 3600);
        let c = recorrer(&d, true, futuro);
        assert!(d.join("viejo.tmp").exists());
        assert!(c.omitidos >= 1);
        assert_eq!(c.archivos, 2);
        assert_eq!(c.bytes, 800);
        drop(en_uso);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(windows)]
    #[test]
    fn no_sigue_uniones_fuera_de_la_carpeta() {
        let d = preparar("union");
        let destino = std::env::temp_dir().join(format!("glassdeck-destino-{}", std::process::id()));
        let _ = fs::remove_dir_all(&destino);
        fs::create_dir_all(&destino).unwrap();
        fs::write(destino.join("importante.txt"), b"no borrar").unwrap();
        let union = d.join("union");
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&union)
            .arg(&destino)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            let futuro = SystemTime::now() + Duration::from_secs(25 * 3600);
            recorrer(&d, true, futuro);
            assert!(destino.join("importante.txt").exists());
        }
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&destino);
    }
}

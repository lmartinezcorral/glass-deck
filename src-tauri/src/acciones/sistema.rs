use std::process::Command;
use std::time::Duration;

#[cfg(windows)]
const SIN_VENTANA: u32 = 0x0800_0000;

fn comando(programa: &str) -> Command {
    let mut c = Command::new(programa);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(SIN_VENTANA);
    }
    c
}

pub fn abrir_administrador_tareas() -> Result<(), String> {
    Command::new("taskmgr.exe")
        .spawn()
        .map(|_| ())
        .map_err(|_| "No se pudo abrir el Administrador de tareas".into())
}

pub fn reiniciar_explorer() -> Result<(), String> {
    let salida = comando("taskkill.exe")
        .args(["/F", "/IM", "explorer.exe", "/FI", &format!("USERNAME eq {}", usuario_actual())])
        .output()
        .map_err(|_| "No se pudo detener Explorer".to_string())?;
    if !salida.status.success() {
        return Err("No se pudo detener Explorer".into());
    }
    std::thread::sleep(Duration::from_millis(800));
    Command::new("explorer.exe")
        .spawn()
        .map(|_| ())
        .map_err(|_| "Explorer se detuvo pero no se pudo volver a iniciar; ábrelo con Ctrl+Shift+Esc → Archivo → Ejecutar".into())
}

fn usuario_actual() -> String {
    std::env::var("USERNAME").unwrap_or_default()
}

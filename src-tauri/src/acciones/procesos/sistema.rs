use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::core::{BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, HANDLE, HWND, LPARAM, WAIT_OBJECT_0,
    WAIT_TIMEOUT, WIN32_ERROR, WPARAM,
};
use windows::Win32::Security::{
    EqualSid, GetLengthSid, GetTokenInformation, IsWellKnownSid, LookupAccountSidW, TokenUser,
    WinLocalServiceSid, WinLocalSystemSid, WinNetworkServiceSid, PSID, SID_NAME_USE, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, TerminateProcess, WaitForSingleObject,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
};

use super::politica::{Candidato, Dueno, ResultadoCierre, PIDS_PROTEGIDOS};

pub const ESPERA_CIERRE_AMABLE: Duration = Duration::from_secs(5);

const INTERVALO_SONDEO: Duration = Duration::from_millis(250);
const ESPERA_TRAS_TERMINAR: Duration = Duration::from_secs(2);
const CODIGO_SALIDA_FORZADA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcesoVivo {
    pub pid: u32,
    pub nombre: String,
    pub dueno: Dueno,
    pub usuario: Option<String>,
}

impl ProcesoVivo {
    pub fn candidato(&self) -> Candidato {
        Candidato {
            pid: self.pid,
            nombre: self.nombre.clone(),
            dueno: self.dueno,
        }
    }
}

impl From<&ProcesoVivo> for Candidato {
    fn from(p: &ProcesoVivo) -> Self {
        p.candidato()
    }
}

struct Manija(HANDLE);

impl Drop for Manija {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

struct UsuarioToken {
    buffer: Vec<u64>,
}

impl UsuarioToken {
    fn sid(&self) -> PSID {
        unsafe { (*(self.buffer.as_ptr() as *const TOKEN_USER)).User.Sid }
    }
}

type CacheDuenos = HashMap<Vec<u8>, (Dueno, Option<String>)>;

fn es_error(e: &windows::core::Error, codigo: WIN32_ERROR) -> bool {
    e.code() == codigo.to_hresult()
}

fn texto_utf16(buffer: &[u16]) -> String {
    let fin = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..fin])
}

fn usuario_de_token(token: HANDLE) -> Option<UsuarioToken> {
    let mut largo = 0u32;
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut largo) };
    if largo == 0 {
        return None;
    }
    let mut buffer = vec![0u64; (largo as usize + 7) / 8];
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut c_void),
            (buffer.len() * 8) as u32,
            &mut largo,
        )
    }
    .ok()?;
    let usuario = UsuarioToken { buffer };
    if usuario.sid().is_invalid() {
        return None;
    }
    Some(usuario)
}

fn usuario_de_proceso(proceso: HANDLE) -> Option<UsuarioToken> {
    let mut token = HANDLE(std::ptr::null_mut());
    unsafe { OpenProcessToken(proceso, TOKEN_QUERY, &mut token) }.ok()?;
    let token = Manija(token);
    usuario_de_token(token.0)
}

fn usuario_actual() -> Option<&'static UsuarioToken> {
    static ACTUAL: OnceLock<Option<UsuarioToken>> = OnceLock::new();
    ACTUAL
        .get_or_init(|| usuario_de_proceso(unsafe { GetCurrentProcess() }))
        .as_ref()
}

fn bytes_sid(sid: PSID) -> Vec<u8> {
    let largo = unsafe { GetLengthSid(sid) } as usize;
    unsafe { std::slice::from_raw_parts(sid.0 as *const u8, largo) }.to_vec()
}

fn clasificar_sid(sid: PSID) -> Dueno {
    let es_servicio = [WinLocalSystemSid, WinLocalServiceSid, WinNetworkServiceSid]
        .into_iter()
        .any(|tipo| unsafe { IsWellKnownSid(sid, tipo) }.as_bool());
    if es_servicio {
        return Dueno::Sistema;
    }
    match usuario_actual() {
        Some(actual) if unsafe { EqualSid(sid, actual.sid()) }.is_ok() => Dueno::UsuarioActual,
        Some(_) => Dueno::OtroUsuario,
        None => Dueno::Desconocido,
    }
}

fn nombre_de_cuenta(sid: PSID) -> Option<String> {
    let mut largo_nombre = 0u32;
    let mut largo_dominio = 0u32;
    let mut uso = SID_NAME_USE::default();
    let _ = unsafe {
        LookupAccountSidW(
            PCWSTR::null(),
            sid,
            None,
            &mut largo_nombre,
            None,
            &mut largo_dominio,
            &mut uso,
        )
    };
    if largo_nombre == 0 {
        return None;
    }
    let mut nombre = vec![0u16; largo_nombre as usize];
    let mut dominio = vec![0u16; largo_dominio.max(1) as usize];
    largo_dominio = dominio.len() as u32;
    unsafe {
        LookupAccountSidW(
            PCWSTR::null(),
            sid,
            Some(PWSTR(nombre.as_mut_ptr())),
            &mut largo_nombre,
            Some(PWSTR(dominio.as_mut_ptr())),
            &mut largo_dominio,
            &mut uso,
        )
    }
    .ok()?;
    let nombre = texto_utf16(&nombre);
    let dominio = texto_utf16(&dominio);
    if nombre.is_empty() {
        return None;
    }
    Some(if dominio.is_empty() {
        nombre
    } else {
        format!("{dominio}\\{nombre}")
    })
}

fn dueno_con_cache(pid: u32, cache: &mut CacheDuenos) -> (Dueno, Option<String>) {
    if PIDS_PROTEGIDOS.contains(&pid) {
        return (Dueno::Sistema, None);
    }
    let proceso = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => Manija(h),
        Err(_) => return (Dueno::Desconocido, None),
    };
    let Some(usuario) = usuario_de_proceso(proceso.0) else {
        return (Dueno::Desconocido, None);
    };
    let sid = usuario.sid();
    cache
        .entry(bytes_sid(sid))
        .or_insert_with(|| (clasificar_sid(sid), nombre_de_cuenta(sid)))
        .clone()
}

pub fn dueno_de(pid: u32) -> (Dueno, Option<String>) {
    dueno_con_cache(pid, &mut HashMap::new())
}

pub fn enumerar() -> windows::core::Result<Vec<ProcesoVivo>> {
    let instantanea = Manija(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?);
    let mut entrada = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut cache = CacheDuenos::new();
    let mut vivos = Vec::new();
    let mut hay = unsafe { Process32FirstW(instantanea.0, &mut entrada) }.is_ok();
    while hay {
        let pid = entrada.th32ProcessID;
        let (dueno, usuario) = dueno_con_cache(pid, &mut cache);
        vivos.push(ProcesoVivo {
            pid,
            nombre: texto_utf16(&entrada.szExeFile),
            dueno,
            usuario,
        });
        hay = unsafe { Process32NextW(instantanea.0, &mut entrada) }.is_ok();
    }
    Ok(vivos)
}

struct BusquedaVentanas {
    pid: u32,
    visibles: Vec<HWND>,
    ocultas: Vec<HWND>,
}

unsafe extern "system" fn recolectar_ventana(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let busqueda = unsafe { &mut *(lparam.0 as *mut BusquedaVentanas) };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == busqueda.pid {
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            busqueda.visibles.push(hwnd);
        } else {
            busqueda.ocultas.push(hwnd);
        }
    }
    BOOL(1)
}

fn enviar_wm_close(pid: u32) -> usize {
    let mut busqueda = BusquedaVentanas {
        pid,
        visibles: Vec::new(),
        ocultas: Vec::new(),
    };
    let _ = unsafe {
        EnumWindows(
            Some(recolectar_ventana),
            LPARAM(&mut busqueda as *mut BusquedaVentanas as isize),
        )
    };
    let destino = if busqueda.visibles.is_empty() {
        &busqueda.ocultas
    } else {
        &busqueda.visibles
    };
    destino
        .iter()
        .filter(|&&hwnd| {
            unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok()
        })
        .count()
}

fn termino(proceso: &Manija) -> bool {
    let estado = unsafe { WaitForSingleObject(proceso.0, 0) };
    estado == WAIT_OBJECT_0
}

fn esperar_salida(proceso: &Manija, espera: Duration) -> bool {
    let limite = Instant::now() + espera;
    loop {
        let restante = limite.saturating_duration_since(Instant::now());
        let tramo = restante.min(INTERVALO_SONDEO).as_millis() as u32;
        match unsafe { WaitForSingleObject(proceso.0, tramo) } {
            WAIT_OBJECT_0 => return true,
            WAIT_TIMEOUT if !restante.is_zero() => continue,
            _ => return false,
        }
    }
}

fn forzar(pid: u32, sincronia: Option<&Manija>) -> ResultadoCierre {
    let ya_termino = || sincronia.map(termino).unwrap_or(false);
    let proceso = match unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) } {
        Ok(h) => Manija(h),
        Err(_) if ya_termino() => return ResultadoCierre::Cerrado,
        Err(e) if es_error(&e, ERROR_INVALID_PARAMETER) => return ResultadoCierre::YaNoExistia,
        Err(_) => return ResultadoCierre::SinPermiso,
    };
    match unsafe { TerminateProcess(proceso.0, CODIGO_SALIDA_FORZADA) } {
        Ok(()) => {
            if let Some(s) = sincronia {
                esperar_salida(s, ESPERA_TRAS_TERMINAR);
            }
            ResultadoCierre::Cerrado
        }
        Err(_) if ya_termino() => ResultadoCierre::Cerrado,
        Err(e) if es_error(&e, ERROR_INVALID_PARAMETER) => ResultadoCierre::YaNoExistia,
        Err(_) => ResultadoCierre::SinPermiso,
    }
}

pub fn cerrar(pid: u32, espera: Duration) -> ResultadoCierre {
    if PIDS_PROTEGIDOS.contains(&pid) {
        return ResultadoCierre::Rechazado;
    }
    let sincronia = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(h) => Manija(h),
        Err(e) if es_error(&e, ERROR_INVALID_PARAMETER) => return ResultadoCierre::YaNoExistia,
        Err(e) if es_error(&e, ERROR_ACCESS_DENIED) => return ResultadoCierre::SinPermiso,
        Err(_) => return forzar(pid, None),
    };
    if termino(&sincronia) {
        return ResultadoCierre::YaNoExistia;
    }
    if enviar_wm_close(pid) > 0 && esperar_salida(&sincronia, espera) {
        return ResultadoCierre::Cerrado;
    }
    forzar(pid, Some(&sincronia))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proceso_propio_es_del_usuario_actual() {
        let (dueno, usuario) = dueno_de(std::process::id());
        assert_eq!(dueno, Dueno::UsuarioActual);
        assert!(usuario.is_some());
    }

    #[test]
    fn enumerar_incluye_proceso_propio() {
        let vivos = enumerar().expect("snapshot");
        let propio = vivos
            .iter()
            .find(|p| p.pid == std::process::id())
            .expect("pid propio");
        assert_eq!(propio.dueno, Dueno::UsuarioActual);
        assert!(!propio.nombre.is_empty());
    }

    #[test]
    fn pid_inexistente() {
        assert_eq!(dueno_de(u32::MAX - 2).0, Dueno::Desconocido);
        assert_eq!(cerrar(u32::MAX - 2, Duration::from_millis(10)), ResultadoCierre::YaNoExistia);
    }

    #[test]
    fn pids_de_sistema_no_se_tocan() {
        assert_eq!(cerrar(0, Duration::ZERO), ResultadoCierre::Rechazado);
        assert_eq!(cerrar(4, Duration::ZERO), ResultadoCierre::Rechazado);
    }

    #[test]
    fn cierra_proceso_hijo_sin_ventanas() {
        let mut hijo = std::process::Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawn");
        let pid = hijo.id();
        assert_eq!(dueno_de(pid).0, Dueno::UsuarioActual);
        assert_eq!(cerrar(pid, Duration::from_millis(500)), ResultadoCierre::Cerrado);
        let _ = hijo.wait();
    }
}

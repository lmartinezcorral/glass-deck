//! Arranque con Windows (GD-INST-02, GD-CUST-07): tarea programada «al iniciar sesión» del usuario, registrada con
//! la API COM del Programador de tareas, y la clave Run de `tauri-plugin-autostart` solo como respaldo.

use parking_lot::Mutex;
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub const NOMBRE_TAREA: &str = "Glassdeck";
pub const ARGUMENTO: &str = "--arranque";
const ESPERA_SHELL_MAX: Duration = Duration::from_secs(30);
const SONDEO_SHELL: Duration = Duration::from_millis(200);

static TURNO: Mutex<()> = Mutex::new(());
static ESPERA_SHELL: Mutex<Option<Duration>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefTarea {
    /// `DOMINIO\usuario`: el disparador y la tarea son solo de este usuario.
    pub usuario: String,
    pub exe: String,
    pub argumentos: String,
    pub carpeta: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metodo {
    Tarea,
    Run,
    Ninguno,
}

pub trait Sistema {
    /// XML de la tarea registrada, si existe.
    fn consultar_tarea(&mut self) -> Option<String>;
    fn crear_tarea(&mut self, xml: &str) -> Result<(), String>;
    fn borrar_tarea(&mut self) -> Result<(), String>;
    fn run_activo(&mut self) -> bool;
    fn activar_run(&mut self) -> Result<(), String>;
    fn borrar_run(&mut self) -> Result<(), String>;
}

pub fn lanzado_al_arrancar() -> bool {
    std::env::args().any(|a| a == ARGUMENTO)
}

pub fn es_binario_de_desarrollo(exe: &Path) -> bool {
    exe.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target"))
        && exe.components().any(|c| {
            let s = c.as_os_str();
            s.eq_ignore_ascii_case("debug") || s.eq_ignore_ascii_case("release")
        })
}

fn escapar(texto: &str) -> String {
    texto
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn desescapar(texto: &str) -> String {
    texto
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Sin `<Delay>` en el disparador: Windows la lanza en cuanto el usuario inicia sesión. Las tareas corren por
/// defecto con prioridad 7 (debajo de lo normal, que también baja la prioridad de E/S); 4 es la normal.
pub fn xml_tarea(def: &DefTarea) -> String {
    let usuario = escapar(&def.usuario);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Abre Glassdeck al iniciar sesión</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{usuario}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{usuario}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{exe}"</Command>
      <Arguments>{argumentos}</Arguments>
      <WorkingDirectory>{carpeta}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
        exe = escapar(&def.exe),
        argumentos = escapar(&def.argumentos),
        carpeta = escapar(&def.carpeta),
    )
}

fn etiqueta<'a>(xml: &'a str, nombre: &str) -> Option<&'a str> {
    let abre = format!("<{nombre}>");
    let inicio = xml.find(&abre)? + abre.len();
    let fin = xml[inicio..].find(&format!("</{nombre}>"))? + inicio;
    Some(xml[inicio..fin].trim())
}

/// Las tareas registradas por versiones que usaban `schtasks` pudieron quedar con los caracteres fuera de ASCII
/// cambiados por la página de códigos de la consola, así que se comparan solo los ASCII.
fn ascii(texto: &str) -> String {
    texto.chars().filter(|c| c.is_ascii() && *c != '?').collect::<String>().to_ascii_lowercase()
}

/// Campos de la tarea registrada que no son los que se crearían.
pub fn diferencias(xml: &str, def: &DefTarea) -> Vec<&'static str> {
    let valor = |nombre: &str| etiqueta(xml, nombre).map(desescapar);
    let disparador = etiqueta(xml, "LogonTrigger");
    let comando = valor("Command").map(|c| c.trim_matches('"').to_string());
    let esperados = [
        ("LogonTrigger", disparador.is_some_and(|d| !d.contains("<Delay>"))),
        ("UserId", disparador.and_then(|d| etiqueta(d, "UserId")).map(|u| ascii(&desescapar(u))) == Some(ascii(&def.usuario))),
        ("Command", comando.map(|c| ascii(&c)) == Some(ascii(&def.exe))),
        ("Arguments", valor("Arguments").as_deref() == Some(def.argumentos.as_str())),
        ("Priority", valor("Priority").as_deref() == Some("4")),
        ("ExecutionTimeLimit", valor("ExecutionTimeLimit").as_deref() == Some("PT0S")),
        ("DisallowStartIfOnBatteries", valor("DisallowStartIfOnBatteries").as_deref() == Some("false")),
        ("StopIfGoingOnBatteries", valor("StopIfGoingOnBatteries").as_deref() == Some("false")),
        // Al exportar, Windows omite `RunLevel` cuando es `LeastPrivilege` (el valor por defecto).
        ("RunLevel", valor("RunLevel").is_none_or(|r| r == "LeastPrivilege")),
    ];
    esperados.into_iter().filter(|(_, ok)| !ok).map(|(n, _)| n).collect()
}

/// La tarea registrada ya es la que se crearía: no se vuelve a registrar en cada arranque.
pub fn coincide(xml: &str, def: &DefTarea) -> bool {
    diferencias(xml, def).is_empty()
}

pub fn decidir(s: &mut impl Sistema, activo: bool, def: &DefTarea) -> Result<Metodo, String> {
    if !activo {
        let tarea = if s.consultar_tarea().is_some() { s.borrar_tarea() } else { Ok(()) };
        let run = if s.run_activo() { s.borrar_run() } else { Ok(()) };
        return tarea.and(run).map(|_| Metodo::Ninguno);
    }
    let vigente = s.consultar_tarea().is_some_and(|x| coincide(&x, def));
    let tarea = if vigente { Ok(()) } else { s.crear_tarea(&xml_tarea(def)) };
    match tarea {
        Ok(()) => {
            if s.run_activo() {
                if let Err(e) = s.borrar_run() {
                    log::warn!("No se pudo borrar la clave Run tras crear la tarea: {e}");
                }
            }
            Ok(Metodo::Tarea)
        }
        Err(e) => {
            log::warn!("No se pudo crear la tarea programada ({e}); se usa la clave Run");
            if !s.run_activo() {
                s.activar_run()?;
            }
            Ok(Metodo::Run)
        }
    }
}

/// Sondea `listo` hasta que devuelva `true` o pase `max`; devuelve si quedó listo y cuánto esperó.
pub fn esperar_hasta(mut listo: impl FnMut() -> bool, max: Duration, paso: Duration) -> (bool, Duration) {
    let inicio = Instant::now();
    loop {
        if listo() {
            return (true, inicio.elapsed());
        }
        if inicio.elapsed() >= max {
            return (false, inicio.elapsed());
        }
        std::thread::sleep(paso);
    }
}

/// Segundos desde que arrancó Explorer hasta que Windows lanzó Glassdeck y hasta «arranque listo».
/// Si Explorer arrancó después que Glassdeck o hace más de 10 min, no describe el inicio de sesión.
pub fn tiempos_desde_explorer(explorer_ms: i64, proceso_ms: i64, ahora_ms: i64) -> Option<(f64, f64)> {
    let lanzado = proceso_ms - explorer_ms;
    let listo = ahora_ms - explorer_ms;
    ((0..600_000).contains(&lanzado) && listo >= lanzado).then(|| (lanzado as f64 / 1000.0, listo as f64 / 1000.0))
}

/// Con `--arranque` Windows puede lanzarnos antes de que existan el escritorio y la bandeja; los widgets se
/// enganchan a la capa del escritorio y el icono a la bandeja, así que se espera antes de iniciar Tauri.
pub fn esperar_escritorio_si_hace_falta() {
    if !lanzado_al_arrancar() {
        return;
    }
    #[cfg(windows)]
    {
        let (_, esperado) = esperar_hasta(win::shell_listo, ESPERA_SHELL_MAX, SONDEO_SHELL);
        *ESPERA_SHELL.lock() = Some(esperado);
    }
}

pub fn registrar_arranque_listo() {
    if !lanzado_al_arrancar() {
        return;
    }
    let espera = ESPERA_SHELL.lock().unwrap_or_default().as_secs_f64();
    #[cfg(windows)]
    if let Some((lanzado, listo)) = win::tiempos() {
        log::info!(
            "Arranque con Windows: lanzado {lanzado:.1} s después de Explorer; arranque listo a los {listo:.1} s (espera del escritorio {espera:.1} s)"
        );
        return;
    }
    log::info!("Arranque con Windows: arranque listo (espera del escritorio {espera:.1} s)");
}

/// Corre fuera del hilo principal (el Programador de tareas puede tardar) y lee la opción guardada dentro del turno: si se cambia
/// dos veces seguidas, gana el último valor guardado.
pub fn aplicar(app: &AppHandle, activo: bool) {
    if std::env::current_exe().map(|e| es_binario_de_desarrollo(&e)).unwrap_or(false) {
        if activo {
            log::info!("Binario de desarrollo: no se registra el arranque con Windows");
        }
        return;
    }
    let app = app.clone();
    let hilo = std::thread::Builder::new().name("glassdeck-arranque".into()).spawn(move || {
        let _turno = TURNO.lock();
        let activo = app.state::<crate::config::AlmacenConfig>().leer().arranque_windows;
        aplicar_ahora(&app, activo);
    });
    if hilo.is_err() {
        log::warn!("No se pudo cambiar el arranque con Windows: sin hilo");
    }
}

#[cfg(windows)]
fn aplicar_ahora(app: &AppHandle, activo: bool) {
    let Some(def) = win::def_actual() else {
        log::warn!("No se pudo cambiar el arranque con Windows: sin ruta del ejecutable");
        return;
    };
    let mut sistema = win::SistemaReal { tareas: win::ProgramadorTareas::nuevo(NOMBRE_TAREA), app };
    match decidir(&mut sistema, activo, &def) {
        Ok(Metodo::Tarea) => log::info!("Arranque con Windows: tarea programada"),
        Ok(Metodo::Run) => log::info!("Arranque con Windows: clave Run (respaldo)"),
        Ok(Metodo::Ninguno) => log::info!("Arranque con Windows desactivado"),
        Err(e) => log::warn!("No se pudo cambiar el arranque con Windows: {e}"),
    }
}

#[cfg(not(windows))]
fn aplicar_ahora(_app: &AppHandle, _activo: bool) {}

#[cfg(windows)]
pub mod win {
    use super::{DefTarea, Sistema, ARGUMENTO};
    use tauri::AppHandle;
    use tauri_plugin_autostart::ManagerExt;
    use windows::core::{w, BSTR, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, FILETIME};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::System::TaskScheduler::{ITaskFolder, ITaskService, TaskScheduler, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};

    const MS_ENTRE_1601_Y_1970: i64 = 11_644_473_600_000;

    pub fn def_actual() -> Option<DefTarea> {
        let exe = std::env::current_exe().ok()?;
        let carpeta = exe.parent()?.to_string_lossy().to_string();
        let nombre = std::env::var("USERNAME").ok().filter(|u| !u.is_empty())?;
        let usuario = match std::env::var("USERDOMAIN") {
            Ok(d) if !d.is_empty() => format!("{d}\\{nombre}"),
            _ => nombre,
        };
        Some(DefTarea { usuario, exe: exe.to_string_lossy().to_string(), argumentos: ARGUMENTO.into(), carpeta })
    }

    /// `CoUninitialize` solo si este hilo inicializó COM (`S_OK` o `S_FALSE`); si ya estaba en otro modo, se usa ese.
    struct Com(bool);

    impl Com {
        fn iniciar() -> Self {
            Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    fn describir(e: &windows::core::Error) -> String {
        format!("0x{:08X}", e.code().0 as u32)
    }

    /// Las interfaces COM se sueltan dentro de `f`, antes de liberar COM al salir.
    fn con_carpeta_raiz<T>(f: impl FnOnce(&ITaskFolder) -> windows::core::Result<T>) -> windows::core::Result<T> {
        let _com = Com::iniciar();
        let raiz = unsafe {
            let servicio: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let vacio = VARIANT::default();
            servicio.Connect(&vacio, &vacio, &vacio, &vacio)?;
            servicio.GetFolder(&BSTR::from("\\"))?
        };
        f(&raiz)
    }

    /// API COM del Programador de tareas (GD-INST-02): el XML va en memoria, sin procesos hijos ni archivos temporales.
    pub struct ProgramadorTareas {
        nombre: String,
    }

    impl ProgramadorTareas {
        pub fn nuevo(nombre: &str) -> Self {
            Self { nombre: nombre.into() }
        }

        pub fn consultar(&self) -> Option<String> {
            con_carpeta_raiz(|c| unsafe { c.GetTask(&BSTR::from(self.nombre.as_str()))?.Xml() }).ok().map(|x| x.to_string())
        }

        pub fn crear(&self, xml: &str) -> Result<(), String> {
            con_carpeta_raiz(|c| unsafe {
                let vacio = VARIANT::default();
                c.RegisterTask(
                    &BSTR::from(self.nombre.as_str()),
                    &BSTR::from(xml),
                    TASK_CREATE_OR_UPDATE.0,
                    &vacio,
                    &vacio,
                    TASK_LOGON_INTERACTIVE_TOKEN,
                    &vacio,
                )
                .map(|_| ())
            })
            .map_err(|e| format!("RegisterTask falló ({})", describir(&e)))
        }

        pub fn borrar(&self) -> Result<(), String> {
            match con_carpeta_raiz(|c| unsafe { c.DeleteTask(&BSTR::from(self.nombre.as_str()), 0) }) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() => Ok(()),
                Err(e) => Err(format!("DeleteTask falló ({})", describir(&e))),
            }
        }
    }

    pub struct SistemaReal<'a> {
        pub tareas: ProgramadorTareas,
        pub app: &'a AppHandle,
    }

    impl Sistema for SistemaReal<'_> {
        fn consultar_tarea(&mut self) -> Option<String> {
            self.tareas.consultar()
        }
        fn crear_tarea(&mut self, xml: &str) -> Result<(), String> {
            self.tareas.crear(xml)
        }
        fn borrar_tarea(&mut self) -> Result<(), String> {
            self.tareas.borrar()
        }
        fn run_activo(&mut self) -> bool {
            self.app.autolaunch().is_enabled().unwrap_or(false)
        }
        fn activar_run(&mut self) -> Result<(), String> {
            self.app.autolaunch().enable().map_err(|e| e.to_string())
        }
        fn borrar_run(&mut self) -> Result<(), String> {
            self.app.autolaunch().disable().map_err(|e| e.to_string())
        }
    }

    pub fn shell_listo() -> bool {
        unsafe { FindWindowW(w!("Progman"), PCWSTR::null()).is_ok() && FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).is_ok() }
    }

    fn a_unix_ms(ft: FILETIME) -> i64 {
        let intervalos = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
        (intervalos / 10_000) as i64 - MS_ENTRE_1601_Y_1970
    }

    fn creacion(proceso: windows::Win32::Foundation::HANDLE) -> Option<i64> {
        let (mut c, mut s, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe { GetProcessTimes(proceso, &mut c, &mut s, &mut k, &mut u) }.ok()?;
        Some(a_unix_ms(c))
    }

    /// Explorer es del mismo usuario: basta `PROCESS_QUERY_LIMITED_INFORMATION`, sin privilegios.
    pub fn tiempos() -> Option<(f64, f64)> {
        let bandeja = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }.ok()?;
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(bandeja, Some(&mut pid)) };
        if pid == 0 {
            return None;
        }
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let explorer = creacion(h);
        unsafe {
            let _ = CloseHandle(h);
        }
        let propio = creacion(unsafe { GetCurrentProcess() })?;
        super::tiempos_desde_explorer(explorer?, propio, crate::config::estado::ahora_ms())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn def() -> DefTarea {
        DefTarea {
            usuario: r"EQUIPO\luis".into(),
            exe: r"C:\Users\Luis Angel\AppData\Local\Glass & Deck\glassdeck.exe".into(),
            argumentos: ARGUMENTO.into(),
            carpeta: r"C:\Users\Luis Angel\AppData\Local\Glass & Deck".into(),
        }
    }

    #[derive(Default)]
    struct Simulado {
        tarea: Option<String>,
        run: bool,
        falla_crear: bool,
        falla_run: bool,
        creaciones: usize,
    }

    impl Sistema for Simulado {
        fn consultar_tarea(&mut self) -> Option<String> {
            self.tarea.clone()
        }
        fn crear_tarea(&mut self, xml: &str) -> Result<(), String> {
            if self.falla_crear {
                return Err("acceso denegado".into());
            }
            self.creaciones += 1;
            self.tarea = Some(xml.into());
            Ok(())
        }
        fn borrar_tarea(&mut self) -> Result<(), String> {
            self.tarea = None;
            Ok(())
        }
        fn run_activo(&mut self) -> bool {
            self.run
        }
        fn activar_run(&mut self) -> Result<(), String> {
            if self.falla_run {
                return Err("registro bloqueado".into());
            }
            self.run = true;
            Ok(())
        }
        fn borrar_run(&mut self) -> Result<(), String> {
            self.run = false;
            Ok(())
        }
    }

    #[test]
    fn el_xml_es_del_usuario_sin_retraso_prioridad_normal_y_sin_limite() {
        let x = xml_tarea(&def());
        let disparador = etiqueta(&x, "LogonTrigger").unwrap();
        assert_eq!(etiqueta(disparador, "UserId"), Some(r"EQUIPO\luis"));
        assert!(!x.contains("<Delay>"));
        assert_eq!(etiqueta(&x, "Priority"), Some("4"));
        assert_eq!(etiqueta(&x, "ExecutionTimeLimit"), Some("PT0S"));
        assert_eq!(etiqueta(&x, "DisallowStartIfOnBatteries"), Some("false"));
        assert_eq!(etiqueta(&x, "StopIfGoingOnBatteries"), Some("false"));
        assert_eq!(etiqueta(&x, "MultipleInstancesPolicy"), Some("IgnoreNew"));
        assert_eq!(etiqueta(&x, "RunLevel"), Some("LeastPrivilege"));
        assert_eq!(etiqueta(&x, "LogonType"), Some("InteractiveToken"));
        assert_eq!(etiqueta(&x, "Arguments"), Some("--arranque"));
        assert!(!x.contains("HighestAvailable"));
    }

    #[test]
    fn la_ruta_con_espacios_va_entre_comillas_y_escapada() {
        let x = xml_tarea(&def());
        assert_eq!(etiqueta(&x, "Command"), Some(r#""C:\Users\Luis Angel\AppData\Local\Glass &amp; Deck\glassdeck.exe""#));
        assert_eq!(etiqueta(&x, "WorkingDirectory"), Some(r"C:\Users\Luis Angel\AppData\Local\Glass &amp; Deck"));
        assert!(!x.contains("Glass & Deck"), "un & sin escapar rompe el XML");
        assert!(coincide(&x, &def()), "el XML generado se reconoce como vigente");
    }

    #[test]
    fn la_tarea_vigente_no_se_vuelve_a_registrar_y_una_distinta_si() {
        let mut s = Simulado { tarea: Some(xml_tarea(&def())), ..Default::default() };
        assert_eq!(decidir(&mut s, true, &def()), Ok(Metodo::Tarea));
        assert_eq!(s.creaciones, 0);
        let mut otra = def();
        otra.exe = r"C:\Otra\glassdeck.exe".into();
        s.tarea = Some(xml_tarea(&otra));
        assert_eq!(decidir(&mut s, true, &def()), Ok(Metodo::Tarea));
        assert_eq!(s.creaciones, 1);
        s.tarea = Some(xml_tarea(&def()).replace("<Priority>4</Priority>", "<Priority>7</Priority>"));
        decidir(&mut s, true, &def()).unwrap();
        assert_eq!(s.creaciones, 2, "una tarea con la prioridad por defecto se corrige");
        s.tarea = Some(xml_tarea(&def()).replace("LeastPrivilege", "HighestAvailable"));
        decidir(&mut s, true, &def()).unwrap();
        assert_eq!(s.creaciones, 3, "nunca se deja una tarea elevada");
        s.tarea = Some(xml_tarea(&def()).replace("<RunLevel>LeastPrivilege</RunLevel>", ""));
        decidir(&mut s, true, &def()).unwrap();
        assert_eq!(s.creaciones, 3, "Windows omite RunLevel cuando es el mínimo");
    }

    #[test]
    fn con_la_tarea_creada_se_borra_la_clave_run() {
        let mut s = Simulado { run: true, ..Default::default() };
        assert_eq!(decidir(&mut s, true, &def()), Ok(Metodo::Tarea));
        assert!(s.tarea.is_some());
        assert!(!s.run, "sin doble lanzamiento");
    }

    #[test]
    fn si_la_tarea_falla_queda_la_clave_run_de_respaldo() {
        let mut s = Simulado { falla_crear: true, ..Default::default() };
        assert_eq!(decidir(&mut s, true, &def()), Ok(Metodo::Run));
        assert!(s.run);
        let mut s = Simulado { falla_crear: true, falla_run: true, ..Default::default() };
        assert!(decidir(&mut s, true, &def()).is_err());
    }

    #[test]
    fn desactivar_borra_la_tarea_y_la_clave_run() {
        let mut s = Simulado { tarea: Some(xml_tarea(&def())), run: true, ..Default::default() };
        assert_eq!(decidir(&mut s, false, &def()), Ok(Metodo::Ninguno));
        assert!(s.tarea.is_none() && !s.run);
        assert_eq!(decidir(&mut s, false, &def()), Ok(Metodo::Ninguno), "sin nada registrado no es error");
    }

    #[test]
    fn la_tarea_consultada_se_compara_sin_caracteres_fuera_de_ascii() {
        let mut d = def();
        d.exe = r"C:\Users\Martínez\glassdeck.exe".into();
        let consultado = xml_tarea(&d).replace("Martínez", "Mart\u{FFFD}nez");
        assert!(coincide(&consultado, &d));
        let mut otro = d.clone();
        otro.usuario = r"EQUIPO\otra".into();
        assert!(!coincide(&consultado, &otro), "la tarea de otro usuario no cuenta");
    }

    #[test]
    fn detecta_binarios_de_cargo() {
        assert!(es_binario_de_desarrollo(Path::new(r"C:\x\glassdeck\src-tauri\target\release\glassdeck.exe")));
        assert!(es_binario_de_desarrollo(Path::new(r"C:\x\target\debug\glassdeck.exe")));
        assert!(!es_binario_de_desarrollo(Path::new(r"C:\Program Files\Glassdeck\glassdeck.exe")));
        assert!(!es_binario_de_desarrollo(Path::new(r"C:\Users\x\AppData\Local\Glassdeck\glassdeck.exe")));
    }

    #[test]
    fn la_espera_del_escritorio_sondea_y_tiene_tope() {
        let mut intentos = 0;
        let (listo, _) = esperar_hasta(
            || {
                intentos += 1;
                intentos >= 3
            },
            Duration::from_secs(5),
            Duration::from_millis(1),
        );
        assert!(listo);
        assert_eq!(intentos, 3);
        let (listo, esperado) = esperar_hasta(|| false, Duration::from_millis(30), Duration::from_millis(5));
        assert!(!listo);
        assert!(esperado >= Duration::from_millis(30) && esperado < Duration::from_secs(2));
    }

    #[test]
    fn los_tiempos_desde_explorer_se_omiten_si_no_son_fiables() {
        assert_eq!(tiempos_desde_explorer(1_000, 4_500, 7_000), Some((3.5, 6.0)));
        assert_eq!(tiempos_desde_explorer(5_000, 4_000, 7_000), None, "Explorer reiniciado después");
        assert_eq!(tiempos_desde_explorer(0, 700_000, 800_000), None, "no fue un inicio de sesión");
    }
}

/// QA real contra la API COM del Programador de tareas: registra una tarea de prueba con nombre único y la borra
/// siempre. Nunca toca la tarea `Glassdeck`.
#[cfg(all(test, windows))]
mod qa {
    use super::win::ProgramadorTareas;
    use super::*;

    struct BorrarAlSalir<'a>(&'a ProgramadorTareas);

    impl Drop for BorrarAlSalir<'_> {
        fn drop(&mut self) {
            let _ = self.0.borrar();
        }
    }

    #[test]
    fn qa_registra_consulta_y_borra_una_tarea_de_prueba() {
        let nombre = format!("GlassdeckPruebaQA-{}", std::process::id());
        assert_ne!(nombre, NOMBRE_TAREA);
        let cmd = std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
        let mut def = win::def_actual().expect("usuario y ejecutable");
        def.carpeta = Path::new(&cmd).parent().unwrap().to_string_lossy().to_string();
        def.exe = cmd;
        def.argumentos = "/c exit".into();
        let tareas = ProgramadorTareas::nuevo(&nombre);
        let _borrar = BorrarAlSalir(&tareas);
        tareas.crear(&xml_tarea(&def)).expect("el usuario puede registrar su tarea sin elevar");
        tareas.crear(&xml_tarea(&def)).expect("registrar de nuevo actualiza la misma tarea");
        let consultado = tareas.consultar().expect("la tarea existe");
        assert_eq!(etiqueta(&consultado, "Priority"), Some("4"));
        assert_eq!(diferencias(&consultado, &def), Vec::<&str>::new(), "la consulta se reconoce: no se vuelve a registrar en cada arranque");
        tareas.borrar().expect("se borra");
        assert!(tareas.consultar().is_none());
        assert_eq!(tareas.borrar(), Ok(()), "borrar una tarea que ya no existe no es error");
    }

    #[test]
    fn la_app_no_lanza_schtasks_ni_escribe_el_xml_en_disco() {
        let codigo = include_str!("arranque.rs").split("#[cfg(test)]").next().unwrap();
        for prohibido in [["schtasks", ".exe"].concat(), ["Command", "::new"].concat(), ["temp", "_dir()"].concat(), ["fs::", "write"].concat()] {
            assert!(!codigo.contains(&prohibido), "{prohibido}");
        }
        assert!(codigo.contains("RegisterTask(") && codigo.contains("TASK_LOGON_INTERACTIVE_TOKEN"));
    }
}

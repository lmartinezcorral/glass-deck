use std::sync::OnceLock;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, RedrawWindow, SetWindowRgn, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE,
    RDW_UPDATENOW,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindowLongPtrW, GetWindowThreadProcessId, SetWindowLongPtrW, SetWindowPos,
    EVENT_SYSTEM_FOREGROUND, GWL_EXSTYLE, GWL_STYLE, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOPMOST, STYLESTRUCT,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_NCACTIVATE, WM_NCDESTROY, WM_SETICON, WM_SETTEXT, WM_STYLECHANGING, WS_CAPTION,
    WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    WS_VISIBLE,
};

const ID_SUBCLASE: usize = 0x6764;
const ESTILOS_QUITADOS: u32 = WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;

pub fn hwnd_de(ventana: &tauri::WebviewWindow) -> Option<HWND> {
    ventana.hwnd().ok().map(|h| HWND(h.0 as _))
}

fn estilo_widget(st: u32) -> u32 {
    (st & !ESTILOS_QUITADOS) | WS_POPUP.0
}

fn estilo_ext_widget(ex: u32) -> u32 {
    (ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0
}

unsafe extern "system" fn filtrar_estilos(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _datos: usize,
) -> LRESULT {
    match msg {
        WM_STYLECHANGING if lparam.0 != 0 => {
            let cambio = &mut *(lparam.0 as *mut STYLESTRUCT);
            let indice = wparam.0 as i32;
            if indice == GWL_STYLE.0 {
                cambio.styleNew = estilo_widget(cambio.styleNew);
            } else if indice == GWL_EXSTYLE.0 {
                cambio.styleNew = estilo_ext_widget(cambio.styleNew);
            }
        }
        // DefWindowProc dibuja la barra clásica al activar la ventana o cambiar su texto o icono aunque no
        // tenga WS_CAPTION; con lParam = -1 no repinta el área no cliente.
        WM_NCACTIVATE => return DefSubclassProc(hwnd, msg, wparam, LPARAM(-1)),
        // Sin WS_VISIBLE durante la llamada, DefWindowProc actualiza el dato sin pintar nada.
        WM_SETTEXT | WM_SETICON => {
            let st = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let visible = st & WS_VISIBLE.0 as isize != 0;
            if visible {
                SetWindowLongPtrW(hwnd, GWL_STYLE, st & !(WS_VISIBLE.0 as isize));
            }
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            if visible {
                SetWindowLongPtrW(hwnd, GWL_STYLE, GetWindowLongPtrW(hwnd, GWL_STYLE) | WS_VISIBLE.0 as isize);
            }
            return r;
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(filtrar_estilos), ID_SUBCLASE);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

/// tao reescribe los estilos en cada show y vuelve a poner WS_CAPTION; Windows alcanza a pintar esa barra
/// sobre la superficie transparente y se queda ahí. Filtrar WM_STYLECHANGING evita que llegue a aplicarse.
/// La subclase solo se puede instalar desde el hilo dueño de la ventana.
pub fn proteger_estilos_widget(ventana: &tauri::WebviewWindow) {
    let Some(hwnd) = hwnd_de(ventana) else { return };
    let instalar = |h: HWND| unsafe {
        let _ = SetWindowSubclass(h, Some(filtrar_estilos), ID_SUBCLASE, 0);
    };
    if unsafe { GetWindowThreadProcessId(hwnd, None) == GetCurrentThreadId() } {
        instalar(hwnd);
        return;
    }
    let crudo = hwnd.0 as isize;
    let (tx, rx) = std::sync::mpsc::channel();
    let enviado = ventana.run_on_main_thread(move || {
        instalar(HWND(crudo as _));
        let _ = tx.send(());
    });
    if enviado.is_ok() {
        let _ = rx.recv_timeout(Duration::from_millis(500));
    }
}

/// El WebView es translúcido y deja ver la superficie de la ventana padre, que conserva la barra de título
/// pintada antes de quitar WS_CAPTION; nadie más la repinta porque el WebView cubre todo el cliente.
pub fn repintar_superficie(hwnd: HWND) {
    unsafe {
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
    }
}

pub fn aplicar_estilos_widget(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, estilo_ext_widget(ex as u32) as isize);
        let st = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(hwnd, GWL_STYLE, estilo_widget(st as u32) as isize);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_NOOWNERZORDER,
        );
    }
}

fn reordenar(hwnd: HWND, despues_de: HWND) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(despues_de),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
    }
}

pub fn enviar_al_fondo(hwnd: HWND) {
    reordenar(hwnd, HWND_NOTOPMOST);
    reordenar(hwnd, HWND_BOTTOM);
}

pub fn siempre_encima(hwnd: HWND, activo: bool) {
    reordenar(hwnd, if activo { HWND_TOPMOST } else { HWND_NOTOPMOST });
}

pub fn esquinas_redondeadas(hwnd: HWND, ancho: i32, alto: i32, radio: i32) {
    unsafe {
        let rgn = CreateRoundRectRgn(0, 0, ancho + 1, alto + 1, radio * 2, radio * 2);
        SetWindowRgn(hwnd, Some(rgn), true);
    }
}

pub fn clase_de(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn es_escritorio(clase: &str) -> bool {
    clase == "WorkerW" || clase == "Progman"
}

type AvisoPrimerPlano = Box<dyn Fn(bool) + Send + Sync>;
static AVISO: OnceLock<AvisoPrimerPlano> = OnceLock::new();

unsafe extern "system" fn al_cambiar_primer_plano(
    _hook: HWINEVENTHOOK,
    _evento: u32,
    hwnd: HWND,
    _id_objeto: i32,
    _id_hijo: i32,
    _hilo: u32,
    _tiempo: u32,
) {
    if let Some(aviso) = AVISO.get() {
        aviso(es_escritorio(&clase_de(hwnd)));
    }
}

pub fn vigilar_primer_plano<F: Fn(bool) + Send + Sync + 'static>(aviso: F) {
    if AVISO.set(Box::new(aviso)).is_err() {
        return;
    }
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(al_cambiar_primer_plano),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
    }
}

/// Diálogo «Abrir» de Windows filtrado a `*.json`; `None` si se cancela. Se llama fuera del hilo principal:
/// el diálogo modal tiene su propio bucle de mensajes y bloquearía el de Tauri.
pub fn elegir_json(dueno: isize, titulo: &str) -> Result<Option<std::path::PathBuf>, String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH};

    let fallo = |_| "No se pudo abrir el diálogo para elegir el archivo".to_string();
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let resultado = (|| {
            let dialogo: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(fallo)?;
            let nombre = HSTRING::from("Cliente de Google (JSON)");
            let patron = HSTRING::from("*.json");
            let filtros = [COMDLG_FILTERSPEC { pszName: PCWSTR(nombre.as_ptr()), pszSpec: PCWSTR(patron.as_ptr()) }];
            dialogo.SetFileTypes(&filtros).map_err(fallo)?;
            let opciones = dialogo.GetOptions().map_err(fallo)?;
            dialogo.SetOptions(opciones | FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM).map_err(fallo)?;
            dialogo.SetTitle(&HSTRING::from(titulo)).map_err(fallo)?;
            let dueno = (dueno != 0).then_some(HWND(dueno as _));
            if let Err(e) = dialogo.Show(dueno) {
                return if e.code() == ERROR_CANCELLED.to_hresult() { Ok(None) } else { Err(fallo(e)) };
            }
            let ruta = dialogo.GetResult().and_then(|i| i.GetDisplayName(SIGDN_FILESYSPATH)).map_err(fallo)?;
            let texto = ruta.to_string();
            CoTaskMemFree(Some(ruta.0 as _));
            texto.map(|t| Some(std::path::PathBuf::from(t))).map_err(|_| "La ruta del archivo no es válida".to_string())
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        resultado
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW;

    #[test]
    fn el_filtro_quita_la_barra_y_conserva_la_visibilidad() {
        let st = estilo_widget(WS_OVERLAPPEDWINDOW.0 | WS_VISIBLE.0);
        assert_eq!(st & WS_CAPTION.0, 0);
        assert_eq!(st & WS_THICKFRAME.0, 0);
        assert_eq!(st & WS_SYSMENU.0, 0);
        assert_ne!(st & WS_POPUP.0, 0);
        assert_ne!(st & WS_VISIBLE.0, 0);
    }

    #[test]
    fn el_filtro_saca_la_ventana_de_alt_tab() {
        let ex = estilo_ext_widget(WS_EX_APPWINDOW.0);
        assert_ne!(ex & WS_EX_TOOLWINDOW.0, 0);
        assert_eq!(ex & WS_EX_APPWINDOW.0, 0);
    }
}

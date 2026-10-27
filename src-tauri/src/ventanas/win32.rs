use std::sync::OnceLock;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, EVENT_SYSTEM_FOREGROUND,
    GWL_EXSTYLE, GWL_STYLE, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
};

pub fn hwnd_de(ventana: &tauri::WebviewWindow) -> Option<HWND> {
    ventana.hwnd().ok().map(|h| HWND(h.0 as _))
}

pub fn aplicar_estilos_widget(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let ex = (ex | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
        let st = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let st = st & !((WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) as isize);
        SetWindowLongPtrW(hwnd, GWL_STYLE, st);
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

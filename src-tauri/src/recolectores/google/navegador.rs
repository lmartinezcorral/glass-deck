use reqwest::Url;

/// Solo Gmail y Calendar se abren en el navegador del sistema (GD-ROT-03); no son conexiones de Glassdeck.
pub fn url_permitida(url: &str) -> bool {
    let Ok(u) = Url::parse(url) else { return false };
    let base = u.scheme() == "https" && u.username().is_empty() && u.password().is_none() && u.port().is_none();
    base && match u.host_str() {
        Some("mail.google.com") => u.path().starts_with("/mail/"),
        Some("calendar.google.com") => true,
        Some("www.google.com") => u.path().starts_with("/calendar/"),
        _ => false,
    }
}

fn es_autorizacion(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https" && u.host_str() == Some("accounts.google.com") && u.port().is_none() && u.username().is_empty()
    })
}

pub fn abrir(url: &str) -> Result<(), String> {
    if !url_permitida(url) {
        return Err("Ese enlace no es de Gmail ni de Google Calendar".into());
    }
    lanzar(url)
}

pub fn abrir_autorizacion(url: &str) -> Result<(), String> {
    if !es_autorizacion(url) {
        return Err("Dirección de inicio de sesión inválida".into());
    }
    lanzar(url)
}

#[cfg(windows)]
fn lanzar(url: &str) -> Result<(), String> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let destino = HSTRING::from(url);
    let r = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(destino.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err("No se pudo abrir el navegador predeterminado".into())
    }
}

#[cfg(not(windows))]
fn lanzar(_: &str) -> Result<(), String> {
    Err("Solo disponible en Windows".into())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_se_abren_enlaces_de_gmail_y_calendar() {
        for bueno in [
            "https://mail.google.com/mail/u/cuenta.uno@example.com/#inbox/18f0000000000001",
            "https://www.google.com/calendar/event?eid=abc&authuser=cuenta.uno@example.com",
            "https://calendar.google.com/calendar/r",
        ] {
            assert!(url_permitida(bueno), "{bueno}");
        }
        for malo in [
            "http://mail.google.com/mail/u/0/",
            "https://mail.google.com.example.com/mail/",
            "https://www.google.com/search?q=x",
            "https://usuario@mail.google.com/mail/",
            "https://mail.google.com:8443/mail/",
            "https://accounts.google.com/o/oauth2/v2/auth",
            "javascript:alert(1)",
            "file:///C:/Windows/win.ini",
        ] {
            assert!(!url_permitida(malo), "{malo}");
            assert!(abrir(malo).is_err());
        }
        assert!(es_autorizacion("https://accounts.google.com/o/oauth2/v2/auth?x=1"));
        assert!(!es_autorizacion("https://mail.google.com/mail/"));
        assert!(abrir_autorizacion("https://example.com/").is_err());
    }
}

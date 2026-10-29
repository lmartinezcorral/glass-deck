use keyring::Entry;
use serde::{Deserialize, Serialize};

pub const PREFIJO: &str = "Glassdeck/";
const USUARIO: &str = "glassdeck";
const LARGO_MAX_VALOR: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clave {
    AnthropicAdmin,
    GoogleCliente,
    OpenaiAdmin,
}

impl Clave {
    pub const TODAS: [Clave; 3] = [Clave::AnthropicAdmin, Clave::GoogleCliente, Clave::OpenaiAdmin];

    pub fn nombre(self) -> &'static str {
        match self {
            Clave::AnthropicAdmin => "anthropic/admin",
            Clave::GoogleCliente => "google/cliente",
            Clave::OpenaiAdmin => "openai/admin",
        }
    }

    /// El cliente OAuth llega por su propio flujo; solo las claves de administrador se teclean (GD-CLA-05, GD-OAI-05).
    pub fn se_captura_a_mano(self) -> bool {
        matches!(self, Clave::AnthropicAdmin | Clave::OpenaiAdmin)
    }
}

/// Donde las versiones anteriores a SPEC 1.6 copiaban la cookie de sesión (R-10). Las sesiones web viven solo en el
/// perfil aislado de WebView2: estos nombres no se pueden guardar y solo se borran, sin leerlos (GD-CUR-02).
pub const SESIONES_WEB_ANTIGUAS: [&str; 3] = ["cursor/sesion", "claude_ai/sesion", "chatgpt/sesion"];

fn es_sesion_web(nombre: &str) -> bool {
    SESIONES_WEB_ANTIGUAS.iter().any(|s| nombre.strip_prefix(s).is_some_and(|resto| resto.is_empty() || resto.starts_with('/')))
}

/// Envoltura sin `Serialize` ni `Display`: no puede viajar a la interfaz ni imprimirse por accidente.
#[allow(dead_code)]
pub struct Secreto(String);

#[allow(dead_code)]
impl Secreto {
    pub fn exponer(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn de_prueba(valor: &str) -> Self {
        Self(valor.to_string())
    }
}

impl std::fmt::Debug for Secreto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secreto([oculto])")
    }
}

fn validar_nombre(nombre: &str) -> Result<(), String> {
    let valido = !nombre.is_empty()
        && nombre.len() <= 200
        && nombre.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/' | '@'));
    if valido {
        Ok(())
    } else {
        Err("Nombre de credencial inválido".into())
    }
}

pub fn objetivo(nombre: &str) -> String {
    format!("{PREFIJO}{nombre}")
}

pub fn es_de_glassdeck(objetivo: &str) -> bool {
    objetivo.starts_with(PREFIJO) && objetivo.len() > PREFIJO.len()
}

/// Los errores de keyring pueden traer los bytes del secreto (`BadEncoding`): nunca se usa su texto.
fn describir(e: &keyring::Error) -> String {
    match e {
        keyring::Error::NoStorageAccess(_) => "Sin acceso al Administrador de credenciales".into(),
        keyring::Error::TooLong(..) => "El valor es demasiado largo para el Administrador de credenciales".into(),
        keyring::Error::Invalid(..) => "Nombre de credencial inválido".into(),
        keyring::Error::BadEncoding(_) => "La credencial guardada no es texto válido".into(),
        _ => "Error del Administrador de credenciales".into(),
    }
}

fn entrada(nombre: &str) -> Result<Entry, String> {
    #[cfg(test)]
    mock::instalar();
    validar_nombre(nombre)?;
    let objetivo = objetivo(nombre);
    Entry::new_with_target(&objetivo, &objetivo, USUARIO).map_err(|e| describir(&e))
}

pub fn guardar(nombre: &str, valor: &str) -> Result<(), String> {
    if es_sesion_web(nombre) {
        return Err("Las sesiones web no se guardan en la bóveda".into());
    }
    if valor.is_empty() {
        return Err("El valor está vacío".into());
    }
    if valor.len() > LARGO_MAX_VALOR {
        return Err("El valor es demasiado largo".into());
    }
    entrada(nombre)?.set_password(valor).map_err(|e| describir(&e))?;
    log::info!("Bóveda: credencial {nombre} guardada");
    Ok(())
}

pub fn existe(nombre: &str) -> bool {
    entrada(nombre).map(|e| e.get_password().is_ok()).unwrap_or(false)
}

pub fn borrar(nombre: &str) -> Result<(), String> {
    match entrada(nombre)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            log::info!("Bóveda: credencial {nombre} borrada");
            Ok(())
        }
        Err(e) => Err(describir(&e)),
    }
}

/// Solo para los adaptadores del backend (bloques 13 y 14); ningún comando de Tauri la llama.
#[allow(dead_code)]
pub(crate) fn leer(nombre: &str) -> Result<Option<Secreto>, String> {
    match entrada(nombre)?.get_password() {
        Ok(v) => Ok(Some(Secreto(v))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(describir(&e)),
    }
}

/// Las cookies que no cabían en una credencial se repartían en `nombre/1..16`.
const PARTES_MAX: usize = 16;

fn nombre_parte(nombre: &str, i: usize) -> String {
    format!("{nombre}/{i}")
}

/// Borra una sesión web que dejó una versión anterior, con sus partes, sin leer su valor; `true` si existía.
pub fn borrar_sesion_antigua(nombre: &str) -> Result<bool, String> {
    if !SESIONES_WEB_ANTIGUAS.contains(&nombre) {
        return Err("No es una sesión web antigua".into());
    }
    let existia = match entrada(nombre)?.delete_credential() {
        Ok(()) => true,
        Err(keyring::Error::NoEntry) => false,
        Err(e) => return Err(describir(&e)),
    };
    if existia {
        for i in 1..=PARTES_MAX {
            if let Ok(e) = entrada(&nombre_parte(nombre, i)) {
                let _ = e.delete_credential();
            }
        }
        log::info!("Bóveda: sesión web antigua {nombre} borrada sin leerla");
    }
    Ok(existia)
}

/// Borra todas las credenciales `Glassdeck/*` del usuario; lo usa el desinstalador (GD-INST-03).
#[cfg(windows)]
pub fn borrar_todas() -> usize {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Security::Credentials::{CredEnumerateW, CredFree, CREDENTIALW, CRED_ENUMERATE_FLAGS};
    let filtro: Vec<u16> = format!("{PREFIJO}*").encode_utf16().chain(std::iter::once(0)).collect();
    let mut cuenta = 0u32;
    let mut lista: *mut *mut CREDENTIALW = std::ptr::null_mut();
    let objetivos: Vec<String> = unsafe {
        if CredEnumerateW(PCWSTR(filtro.as_ptr()), Some(CRED_ENUMERATE_FLAGS(0)), &mut cuenta, &mut lista).is_err() {
            return 0;
        }
        let v = (0..cuenta as usize)
            .filter_map(|i| {
                let c = *lista.add(i);
                let nombre: PWSTR = (*c).TargetName;
                nombre.to_string().ok()
            })
            .collect();
        CredFree(lista as *const std::ffi::c_void);
        v
    };
    objetivos
        .iter()
        .filter(|o| es_de_glassdeck(o))
        .filter(|o| {
            Entry::new_with_target(o, o, USUARIO).map(|e| e.delete_credential().is_ok()).unwrap_or(false)
        })
        .count()
}

#[cfg(test)]
pub(crate) mod mock {
    use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi};
    use parking_lot::Mutex;
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::{LazyLock, Once};

    static ALMACEN: LazyLock<Mutex<HashMap<String, Vec<u8>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    static LECTURAS: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

    struct Credencial(String);

    impl CredentialApi for Credencial {
        fn set_secret(&self, secreto: &[u8]) -> keyring::Result<()> {
            ALMACEN.lock().insert(self.0.clone(), secreto.to_vec());
            Ok(())
        }
        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            *LECTURAS.lock().entry(self.0.clone()).or_default() += 1;
            ALMACEN.lock().get(&self.0).cloned().ok_or(keyring::Error::NoEntry)
        }
        fn delete_credential(&self) -> keyring::Result<()> {
            ALMACEN.lock().remove(&self.0).map(|_| ()).ok_or(keyring::Error::NoEntry)
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct Constructor;

    impl CredentialBuilderApi for Constructor {
        fn build(&self, objetivo: Option<&str>, servicio: &str, _: &str) -> keyring::Result<Box<Credential>> {
            Ok(Box::new(Credencial(objetivo.unwrap_or(servicio).to_string())))
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    /// Las pruebas nunca tocan el Administrador de credenciales real: todo pasa por este almacén en memoria.
    pub fn instalar() {
        static UNA: Once = Once::new();
        UNA.call_once(|| keyring::set_default_credential_builder(Box::new(Constructor)));
    }

    pub fn objetivos() -> Vec<String> {
        ALMACEN.lock().keys().cloned().collect()
    }

    /// Siembra un valor como lo habría dejado una versión anterior, sin pasar por `guardar`.
    pub fn sembrar(objetivo: &str, valor: &str) {
        instalar();
        ALMACEN.lock().insert(objetivo.to_string(), valor.as_bytes().to_vec());
    }

    /// Cuántas veces se pidió el valor de un objetivo.
    pub fn lecturas(objetivo: &str) -> usize {
        LECTURAS.lock().get(objetivo).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn guardar_existe_leer_y_borrar_con_prefijo_glassdeck() {
        let nombre = "prueba/ciclo";
        assert!(!existe(nombre));
        guardar(nombre, "valor-de-prueba-123").unwrap();
        assert!(existe(nombre));
        assert!(mock::objetivos().contains(&"Glassdeck/prueba/ciclo".to_string()));
        assert_eq!(leer(nombre).unwrap().unwrap().exponer(), "valor-de-prueba-123");
        borrar(nombre).unwrap();
        assert!(!existe(nombre));
        assert!(leer(nombre).unwrap().is_none());
        borrar(nombre).unwrap();
    }

    #[test]
    fn el_secreto_no_se_imprime_ni_se_serializa() {
        guardar("prueba/debug", "no-debe-verse-987").unwrap();
        let s = leer("prueba/debug").unwrap().unwrap();
        assert!(!format!("{s:?}").contains("no-debe-verse"));
        borrar("prueba/debug").unwrap();
    }

    #[test]
    fn rechaza_nombres_y_valores_invalidos() {
        assert!(guardar("", "x").is_err());
        assert!(guardar("Con Espacios", "x").is_err());
        assert!(guardar("ok/nombre", "").is_err());
        assert!(guardar("ok/nombre", &"x".repeat(LARGO_MAX_VALOR + 1)).is_err());
        assert!(!existe("..\\fuera"));
    }

    #[test]
    fn solo_se_reconocen_objetivos_con_prefijo() {
        assert!(es_de_glassdeck("Glassdeck/cursor/sesion"));
        assert!(!es_de_glassdeck("Glassdeck/"));
        assert!(!es_de_glassdeck("OtraApp/Glassdeck/x"));
        assert!(!es_de_glassdeck("glassdeck.Glassdeck/x"));
    }

    #[test]
    fn las_claves_conocidas_tienen_nombres_validos_y_solo_las_de_administrador_se_teclean() {
        for c in Clave::TODAS {
            assert!(validar_nombre(c.nombre()).is_ok());
            assert!(validar_nombre(&nombre_parte(c.nombre(), PARTES_MAX)).is_ok());
        }
        let a_mano: Vec<Clave> = Clave::TODAS.iter().copied().filter(|c| c.se_captura_a_mano()).collect();
        assert_eq!(a_mano, vec![Clave::AnthropicAdmin, Clave::OpenaiAdmin]);
    }

    #[test]
    fn las_sesiones_web_no_se_pueden_guardar_en_la_boveda() {
        for nombre in SESIONES_WEB_ANTIGUAS {
            for n in [nombre.to_string(), nombre_parte(nombre, 1)] {
                assert!(guardar(&n, "cookie-de-prueba").is_err(), "{n}");
                assert!(!mock::objetivos().contains(&objetivo(&n)), "{n}");
            }
        }
        assert!(!es_sesion_web("cursor/sesiones") && !es_sesion_web("anthropic/admin"));
        assert!(Clave::TODAS.iter().all(|c| !es_sesion_web(c.nombre())));
    }

    #[test]
    fn la_sesion_antigua_se_borra_con_sus_partes_sin_leerla() {
        let nombre = "chatgpt/sesion";
        mock::sembrar(&objetivo(nombre), "glassdeck-partes:2");
        mock::sembrar(&objetivo(&nombre_parte(nombre, 1)), "cookie-de-prueba-a");
        mock::sembrar(&objetivo(&nombre_parte(nombre, 2)), "cookie-de-prueba-b");
        assert_eq!(borrar_sesion_antigua(nombre), Ok(true));
        assert!(!mock::objetivos().iter().any(|o| o.starts_with("Glassdeck/chatgpt/sesion")));
        assert_eq!(borrar_sesion_antigua(nombre), Ok(false), "sin nada guardado no es error");
        for o in [objetivo(nombre), objetivo(&nombre_parte(nombre, 1)), objetivo(&nombre_parte(nombre, 2))] {
            assert_eq!(mock::lecturas(&o), 0, "{o}");
        }
        assert!(borrar_sesion_antigua("anthropic/admin").is_err(), "solo borra sesiones web");
    }
}

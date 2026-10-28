use keyring::Entry;
use serde::{Deserialize, Serialize};

pub const PREFIJO: &str = "Glassdeck/";
const USUARIO: &str = "glassdeck";
const LARGO_MAX_VALOR: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clave {
    CursorSesion,
    ClaudeAiSesion,
    AnthropicAdmin,
    GoogleCliente,
    ChatgptSesion,
    OpenaiAdmin,
}

impl Clave {
    pub const TODAS: [Clave; 6] = [
        Clave::CursorSesion,
        Clave::ClaudeAiSesion,
        Clave::AnthropicAdmin,
        Clave::GoogleCliente,
        Clave::ChatgptSesion,
        Clave::OpenaiAdmin,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Clave::CursorSesion => "cursor/sesion",
            Clave::ClaudeAiSesion => "claude_ai/sesion",
            Clave::AnthropicAdmin => "anthropic/admin",
            Clave::GoogleCliente => "google/cliente",
            Clave::ChatgptSesion => "chatgpt/sesion",
            Clave::OpenaiAdmin => "openai/admin",
        }
    }

    /// Las cookies y el cliente OAuth llegan por sus propios flujos; solo las claves de administrador se teclean (GD-CLA-05, GD-OAI-05).
    pub fn se_captura_a_mano(self) -> bool {
        matches!(self, Clave::AnthropicAdmin | Clave::OpenaiAdmin)
    }
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

/// Windows guarda cada valor en UTF-16 con un máximo de 2 560 bytes (1 280 caracteres): una cookie de ChatGPT no cabe.
const LARGO_PARTE: usize = 1_200;
const PARTES_MAX: usize = 16;
const MARCA_PARTES: &str = "glassdeck-partes:";

fn nombre_parte(nombre: &str, i: usize) -> String {
    format!("{nombre}/{i}")
}

/// Para cookies de sesión: si el valor no cabe en una credencial se reparte en `nombre/1..N` y `nombre` guarda cuántas son.
pub fn guardar_largo(nombre: &str, valor: &str) -> Result<(), String> {
    if valor.is_empty() {
        return Err("El valor está vacío".into());
    }
    borrar_partes(nombre);
    let caracteres: Vec<char> = valor.chars().collect();
    if caracteres.len() <= LARGO_PARTE {
        return guardar(nombre, valor);
    }
    let partes: Vec<String> = caracteres.chunks(LARGO_PARTE).map(|c| c.iter().collect()).collect();
    if partes.len() > PARTES_MAX {
        return Err("El valor es demasiado largo".into());
    }
    for (i, p) in partes.iter().enumerate() {
        entrada(&nombre_parte(nombre, i + 1))?.set_password(p).map_err(|e| describir(&e))?;
    }
    guardar(nombre, &format!("{MARCA_PARTES}{}", partes.len()))
}

#[allow(dead_code)]
pub(crate) fn leer_largo(nombre: &str) -> Result<Option<Secreto>, String> {
    let Some(s) = leer(nombre)? else { return Ok(None) };
    let Some(n) = s.0.strip_prefix(MARCA_PARTES).and_then(|n| n.parse::<usize>().ok()) else { return Ok(Some(s)) };
    if !(1..=PARTES_MAX).contains(&n) {
        return Ok(None);
    }
    let mut todo = String::new();
    for i in 1..=n {
        match leer(&nombre_parte(nombre, i))? {
            Some(p) => todo.push_str(&p.0),
            None => return Ok(None),
        }
    }
    Ok(Some(Secreto(todo)))
}

fn borrar_partes(nombre: &str) {
    for i in 1..=PARTES_MAX {
        if let Ok(e) = entrada(&nombre_parte(nombre, i)) {
            let _ = e.delete_credential();
        }
    }
}

pub fn borrar_largo(nombre: &str) -> Result<(), String> {
    borrar_partes(nombre);
    borrar(nombre)
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

    struct Credencial(String);

    impl CredentialApi for Credencial {
        fn set_secret(&self, secreto: &[u8]) -> keyring::Result<()> {
            ALMACEN.lock().insert(self.0.clone(), secreto.to_vec());
            Ok(())
        }
        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
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
    fn un_valor_largo_se_reparte_en_partes_y_se_reconstruye() {
        let nombre = "prueba/largo";
        let valor: String = (0..4_000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        guardar_largo(nombre, &valor).unwrap();
        assert!(existe(nombre));
        let partes = mock::objetivos().iter().filter(|o| o.starts_with("Glassdeck/prueba/largo/")).count();
        assert_eq!(partes, 4);
        assert_eq!(leer_largo(nombre).unwrap().unwrap().exponer(), valor);
        guardar_largo(nombre, "corto-de-prueba").unwrap();
        assert_eq!(leer_largo(nombre).unwrap().unwrap().exponer(), "corto-de-prueba");
        assert!(!mock::objetivos().iter().any(|o| o.starts_with("Glassdeck/prueba/largo/")), "las partes viejas se borran");
        borrar_largo(nombre).unwrap();
        assert!(leer_largo(nombre).unwrap().is_none());
        assert!(guardar_largo(nombre, &"x".repeat(LARGO_PARTE * PARTES_MAX + 1)).is_err());
    }
}

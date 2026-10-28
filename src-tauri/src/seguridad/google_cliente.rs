use super::boveda::{self, Clave};
use serde::{Deserialize, Serialize};

/// Un JSON de cliente de Google pesa ~400 bytes; el tope evita leer un archivo enorme elegido por error.
pub const TAMANO_MAX_ARCHIVO: u64 = 64 * 1024;
const SUFIJO_CLIENT_ID: &str = ".apps.googleusercontent.com";

#[derive(Deserialize)]
struct Archivo {
    installed: Option<Datos>,
    web: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Datos {
    client_id: Option<String>,
    client_secret: Option<String>,
}

/// Lo que se guarda en la bóveda (GD-MAIL-07); no implementa `Debug` para no imprimir el secreto.
#[derive(Serialize, Deserialize)]
pub struct ClienteOAuth {
    pub client_id: String,
    pub client_secret: String,
}

fn texto_valido(s: &str, largo_max: usize) -> bool {
    !s.is_empty() && s.len() <= largo_max && s.chars().all(|c| c.is_ascii_graphic())
}

pub fn validar(texto: &str) -> Result<ClienteOAuth, String> {
    let archivo: Archivo =
        serde_json::from_str(texto).map_err(|_| "El archivo no es el JSON del cliente de Google".to_string())?;
    let Some(datos) = archivo.installed else {
        return Err(if archivo.web.is_some() {
            "Ese cliente es de tipo web; crea uno de tipo «App de escritorio» (guía, paso 6)".into()
        } else {
            "El archivo no es el JSON de un cliente de escritorio de Google".into()
        });
    };
    let client_id = datos.client_id.unwrap_or_default().trim().to_string();
    let client_secret = datos.client_secret.unwrap_or_default().trim().to_string();
    let id_valido = texto_valido(&client_id, 200)
        && client_id.ends_with(SUFIJO_CLIENT_ID)
        && client_id.len() > SUFIJO_CLIENT_ID.len();
    if !id_valido {
        return Err("El ID del cliente no tiene el formato de Google".into());
    }
    if !texto_valido(&client_secret, 200) {
        return Err("Falta el secreto del cliente en el archivo".into());
    }
    Ok(ClienteOAuth { client_id, client_secret })
}

pub fn importar(texto: &str) -> Result<(), String> {
    let cliente = validar(texto)?;
    let valor = serde_json::to_string(&cliente).map_err(|_| "No se pudo preparar el cliente".to_string())?;
    boveda::guardar(Clave::GoogleCliente.nombre(), &valor)
}

pub fn importar_archivo(ruta: &std::path::Path) -> Result<(), String> {
    let tamano = std::fs::metadata(ruta).map_err(|_| "No se pudo leer el archivo".to_string())?.len();
    if tamano > TAMANO_MAX_ARCHIVO {
        return Err("El archivo es demasiado grande para ser un cliente de Google".into());
    }
    let texto = std::fs::read_to_string(ruta).map_err(|_| "No se pudo leer el archivo".to_string())?;
    importar(&texto)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ID: &str = "000000000000-prueba.apps.googleusercontent.com";

    fn json_escritorio(id: &str, secreto: &str) -> String {
        serde_json::json!({
            "installed": {
                "client_id": id,
                "project_id": "proyecto-de-prueba",
                "auth_uri": "https://accounts.google.com/o/oauth2/auth",
                "token_uri": "https://oauth2.googleapis.com/token",
                "client_secret": secreto,
                "redirect_uris": ["http://localhost"]
            }
        })
        .to_string()
    }

    #[test]
    fn un_cliente_de_escritorio_queda_en_la_boveda() {
        boveda::borrar(Clave::GoogleCliente.nombre()).unwrap();
        importar(&json_escritorio(ID, "secreto-sintetico-1")).unwrap();
        assert!(boveda::existe(Clave::GoogleCliente.nombre()));
        let guardado = boveda::leer(Clave::GoogleCliente.nombre()).unwrap().unwrap();
        let cliente: ClienteOAuth = serde_json::from_str(guardado.exponer()).unwrap();
        assert_eq!(cliente.client_id, ID);
        assert_eq!(cliente.client_secret, "secreto-sintetico-1");
        assert!(!guardado.exponer().contains("project_id"));
        assert!(importar("{}").is_err());
        let sigue = boveda::leer(Clave::GoogleCliente.nombre()).unwrap().unwrap();
        assert_eq!(sigue.exponer(), guardado.exponer());
        boveda::borrar(Clave::GoogleCliente.nombre()).unwrap();
    }

    #[test]
    fn rechaza_clientes_web_y_archivos_ajenos() {
        let web = serde_json::json!({ "web": { "client_id": ID, "client_secret": "x" } }).to_string();
        assert!(validar(&web).err().is_some_and(|e| e.contains("tipo web")));
        assert!(validar("{\"otra\":1}").is_err());
        assert!(validar("no es json").is_err());
        assert!(validar("[]").is_err());
    }

    #[test]
    fn rechaza_id_o_secreto_invalidos() {
        assert!(validar(&json_escritorio("123-prueba.example.com", "s")).is_err());
        assert!(validar(&json_escritorio(SUFIJO_CLIENT_ID, "s")).is_err());
        assert!(validar(&json_escritorio(ID, "")).is_err());
        assert!(validar(&json_escritorio(ID, "con espacio")).is_err());
        assert!(validar(&json_escritorio(&format!("{}{SUFIJO_CLIENT_ID}", "a".repeat(200)), "s")).is_err());
        assert!(validar(&json_escritorio(&format!(" {ID} "), " s ")).is_ok());
    }

    #[test]
    fn un_archivo_demasiado_grande_no_se_lee() {
        let dir = std::env::temp_dir().join(format!("gd-prueba-oauth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("grande.json");
        std::fs::write(&ruta, " ".repeat(TAMANO_MAX_ARCHIVO as usize + 1)).unwrap();
        assert!(importar_archivo(&ruta).unwrap_err().contains("demasiado grande"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

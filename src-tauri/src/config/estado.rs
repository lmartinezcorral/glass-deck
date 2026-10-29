use super::almacen::{cargar_json, guardar};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const VERSION_ESTADO: u32 = 1;
const VIGENCIA_AVISOS_MS: i64 = 400 * 24 * 3_600 * 1_000;

pub fn ahora_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Solo fuentes sin datos personales (clima, límites de gasto); nunca correo, agenda ni secretos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UltimoDato {
    pub hora_ms: i64,
    pub datos: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Estado {
    pub version: u32,
    pub ultimos: BTreeMap<String, UltimoDato>,
    /// Clave del aviso (fuente, ciclo y umbral) y hora de envío.
    pub avisos: BTreeMap<String, i64>,
    /// Servicios web conectados (Cursor, claude.ai, ChatGPT): solo la marca; la sesión vive en su perfil de WebView2.
    pub sesiones_web: BTreeSet<String>,
}

impl Default for Estado {
    fn default() -> Self {
        Self { version: VERSION_ESTADO, ultimos: BTreeMap::new(), avisos: BTreeMap::new(), sesiones_web: BTreeSet::new() }
    }
}

pub struct AlmacenEstado {
    ruta: PathBuf,
    actual: Mutex<Estado>,
}

impl AlmacenEstado {
    pub fn nuevo(dir: &Path) -> Self {
        let ruta = dir.join("state.json");
        let actual = cargar_json(&ruta).unwrap_or_default();
        Self { ruta, actual: Mutex::new(actual) }
    }

    pub fn leer(&self) -> Estado {
        self.actual.lock().clone()
    }

    pub fn ultimo(&self, fuente: &str) -> Option<UltimoDato> {
        self.actual.lock().ultimos.get(fuente).cloned()
    }

    pub fn modificar<F: FnOnce(&mut Estado)>(&self, f: F) -> Result<(), String> {
        let mut e = self.actual.lock();
        f(&mut e);
        e.version = VERSION_ESTADO;
        guardar(&self.ruta, &*e).map_err(|err| format!("No se pudo guardar state.json: {err}"))
    }

    pub fn guardar_ultimo(&self, fuente: &str, dato: UltimoDato) -> Result<(), String> {
        self.modificar(|e| {
            e.ultimos.insert(fuente.to_string(), dato);
        })
    }

    pub fn sesion_web(&self, servicio: &str) -> bool {
        self.actual.lock().sesiones_web.contains(servicio)
    }

    pub fn marcar_sesion_web(&self, servicio: &str, conectada: bool) -> Result<(), String> {
        if self.sesion_web(servicio) == conectada {
            return Ok(());
        }
        self.modificar(|e| {
            if conectada {
                e.sesiones_web.insert(servicio.to_string());
            } else {
                e.sesiones_web.remove(servicio);
            }
        })
    }

    pub fn registrar_avisos(&self, claves: &[String], ahora: i64) -> Result<(), String> {
        if claves.is_empty() {
            return Ok(());
        }
        self.modificar(|e| {
            e.avisos.retain(|_, enviado| ahora - *enviado < VIGENCIA_AVISOS_MS);
            for c in claves {
                e.avisos.insert(c.clone(), ahora);
            }
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::dir_temporal;

    #[test]
    fn el_ultimo_dato_sobrevive_a_un_reinicio() {
        let d = dir_temporal("estado-ultimo");
        let a = AlmacenEstado::nuevo(&d);
        assert!(a.ultimo("clima").is_none());
        let dato = UltimoDato { hora_ms: 1_000, datos: serde_json::json!({ "temperatura": 21.5 }) };
        a.guardar_ultimo("clima", dato.clone()).unwrap();
        drop(a);
        let b = AlmacenEstado::nuevo(&d);
        assert_eq!(b.ultimo("clima"), Some(dato));
        assert!(d.join("state.json").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn los_avisos_persisten_y_los_muy_viejos_se_descartan() {
        let d = dir_temporal("estado-avisos");
        let a = AlmacenEstado::nuevo(&d);
        a.registrar_avisos(&["viejo".into()], 0).unwrap();
        let despues = VIGENCIA_AVISOS_MS + 10;
        a.registrar_avisos(&["cursor:2026-10:80".into()], despues).unwrap();
        drop(a);
        let e = AlmacenEstado::nuevo(&d).leer();
        assert_eq!(e.avisos.get("cursor:2026-10:80"), Some(&despues));
        assert!(!e.avisos.contains_key("viejo"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_marca_de_sesion_web_persiste_y_un_state_json_anterior_la_trae_vacia() {
        let d = dir_temporal("estado-sesiones");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("state.json"), br#"{ "version": 1, "ultimos": {}, "avisos": { "x": 5 } }"#).unwrap();
        let a = AlmacenEstado::nuevo(&d);
        assert!(a.leer().sesiones_web.is_empty());
        assert_eq!(a.leer().avisos.get("x"), Some(&5));
        a.marcar_sesion_web("cursor", true).unwrap();
        drop(a);
        let b = AlmacenEstado::nuevo(&d);
        assert!(b.sesion_web("cursor") && !b.sesion_web("chatgpt"));
        assert_eq!(b.leer().version, VERSION_ESTADO);
        b.marcar_sesion_web("cursor", false).unwrap();
        assert!(!AlmacenEstado::nuevo(&d).sesion_web("cursor"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn state_json_danado_usa_el_respaldo() {
        let d = dir_temporal("estado-corrupto");
        let a = AlmacenEstado::nuevo(&d);
        a.registrar_avisos(&["x".into()], 5).unwrap();
        a.registrar_avisos(&["y".into()], 6).unwrap();
        drop(a);
        std::fs::write(d.join("state.json"), b"{ roto").unwrap();
        let e = AlmacenEstado::nuevo(&d).leer();
        assert!(e.avisos.contains_key("x"));
        let _ = std::fs::remove_dir_all(&d);
    }
}

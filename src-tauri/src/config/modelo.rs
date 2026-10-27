use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION_CONFIG: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoWidget {
    Normal,
    Pildora,
    Oculto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capa {
    Escritorio,
    SiempreEncima,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tema {
    Oscuro,
    Claro,
    Sistema,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigWidget {
    pub activo: bool,
    pub estado: EstadoWidget,
    pub fijado: bool,
    pub monitor: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub ancho: u32,
    pub alto: u32,
}

impl Default for ConfigWidget {
    fn default() -> Self {
        Self {
            activo: true,
            estado: EstadoWidget::Normal,
            fijado: false,
            monitor: None,
            x: None,
            y: None,
            ancho: 240,
            alto: 200,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Intervalos {
    pub cpu_ram_red_ms: u64,
    pub procesos_ms: u64,
    pub disco_ms: u64,
}

impl Default for Intervalos {
    fn default() -> Self {
        Self { cpu_ram_red_ms: 2_000, procesos_ms: 3_000, disco_ms: 5_000 }
    }
}

impl Intervalos {
    pub const MIN_CPU_RAM_RED_MS: u64 = 1_000;
    pub const MIN_PROCESOS_MS: u64 = 2_000;
    pub const MIN_DISCO_MS: u64 = 1_000;

    pub fn validar(&self) -> Result<(), String> {
        if self.cpu_ram_red_ms < Self::MIN_CPU_RAM_RED_MS {
            return Err("El intervalo de CPU, RAM y red no puede ser menor a 1 s".into());
        }
        if self.procesos_ms < Self::MIN_PROCESOS_MS {
            return Err("El intervalo de procesos no puede ser menor a 2 s".into());
        }
        if self.disco_ms < Self::MIN_DISCO_MS {
            return Err("El intervalo de disco no puede ser menor a 1 s".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Atajos {
    pub mostrar_ocultar: String,
    pub privacidad: String,
}

impl Default for Atajos {
    fn default() -> Self {
        Self { mostrar_ocultar: "Super+Alt+G".into(), privacidad: "Super+Alt+P".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Apariencia {
    pub tema: Tema,
    pub acento: String,
    pub opacidad: f32,
    pub desenfoque: f32,
}

impl Default for Apariencia {
    fn default() -> Self {
        Self { tema: Tema::Oscuro, acento: "#22d3ee".into(), opacidad: 0.44, desenfoque: 24.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigDock {
    pub visible: bool,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub monitor: Option<String>,
}

impl Default for ConfigDock {
    fn default() -> Self {
        Self { visible: true, x: None, y: None, monitor: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub widgets: BTreeMap<String, ConfigWidget>,
    pub dock: ConfigDock,
    pub apariencia: Apariencia,
    pub intervalos: Intervalos,
    pub lista_negra_usuario: Vec<String>,
    pub atajos: Atajos,
    pub arranque_windows: bool,
    pub capa_defecto: Capa,
    pub pausar_con_bateria: bool,
    pub iman: bool,
    pub privacidad: bool,
}

impl Default for Config {
    fn default() -> Self {
        let mut widgets = BTreeMap::new();
        for w in crate::ventanas::catalogo::WIDGETS_FASE_1 {
            widgets.insert(
                w.id.to_string(),
                ConfigWidget { ancho: w.ancho, alto: w.alto, ..ConfigWidget::default() },
            );
        }
        Self {
            version: VERSION_CONFIG,
            widgets,
            dock: ConfigDock::default(),
            apariencia: Apariencia::default(),
            intervalos: Intervalos::default(),
            lista_negra_usuario: Vec::new(),
            atajos: Atajos::default(),
            arranque_windows: true,
            capa_defecto: Capa::Escritorio,
            pausar_con_bateria: true,
            iman: false,
            privacidad: false,
        }
    }
}

impl Config {
    pub fn normalizar(&mut self) {
        let base = Config::default();
        for (id, w) in base.widgets {
            let actual = self.widgets.entry(id).or_insert_with(|| w.clone());
            actual.ancho = actual.ancho.max(w.ancho);
            actual.alto = actual.alto.max(w.alto);
        }
        if self.intervalos.validar().is_err() {
            self.intervalos = Intervalos::default();
        }
        self.apariencia.opacidad = self.apariencia.opacidad.clamp(0.2, 0.95);
        self.apariencia.desenfoque = self.apariencia.desenfoque.clamp(0.0, 80.0);
        self.lista_negra_usuario.retain(|n| !n.trim().is_empty());
        self.version = VERSION_CONFIG;
    }

    pub fn widget(&self, id: &str) -> ConfigWidget {
        self.widgets.get(id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn normalizar_sube_tamanos_guardados_al_minimo_del_catalogo() {
        let mut c = Config::default();
        c.widgets.get_mut("red").unwrap().alto = 120;
        c.widgets.get_mut("red").unwrap().ancho = 900;
        c.normalizar();
        let red = crate::ventanas::catalogo::buscar("red").unwrap();
        assert_eq!(c.widget("red").alto, red.alto);
        assert_eq!(c.widget("red").ancho, 900);
    }

    #[test]
    fn config_por_defecto_trae_widgets_de_fase_1() {
        let c = Config::default();
        for id in ["cpu", "ram", "disco", "red", "procesos"] {
            assert!(c.widgets.contains_key(id), "falta {id}");
        }
    }

    #[test]
    fn json_parcial_se_completa_con_valores_por_defecto() {
        let c: Config = serde_json::from_str(r#"{"iman": true}"#).unwrap();
        assert!(c.iman);
        assert_eq!(c.intervalos, Intervalos::default());
    }

    #[test]
    fn intervalos_menores_al_minimo_se_rechazan() {
        let i = Intervalos { cpu_ram_red_ms: 500, ..Intervalos::default() };
        assert!(i.validar().is_err());
        let i = Intervalos { procesos_ms: 1_000, ..Intervalos::default() };
        assert!(i.validar().is_err());
        assert!(Intervalos::default().validar().is_ok());
    }

    #[test]
    fn normalizar_restaura_widgets_faltantes_e_intervalos_invalidos() {
        let mut c = Config::default();
        c.widgets.remove("cpu");
        c.intervalos.cpu_ram_red_ms = 10;
        c.normalizar();
        assert!(c.widgets.contains_key("cpu"));
        assert_eq!(c.intervalos, Intervalos::default());
    }
}

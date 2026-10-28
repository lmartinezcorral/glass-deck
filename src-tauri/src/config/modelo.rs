use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION_CONFIG: u32 = 3;

/// Valor de fábrica de la versión 1; la Xbox Game Bar lo ocupa en Windows 10 (decisión D-22).
const ATAJO_MOSTRAR_V1: &str = "Super+Alt+G";

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
    pub spending_ms: u64,
    pub clima_ms: u64,
    pub correo_ms: u64,
    pub calendario_ms: u64,
    /// Cada cuánto cambia la alerta del widget de correo (GD-ROT-01).
    pub rotacion_ms: u64,
}

impl Default for Intervalos {
    fn default() -> Self {
        Self {
            cpu_ram_red_ms: 2_000,
            procesos_ms: 3_000,
            disco_ms: 5_000,
            spending_ms: 900_000,
            clima_ms: 1_800_000,
            correo_ms: 300_000,
            calendario_ms: 600_000,
            rotacion_ms: 600_000,
        }
    }
}

impl Intervalos {
    pub const MIN_CPU_RAM_RED_MS: u64 = 1_000;
    pub const MIN_PROCESOS_MS: u64 = 2_000;
    pub const MIN_DISCO_MS: u64 = 1_000;
    pub const MIN_SPENDING_MS: u64 = 300_000;
    pub const MIN_CLIMA_MS: u64 = 900_000;
    pub const MIN_CORREO_MS: u64 = 120_000;
    pub const MIN_CALENDARIO_MS: u64 = 300_000;
    pub const MIN_ROTACION_MS: u64 = 60_000;

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
        if self.spending_ms < Self::MIN_SPENDING_MS {
            return Err("El intervalo de spending no puede ser menor a 5 min".into());
        }
        if self.clima_ms < Self::MIN_CLIMA_MS {
            return Err("El intervalo del clima no puede ser menor a 15 min".into());
        }
        if self.correo_ms < Self::MIN_CORREO_MS {
            return Err("El intervalo del correo no puede ser menor a 2 min".into());
        }
        if self.calendario_ms < Self::MIN_CALENDARIO_MS {
            return Err("El intervalo de la agenda no puede ser menor a 5 min".into());
        }
        if self.rotacion_ms < Self::MIN_ROTACION_MS {
            return Err("La rotación de la alerta no puede ser menor a 1 min".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModoUbicacion {
    Automatica,
    Manual,
}

/// Ciudad elegida en el buscador; las coordenadas solo viajan a Open-Meteo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ubicacion {
    pub nombre: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub pais: Option<String>,
    pub latitud: f64,
    pub longitud: f64,
}

impl Ubicacion {
    pub fn validar(&self) -> Result<(), String> {
        let largo_ok = |t: &str| !t.trim().is_empty() && t.chars().count() <= 120;
        if !largo_ok(&self.nombre) || ![&self.region, &self.pais].iter().all(|o| o.as_deref().map(largo_ok).unwrap_or(true)) {
            return Err("El nombre de la ubicación no es válido".into());
        }
        let lat_ok = self.latitud.is_finite() && (-90.0..=90.0).contains(&self.latitud);
        let lon_ok = self.longitud.is_finite() && (-180.0..=180.0).contains(&self.longitud);
        if !lat_ok || !lon_ok {
            return Err("Las coordenadas de la ubicación no son válidas".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigClima {
    pub modo: ModoUbicacion,
    pub ubicacion: Option<Ubicacion>,
}

impl Default for ConfigClima {
    fn default() -> Self {
        Self { modo: ModoUbicacion::Automatica, ubicacion: None }
    }
}

/// Precios en USD por millón de tokens. `patron` es un fragmento del nombre del modelo; gana el más largo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrecioModelo {
    pub patron: String,
    pub entrada: f64,
    pub salida: f64,
    pub cache_escritura: f64,
    pub cache_lectura: f64,
}

/// Valores de referencia por familia (escritura de caché = 1.25 × entrada, lectura = 0.1 × entrada); Luis los edita en Customize.
pub fn precios_por_defecto() -> Vec<PrecioModelo> {
    let p = |patron: &str, entrada: f64, salida: f64| PrecioModelo {
        patron: patron.into(),
        entrada,
        salida,
        cache_escritura: entrada * 1.25,
        cache_lectura: entrada * 0.1,
    };
    vec![p("opus", 5.0, 25.0), p("sonnet", 3.0, 15.0), p("haiku", 1.0, 5.0)]
}

fn precio_valido(p: &PrecioModelo) -> bool {
    !p.patron.is_empty() && [p.entrada, p.salida, p.cache_escritura, p.cache_lectura].iter().all(|v| v.is_finite() && *v >= 0.0)
}

/// Porcentajes que disparan un aviso (GD-CUST-04); una lista vacía apaga los avisos de ese medidor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Umbrales {
    pub notificaciones: bool,
    pub cursor: Vec<u8>,
    pub claude_5h: Vec<u8>,
    pub claude_semanal: Vec<u8>,
    /// Si está definido, el porcentaje de Cursor se mide contra este presupuesto y no contra el límite del plan.
    pub presupuesto_cursor_usd: Option<f64>,
    pub chatgpt_5h: Vec<u8>,
    pub chatgpt_semanal: Vec<u8>,
}

impl Default for Umbrales {
    fn default() -> Self {
        Self {
            notificaciones: true,
            cursor: vec![50, 80, 100],
            claude_5h: vec![80],
            claude_semanal: vec![80],
            presupuesto_cursor_usd: None,
            chatgpt_5h: vec![80],
            chatgpt_semanal: vec![80],
        }
    }
}

impl Umbrales {
    const MAX_POR_MEDIDOR: usize = 5;

    fn normalizar(&mut self) {
        for lista in [&mut self.cursor, &mut self.claude_5h, &mut self.claude_semanal, &mut self.chatgpt_5h, &mut self.chatgpt_semanal] {
            lista.retain(|u| (1..=100).contains(u));
            lista.sort_unstable();
            lista.dedup();
            lista.truncate(Self::MAX_POR_MEDIDOR);
        }
        self.presupuesto_cursor_usd = self.presupuesto_cursor_usd.filter(|p| p.is_finite() && *p > 0.0);
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
        Self { mostrar_ocultar: "Ctrl+Alt+M".into(), privacidad: "Super+Alt+P".into() }
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
    pub precios_modelos: Vec<PrecioModelo>,
    pub clima: ConfigClima,
    pub umbrales: Umbrales,
}

impl Default for Config {
    fn default() -> Self {
        let mut widgets = BTreeMap::new();
        for w in crate::ventanas::catalogo::WIDGETS {
            widgets.insert(
                w.id.to_string(),
                ConfigWidget { ancho: w.ancho, alto: w.alto, activo: w.activo_defecto, ..ConfigWidget::default() },
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
            precios_modelos: precios_por_defecto(),
            clima: ConfigClima::default(),
            umbrales: Umbrales::default(),
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
        for p in self.precios_modelos.iter_mut() {
            p.patron = p.patron.trim().to_lowercase();
        }
        self.precios_modelos.retain(precio_valido);
        if self.clima.ubicacion.as_ref().map(|u| u.validar().is_err()).unwrap_or(false) {
            self.clima.ubicacion = None;
        }
        if self.clima.ubicacion.is_none() {
            self.clima.modo = ModoUbicacion::Automatica;
        }
        self.umbrales.normalizar();
        if self.version < 2 && self.atajos.mostrar_ocultar == ATAJO_MOSTRAR_V1 {
            self.atajos.mostrar_ocultar = Atajos::default().mostrar_ocultar;
        }
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
    fn la_version_1_migra_el_atajo_de_fabrica_y_respeta_uno_propio() {
        let mut de_fabrica = Config { version: 1, ..Config::default() };
        de_fabrica.atajos.mostrar_ocultar = ATAJO_MOSTRAR_V1.into();
        de_fabrica.normalizar();
        assert_eq!(de_fabrica.atajos.mostrar_ocultar, "Ctrl+Alt+M");
        assert_eq!(de_fabrica.version, VERSION_CONFIG);

        let mut propio = Config { version: 1, ..Config::default() };
        propio.atajos.mostrar_ocultar = "Ctrl+Shift+F9".into();
        propio.normalizar();
        assert_eq!(propio.atajos.mostrar_ocultar, "Ctrl+Shift+F9");

        let mut elegido_despues = Config::default();
        elegido_despues.atajos.mostrar_ocultar = ATAJO_MOSTRAR_V1.into();
        elegido_despues.normalizar();
        assert_eq!(elegido_despues.atajos.mostrar_ocultar, ATAJO_MOSTRAR_V1);
    }

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
    fn intervalos_por_defecto_son_los_de_la_spec() {
        let i = Config::default().intervalos;
        assert_eq!(i.cpu_ram_red_ms, 2_000);
        assert_eq!(i.procesos_ms, 3_000);
        assert_eq!(i.disco_ms, 5_000);
    }

    #[test]
    fn intervalos_en_el_minimo_se_aceptan() {
        let i = Intervalos {
            cpu_ram_red_ms: 1_000,
            procesos_ms: 2_000,
            disco_ms: 1_000,
            spending_ms: 300_000,
            clima_ms: 900_000,
            correo_ms: 120_000,
            calendario_ms: 300_000,
            rotacion_ms: 60_000,
        };
        assert!(i.validar().is_ok());
        let i = Intervalos { correo_ms: 119_999, ..Intervalos::default() };
        assert!(i.validar().is_err());
        let i = Intervalos { rotacion_ms: 59_999, ..Intervalos::default() };
        assert!(i.validar().is_err());
        let i = Intervalos { spending_ms: 299_999, ..Intervalos::default() };
        assert!(i.validar().is_err());
        let i = Intervalos { clima_ms: 899_999, ..Intervalos::default() };
        assert!(i.validar().is_err());
        let i = Intervalos { disco_ms: 999, ..Intervalos::default() };
        assert!(i.validar().is_err());
    }

    #[test]
    fn intervalos_menores_al_minimo_se_rechazan() {
        let i = Intervalos { cpu_ram_red_ms: 500, ..Intervalos::default() };
        assert!(i.validar().is_err());
        let i = Intervalos { procesos_ms: 1_000, ..Intervalos::default() };
        assert!(i.validar().is_err());
        assert!(Intervalos::default().validar().is_ok());
    }

    const CONFIG_V2: &str = r#"{
        "version": 2,
        "widgets": {
            "cpu": { "activo": false, "estado": "pildora", "fijado": true, "monitor": "\\\\.\\DISPLAY2", "x": -1500, "y": 40, "ancho": 300, "alto": 260 },
            "ram": { "activo": true, "estado": "normal", "fijado": false, "monitor": null, "x": null, "y": null, "ancho": 220, "alto": 210 }
        },
        "intervalos": { "cpu_ram_red_ms": 3000, "procesos_ms": 4000, "disco_ms": 6000 },
        "lista_negra_usuario": ["spotify"],
        "atajos": { "mostrar_ocultar": "Ctrl+Shift+F9", "privacidad": "Super+Alt+P" },
        "arranque_windows": false,
        "iman": true
    }"#;

    #[test]
    fn la_version_2_migra_a_3_respetando_lo_del_usuario() {
        let mut c: Config = serde_json::from_str(CONFIG_V2).unwrap();
        c.normalizar();
        assert_eq!(c.version, 3);
        let cpu = c.widget("cpu");
        assert!(!cpu.activo);
        assert_eq!((cpu.x, cpu.y, cpu.ancho, cpu.estado), (Some(-1500), Some(40), 300, EstadoWidget::Pildora));
        assert_eq!(c.intervalos.cpu_ram_red_ms, 3_000);
        assert_eq!(c.intervalos.procesos_ms, 4_000);
        assert_eq!(c.intervalos.disco_ms, 6_000);
        assert_eq!(c.intervalos.spending_ms, 900_000);
        assert_eq!(c.atajos.mostrar_ocultar, "Ctrl+Shift+F9");
        assert_eq!(c.lista_negra_usuario, vec!["spotify".to_string()]);
        assert!(!c.arranque_windows && c.iman);
        for id in ["spending", "clima", "correo"] {
            assert!(c.widgets.contains_key(id), "falta {id}");
            assert!(!c.widget(id).activo, "los widgets nuevos llegan apagados");
        }
        assert_eq!(c.precios_modelos, precios_por_defecto());
        assert_eq!(c.intervalos.clima_ms, 1_800_000);
        assert_eq!((c.intervalos.correo_ms, c.intervalos.calendario_ms, c.intervalos.rotacion_ms), (300_000, 600_000, 600_000));
        assert_eq!(c.clima, ConfigClima::default());
        assert_eq!(c.umbrales, Umbrales::default());
    }

    #[test]
    fn los_umbrales_por_defecto_son_los_de_la_spec() {
        let u = Umbrales::default();
        assert_eq!(u.cursor, vec![50, 80, 100]);
        assert_eq!((u.claude_5h.as_slice(), u.claude_semanal.as_slice()), (&[80u8][..], &[80u8][..]));
        assert_eq!((u.chatgpt_5h.as_slice(), u.chatgpt_semanal.as_slice()), (&[80u8][..], &[80u8][..]));
        assert!(u.notificaciones && u.presupuesto_cursor_usd.is_none());
    }

    #[test]
    fn umbrales_guardados_sin_chatgpt_reciben_los_de_fabrica_sin_cambiar_la_version() {
        let mut c: Config =
            serde_json::from_str(r#"{"version": 3, "umbrales": {"notificaciones": true, "cursor": [90], "claude_5h": [], "claude_semanal": [70]}}"#).unwrap();
        c.normalizar();
        assert_eq!(c.version, VERSION_CONFIG);
        assert_eq!(c.umbrales.cursor, vec![90]);
        assert!(c.umbrales.claude_5h.is_empty());
        assert_eq!((c.umbrales.chatgpt_5h.as_slice(), c.umbrales.chatgpt_semanal.as_slice()), (&[80u8][..], &[80u8][..]));
    }

    #[test]
    fn normalizar_ordena_umbrales_y_descarta_presupuestos_invalidos() {
        let mut c = Config::default();
        c.umbrales.cursor = vec![100, 0, 80, 80, 150, 50, 10, 20, 30];
        c.umbrales.claude_5h = vec![];
        c.umbrales.presupuesto_cursor_usd = Some(-5.0);
        c.normalizar();
        assert_eq!(c.umbrales.cursor, vec![10, 20, 30, 50, 80]);
        assert!(c.umbrales.claude_5h.is_empty(), "una lista vacía apaga ese medidor");
        assert_eq!(c.umbrales.presupuesto_cursor_usd, None);
        c.umbrales.presupuesto_cursor_usd = Some(40.0);
        c.normalizar();
        assert_eq!(c.umbrales.presupuesto_cursor_usd, Some(40.0));
    }

    fn chihuahua() -> Ubicacion {
        Ubicacion { nombre: "Chihuahua".into(), region: Some("Chihuahua".into()), pais: Some("México".into()), latitud: 28.63528, longitud: -106.08889 }
    }

    #[test]
    fn la_ubicacion_manual_sobrevive_a_normalizar() {
        let mut c = Config::default();
        c.clima = ConfigClima { modo: ModoUbicacion::Manual, ubicacion: Some(chihuahua()) };
        let json = serde_json::to_string(&c).unwrap();
        let mut leida: Config = serde_json::from_str(&json).unwrap();
        leida.normalizar();
        assert_eq!(leida.clima, c.clima);
    }

    #[test]
    fn una_ubicacion_invalida_vuelve_al_modo_automatico() {
        for mala in [
            Ubicacion { latitud: 91.0, ..chihuahua() },
            Ubicacion { longitud: f64::NAN, ..chihuahua() },
            Ubicacion { nombre: "  ".into(), ..chihuahua() },
            Ubicacion { pais: Some("x".repeat(121)), ..chihuahua() },
        ] {
            let mut c = Config::default();
            c.clima = ConfigClima { modo: ModoUbicacion::Manual, ubicacion: Some(mala) };
            c.normalizar();
            assert_eq!(c.clima, ConfigClima::default());
        }
        let mut sin_ciudad = Config::default();
        sin_ciudad.clima.modo = ModoUbicacion::Manual;
        sin_ciudad.normalizar();
        assert_eq!(sin_ciudad.clima.modo, ModoUbicacion::Automatica);
    }

    #[test]
    fn la_version_1_migra_a_3_con_el_atajo_nuevo() {
        let mut c: Config = serde_json::from_str(&CONFIG_V2.replace("\"version\": 2", "\"version\": 1").replace("Ctrl+Shift+F9", ATAJO_MOSTRAR_V1)).unwrap();
        c.normalizar();
        assert_eq!(c.version, 3);
        assert_eq!(c.atajos.mostrar_ocultar, "Ctrl+Alt+M");
        assert_eq!(c.intervalos.cpu_ram_red_ms, 3_000);
        assert!(!c.widget("spending").activo && !c.widget("clima").activo);
        assert_eq!(c.intervalos.spending_ms, 900_000);
        assert_eq!(c.clima.modo, ModoUbicacion::Automatica);
    }

    #[test]
    fn los_widgets_de_servicios_arrancan_apagados() {
        let c = Config::default();
        assert!(!c.widget("spending").activo);
        assert!(!c.widget("clima").activo);
        assert!(!c.widget("correo").activo);
        assert!(c.widget("cpu").activo);
    }

    #[test]
    fn normalizar_limpia_precios_invalidos_y_respeta_los_editados() {
        let mut c = Config::default();
        c.precios_modelos[0].entrada = 7.5;
        c.precios_modelos.push(PrecioModelo { patron: "  Mi-Modelo ".into(), entrada: 1.0, salida: 2.0, cache_escritura: 0.0, cache_lectura: 0.0 });
        c.precios_modelos.push(PrecioModelo { patron: "roto".into(), entrada: -1.0, salida: 2.0, cache_escritura: 0.0, cache_lectura: 0.0 });
        c.precios_modelos.push(PrecioModelo { patron: " ".into(), entrada: 1.0, salida: 2.0, cache_escritura: 0.0, cache_lectura: 0.0 });
        c.normalizar();
        assert_eq!(c.precios_modelos.len(), 4);
        assert_eq!(c.precios_modelos[0].entrada, 7.5);
        assert_eq!(c.precios_modelos[3].patron, "mi-modelo");
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

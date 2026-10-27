use std::collections::HashSet;

use serde::Serialize;

pub const CONFIRMACION_REFORZADA: &str = "CERRAR";

pub const PIDS_PROTEGIDOS: [u32; 2] = [0, 4];

pub const PROTEGIDOS_FIJOS: [&str; 20] = [
    "System",
    "Idle",
    "Registry",
    "smss",
    "csrss",
    "wininit",
    "winlogon",
    "services",
    "lsass",
    "svchost",
    "dwm",
    "fontdrvhost",
    "Memory Compression",
    "MsMpEng",
    "audiodg",
    "spoolsv",
    "sihost",
    "ctfmon",
    "conhost",
    "explorer",
];

pub const REFORZADOS_FIJOS: [&str; 9] = [
    "Cursor",
    "glassdeck",
    "claude",
    "Code",
    "WindowsTerminal",
    "cmd",
    "pwsh",
    "powershell",
    "node",
];

const EXPLORER: &str = "explorer";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Nivel {
    Protegido,
    Reforzado,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dueno {
    UsuarioActual,
    Sistema,
    OtroUsuario,
    Desconocido,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidato {
    pub pid: u32,
    pub nombre: String,
    pub dueno: Dueno,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rechazo {
    Protegido,
    RequiereConfirmacion,
    YaNoExiste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultadoCierre {
    Cerrado,
    Rechazado,
    SinPermiso,
    YaNoExistia,
}

impl From<Rechazo> for ResultadoCierre {
    fn from(r: Rechazo) -> Self {
        match r {
            Rechazo::Protegido | Rechazo::RequiereConfirmacion => ResultadoCierre::Rechazado,
            Rechazo::YaNoExiste => ResultadoCierre::YaNoExistia,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rechazado {
    pub candidato: Candidato,
    pub motivo: Rechazo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Validacion {
    pub permitidos: Vec<Candidato>,
    pub rechazados: Vec<Rechazado>,
}

#[derive(Debug, Clone)]
pub struct Politica {
    protegidos: HashSet<String>,
    reforzados: HashSet<String>,
    lista_negra_usuario: HashSet<String>,
    pid_propio: u32,
}

pub fn normalizar(nombre: &str) -> String {
    let minus = nombre.trim().to_lowercase();
    match minus.strip_suffix(".exe") {
        Some(base) => base.trim_end().to_string(),
        None => minus,
    }
}

impl Politica {
    pub fn nueva(lista_negra_usuario: &[String], pid_propio: u32) -> Self {
        Politica {
            protegidos: PROTEGIDOS_FIJOS.iter().map(|n| normalizar(n)).collect(),
            reforzados: REFORZADOS_FIJOS.iter().map(|n| normalizar(n)).collect(),
            lista_negra_usuario: lista_negra_usuario
                .iter()
                .map(|n| normalizar(n))
                .filter(|n| !n.is_empty())
                .collect(),
            pid_propio,
        }
    }

    pub fn pid_propio(&self) -> u32 {
        self.pid_propio
    }

    pub fn clasificar(&self, c: &Candidato) -> Nivel {
        if PIDS_PROTEGIDOS.contains(&c.pid) {
            return Nivel::Protegido;
        }
        let nombre = normalizar(&c.nombre);
        if self.protegidos.contains(&nombre) || self.lista_negra_usuario.contains(&nombre) {
            return Nivel::Protegido;
        }
        if c.dueno != Dueno::UsuarioActual {
            return Nivel::Protegido;
        }
        if c.pid == self.pid_propio || self.reforzados.contains(&nombre) {
            return Nivel::Reforzado;
        }
        Nivel::Normal
    }

    pub fn es_fijo_protegido(nombre: &str) -> bool {
        let n = normalizar(nombre);
        PROTEGIDOS_FIJOS.iter().any(|f| normalizar(f) == n)
    }

    pub fn es_fijo_reforzado(nombre: &str) -> bool {
        let n = normalizar(nombre);
        REFORZADOS_FIJOS.iter().any(|f| normalizar(f) == n)
    }

    pub fn sugerir_reiniciar_explorer(nombre: &str) -> bool {
        normalizar(nombre) == EXPLORER
    }

    pub fn validar_solicitud(
        &self,
        candidatos: &[Candidato],
        confirmacion_escrita: Option<&str>,
    ) -> Validacion {
        let confirmado = confirmacion_escrita == Some(CONFIRMACION_REFORZADA);
        let mut vistos = HashSet::new();
        let mut v = Validacion::default();
        for c in candidatos {
            if !vistos.insert(c.pid) {
                continue;
            }
            match self.clasificar(c) {
                Nivel::Protegido => v.rechazados.push(Rechazado {
                    candidato: c.clone(),
                    motivo: Rechazo::Protegido,
                }),
                Nivel::Reforzado if !confirmado => v.rechazados.push(Rechazado {
                    candidato: c.clone(),
                    motivo: Rechazo::RequiereConfirmacion,
                }),
                Nivel::Reforzado | Nivel::Normal => v.permitidos.push(c.clone()),
            }
        }
        v
    }

    pub fn validar_pids(
        &self,
        pids: &[u32],
        vivos: &[Candidato],
        confirmacion_escrita: Option<&str>,
    ) -> Validacion {
        let mut vistos = HashSet::new();
        let mut encontrados = Vec::new();
        let mut ausentes = Vec::new();
        for &pid in pids {
            if !vistos.insert(pid) {
                continue;
            }
            match vivos.iter().find(|c| c.pid == pid) {
                Some(c) => encontrados.push(c.clone()),
                None => ausentes.push(Rechazado {
                    candidato: Candidato {
                        pid,
                        nombre: String::new(),
                        dueno: Dueno::Desconocido,
                    },
                    motivo: Rechazo::YaNoExiste,
                }),
            }
        }
        let mut v = self.validar_solicitud(&encontrados, confirmacion_escrita);
        v.rechazados.extend(ausentes);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PID_PROPIO: u32 = 9999;

    fn politica() -> Politica {
        Politica::nueva(&[], PID_PROPIO)
    }

    fn cand(pid: u32, nombre: &str, dueno: Dueno) -> Candidato {
        Candidato {
            pid,
            nombre: nombre.to_string(),
            dueno,
        }
    }

    fn mio(pid: u32, nombre: &str) -> Candidato {
        cand(pid, nombre, Dueno::UsuarioActual)
    }

    #[test]
    fn cada_protegido_fijo_es_protegido() {
        let p = politica();
        for (i, nombre) in PROTEGIDOS_FIJOS.iter().enumerate() {
            let c = mio(1000 + i as u32, nombre);
            assert_eq!(p.clasificar(&c), Nivel::Protegido, "{nombre}");
            let con_exe = mio(2000 + i as u32, &format!("{nombre}.exe"));
            assert_eq!(p.clasificar(&con_exe), Nivel::Protegido, "{nombre}.exe");
            assert!(Politica::es_fijo_protegido(nombre), "{nombre}");
        }
    }

    #[test]
    fn protegidos_fijos_explicitos() {
        let p = politica();
        for nombre in [
            "System",
            "Idle",
            "Registry",
            "smss.exe",
            "csrss.exe",
            "wininit.exe",
            "winlogon.exe",
            "services.exe",
            "lsass.exe",
            "svchost.exe",
            "dwm.exe",
            "fontdrvhost.exe",
            "Memory Compression",
            "MsMpEng.exe",
            "audiodg.exe",
            "spoolsv.exe",
            "sihost.exe",
            "ctfmon.exe",
            "conhost.exe",
            "explorer.exe",
        ] {
            assert_eq!(p.clasificar(&mio(1234, nombre)), Nivel::Protegido, "{nombre}");
        }
    }

    #[test]
    fn cada_reforzado_fijo_es_reforzado() {
        let p = politica();
        for (i, nombre) in REFORZADOS_FIJOS.iter().enumerate() {
            let c = mio(3000 + i as u32, nombre);
            assert_eq!(p.clasificar(&c), Nivel::Reforzado, "{nombre}");
            let con_exe = mio(4000 + i as u32, &format!("{nombre}.exe"));
            assert_eq!(p.clasificar(&con_exe), Nivel::Reforzado, "{nombre}.exe");
            assert!(Politica::es_fijo_reforzado(nombre), "{nombre}");
            assert!(!Politica::es_fijo_protegido(nombre), "{nombre}");
        }
    }

    #[test]
    fn glassdeck_con_mayuscula_es_reforzado() {
        let p = politica();
        assert_eq!(p.clasificar(&mio(50, "Glassdeck.exe")), Nivel::Reforzado);
        assert_eq!(p.clasificar(&mio(51, "GLASSDECK")), Nivel::Reforzado);
    }

    #[test]
    fn resto_es_normal() {
        let p = politica();
        for nombre in ["notepad.exe", "chrome.exe", "Spotify.exe", "explorer2.exe", "nodejs.exe"] {
            assert_eq!(p.clasificar(&mio(777, nombre)), Nivel::Normal, "{nombre}");
        }
    }

    #[test]
    fn mayusculas_y_minusculas_no_importan() {
        let p = politica();
        assert_eq!(p.clasificar(&mio(10, "LSASS.EXE")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(11, "ExPlOrEr.ExE")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(12, "memory compression")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(13, "MSMPENG")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(14, "CURSOR.EXE")), Nivel::Reforzado);
        assert_eq!(p.clasificar(&mio(15, "windowsterminal.exe")), Nivel::Reforzado);
        assert_eq!(p.clasificar(&mio(16, "PowerShell.Exe")), Nivel::Reforzado);
        assert_eq!(p.clasificar(&mio(17, "code")), Nivel::Reforzado);
    }

    #[test]
    fn sufijo_exe_se_quita_solo_al_final() {
        assert_eq!(normalizar("Code.exe"), "code");
        assert_eq!(normalizar("Code.EXE"), "code");
        assert_eq!(normalizar("  node.exe  "), "node");
        assert_eq!(normalizar("node"), "node");
        assert_eq!(normalizar("exe"), "exe");
        assert_eq!(normalizar("my.exe.app"), "my.exe.app");
        assert_eq!(normalizar(".exe"), "");
        let p = politica();
        assert_eq!(p.clasificar(&mio(20, "cmd.exe.bak")), Nivel::Normal);
    }

    #[test]
    fn pid_0_y_4_siempre_protegidos() {
        let p = politica();
        assert_eq!(p.clasificar(&mio(0, "notepad.exe")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(4, "notepad.exe")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(0, "[System Process]")), Nivel::Protegido);
        let p0 = Politica::nueva(&[], 4);
        assert_eq!(p0.clasificar(&mio(4, "glassdeck.exe")), Nivel::Protegido);
    }

    #[test]
    fn duenos_ajenos_son_protegidos() {
        let p = politica();
        for dueno in [Dueno::Sistema, Dueno::OtroUsuario, Dueno::Desconocido] {
            assert_eq!(
                p.clasificar(&cand(500, "notepad.exe", dueno)),
                Nivel::Protegido,
                "{dueno:?}"
            );
            assert_eq!(
                p.clasificar(&cand(501, "node.exe", dueno)),
                Nivel::Protegido,
                "{dueno:?}"
            );
            assert_eq!(
                p.clasificar(&cand(PID_PROPIO, "glassdeck.exe", dueno)),
                Nivel::Protegido,
                "{dueno:?}"
            );
        }
        assert_eq!(
            p.clasificar(&cand(500, "notepad.exe", Dueno::UsuarioActual)),
            Nivel::Normal
        );
    }

    #[test]
    fn lista_negra_usuario_protege() {
        let extra = vec![
            "Spotify.exe".to_string(),
            "OBS64".to_string(),
            "  Teams.EXE ".to_string(),
            String::new(),
            "   ".to_string(),
        ];
        let p = Politica::nueva(&extra, PID_PROPIO);
        assert_eq!(p.clasificar(&mio(30, "spotify")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(31, "SPOTIFY.EXE")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(32, "obs64.exe")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(33, "teams.exe")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(34, "notepad.exe")), Nivel::Normal);
        assert!(!Politica::es_fijo_protegido("Spotify.exe"));
    }

    #[test]
    fn lista_negra_usuario_gana_sobre_reforzado() {
        let p = Politica::nueva(&["node".to_string()], PID_PROPIO);
        assert_eq!(p.clasificar(&mio(40, "node.exe")), Nivel::Protegido);
        assert_eq!(p.clasificar(&mio(41, "cmd.exe")), Nivel::Reforzado);
    }

    #[test]
    fn pid_propio_es_reforzado() {
        let p = politica();
        assert_eq!(p.pid_propio(), PID_PROPIO);
        assert_eq!(p.clasificar(&mio(PID_PROPIO, "loquesea.exe")), Nivel::Reforzado);
        assert_eq!(p.clasificar(&mio(PID_PROPIO + 1, "loquesea.exe")), Nivel::Normal);
        assert_eq!(p.clasificar(&mio(PID_PROPIO, "lsass.exe")), Nivel::Protegido);
    }

    #[test]
    fn es_fijo_protegido_estatico() {
        assert!(Politica::es_fijo_protegido("svchost.exe"));
        assert!(Politica::es_fijo_protegido("SVCHOST"));
        assert!(Politica::es_fijo_protegido("Memory Compression"));
        assert!(!Politica::es_fijo_protegido("notepad.exe"));
        assert!(!Politica::es_fijo_protegido("Cursor.exe"));
        assert!(!Politica::es_fijo_protegido(""));
    }

    #[test]
    fn bandera_explorer() {
        assert!(Politica::sugerir_reiniciar_explorer("explorer.exe"));
        assert!(Politica::sugerir_reiniciar_explorer("Explorer"));
        assert!(Politica::sugerir_reiniciar_explorer("EXPLORER.EXE"));
        assert!(!Politica::sugerir_reiniciar_explorer("iexplore.exe"));
        assert!(!Politica::sugerir_reiniciar_explorer("explorer2.exe"));
        assert!(!Politica::sugerir_reiniciar_explorer("svchost.exe"));
        let p = politica();
        assert_eq!(p.clasificar(&mio(60, "explorer.exe")), Nivel::Protegido);
    }

    #[test]
    fn cerrar_exacto_permite_reforzados() {
        let p = politica();
        let lista = vec![mio(100, "Code.exe"), mio(101, "notepad.exe")];
        let v = p.validar_solicitud(&lista, Some("CERRAR"));
        assert_eq!(v.permitidos.len(), 2);
        assert!(v.rechazados.is_empty());
    }

    #[test]
    fn cerrar_ausente_rechaza_solo_reforzados() {
        let p = politica();
        let lista = vec![mio(100, "Code.exe"), mio(101, "notepad.exe")];
        let v = p.validar_solicitud(&lista, None);
        assert_eq!(v.permitidos, vec![mio(101, "notepad.exe")]);
        assert_eq!(
            v.rechazados,
            vec![Rechazado {
                candidato: mio(100, "Code.exe"),
                motivo: Rechazo::RequiereConfirmacion,
            }]
        );
    }

    #[test]
    fn cerrar_minusculas_o_texto_erroneo_falla() {
        let p = politica();
        let lista = vec![mio(100, "cmd.exe")];
        for texto in ["cerrar", "Cerrar", "CERRAR ", " CERRAR", "CERRA", "SI", "", "CERRAR\n"] {
            let v = p.validar_solicitud(&lista, Some(texto));
            assert!(v.permitidos.is_empty(), "{texto:?}");
            assert_eq!(v.rechazados.len(), 1, "{texto:?}");
            assert_eq!(v.rechazados[0].motivo, Rechazo::RequiereConfirmacion, "{texto:?}");
        }
    }

    #[test]
    fn normales_no_requieren_confirmacion() {
        let p = politica();
        let lista = vec![mio(200, "notepad.exe"), mio(201, "chrome.exe")];
        let v = p.validar_solicitud(&lista, None);
        assert_eq!(v.permitidos.len(), 2);
        assert!(v.rechazados.is_empty());
    }

    #[test]
    fn protegidos_se_rechazan_aunque_haya_confirmacion() {
        let p = politica();
        let lista = vec![
            mio(300, "lsass.exe"),
            cand(301, "notepad.exe", Dueno::Sistema),
            mio(4, "notepad.exe"),
            mio(302, "explorer.exe"),
        ];
        let v = p.validar_solicitud(&lista, Some("CERRAR"));
        assert!(v.permitidos.is_empty());
        assert_eq!(v.rechazados.len(), 4);
        assert!(v.rechazados.iter().all(|r| r.motivo == Rechazo::Protegido));
    }

    #[test]
    fn pid_propio_requiere_confirmacion() {
        let p = politica();
        let lista = vec![mio(PID_PROPIO, "glassdeck.exe")];
        let v = p.validar_solicitud(&lista, None);
        assert_eq!(v.rechazados[0].motivo, Rechazo::RequiereConfirmacion);
        let v = p.validar_solicitud(&lista, Some("CERRAR"));
        assert_eq!(v.permitidos.len(), 1);
    }

    #[test]
    fn solicitud_mixta() {
        let p = politica();
        let lista = vec![
            mio(1, "svchost.exe"),
            mio(2, "node.exe"),
            mio(3, "notepad.exe"),
            cand(5, "chrome.exe", Dueno::OtroUsuario),
        ];
        let v = p.validar_solicitud(&lista, Some("cerrar"));
        assert_eq!(v.permitidos, vec![mio(3, "notepad.exe")]);
        let motivos: Vec<(u32, Rechazo)> = v
            .rechazados
            .iter()
            .map(|r| (r.candidato.pid, r.motivo))
            .collect();
        assert_eq!(
            motivos,
            vec![
                (1, Rechazo::Protegido),
                (2, Rechazo::RequiereConfirmacion),
                (5, Rechazo::Protegido),
            ]
        );
    }

    #[test]
    fn pids_duplicados_se_procesan_una_vez() {
        let p = politica();
        let lista = vec![mio(10, "notepad.exe"), mio(10, "notepad.exe")];
        let v = p.validar_solicitud(&lista, None);
        assert_eq!(v.permitidos.len(), 1);
    }

    #[test]
    fn validar_pids_marca_ausentes() {
        let p = politica();
        let vivos = vec![mio(10, "notepad.exe"), mio(11, "Code.exe"), mio(12, "lsass.exe")];
        let v = p.validar_pids(&[10, 11, 12, 99, 10], &vivos, None);
        assert_eq!(v.permitidos, vec![mio(10, "notepad.exe")]);
        let motivos: Vec<(u32, Rechazo)> = v
            .rechazados
            .iter()
            .map(|r| (r.candidato.pid, r.motivo))
            .collect();
        assert_eq!(
            motivos,
            vec![
                (11, Rechazo::RequiereConfirmacion),
                (12, Rechazo::Protegido),
                (99, Rechazo::YaNoExiste),
            ]
        );
    }

    #[test]
    fn rechazo_a_resultado() {
        assert_eq!(ResultadoCierre::from(Rechazo::Protegido), ResultadoCierre::Rechazado);
        assert_eq!(
            ResultadoCierre::from(Rechazo::RequiereConfirmacion),
            ResultadoCierre::Rechazado
        );
        assert_eq!(ResultadoCierre::from(Rechazo::YaNoExiste), ResultadoCierre::YaNoExistia);
    }

    #[test]
    fn serializacion_snake_case() {
        assert_eq!(serde_json::to_string(&Nivel::Protegido).unwrap(), "\"protegido\"");
        assert_eq!(serde_json::to_string(&Nivel::Reforzado).unwrap(), "\"reforzado\"");
        assert_eq!(serde_json::to_string(&Nivel::Normal).unwrap(), "\"normal\"");
        assert_eq!(
            serde_json::to_string(&Dueno::UsuarioActual).unwrap(),
            "\"usuario_actual\""
        );
        assert_eq!(
            serde_json::to_string(&Rechazo::RequiereConfirmacion).unwrap(),
            "\"requiere_confirmacion\""
        );
        assert_eq!(
            serde_json::to_string(&ResultadoCierre::SinPermiso).unwrap(),
            "\"sin_permiso\""
        );
        assert_eq!(
            serde_json::to_string(&ResultadoCierre::YaNoExistia).unwrap(),
            "\"ya_no_existia\""
        );
    }
}

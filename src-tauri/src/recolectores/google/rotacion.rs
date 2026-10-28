use serde::Serialize;
use std::collections::HashSet;

/// Un evento que empieza dentro de este margen se muestra antes que cualquier otra cosa (GD-ROT-01).
pub const PRONTO_MS: i64 = 30 * 60_000;

pub struct ElementoRotable {
    pub clave: String,
    /// Solo eventos con hora; los correos y los de todo el día no tienen prisa.
    pub inicio_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alerta {
    pub clave: Option<String>,
    pub desde_ms: i64,
    pub proxima_ms: i64,
}

fn es_pronto(e: &ElementoRotable, ahora: i64) -> bool {
    e.inicio_ms.is_some_and(|i| i >= ahora && i - ahora <= PRONTO_MS)
}

/// Elige qué mostrar en la alerta; en memoria y con el reloj inyectado para probarlo.
pub struct Rotador {
    actual: Option<String>,
    desde_ms: i64,
    vistos: HashSet<String>,
    semilla: u64,
}

impl Rotador {
    pub fn nuevo(semilla: u64) -> Self {
        Self { actual: None, desde_ms: 0, vistos: HashSet::new(), semilla: semilla | 1 }
    }

    fn azar(&mut self, n: usize) -> usize {
        self.semilla ^= self.semilla << 13;
        self.semilla ^= self.semilla >> 7;
        self.semilla ^= self.semilla << 17;
        (self.semilla % n as u64) as usize
    }

    fn pronto<'a>(&self, elementos: &'a [ElementoRotable], ahora: i64) -> Option<&'a ElementoRotable> {
        elementos
            .iter()
            .filter(|e| es_pronto(e, ahora))
            .filter(|e| !self.vistos.contains(&e.clave))
            .min_by_key(|e| e.inicio_ms)
    }

    fn mostrar(&mut self, clave: String, ahora: i64) {
        self.vistos.insert(clave.clone());
        self.actual = Some(clave);
        self.desde_ms = ahora;
    }

    /// Rota cada `periodo_ms`; antes de tiempo solo si el elemento actual desapareció (correo leído) o si
    /// un evento entra en sus últimos 30 min. Al azar entre lo no mostrado en esta vuelta.
    pub fn avanzar(&mut self, elementos: &[ElementoRotable], ahora: i64, periodo_ms: i64) -> Alerta {
        if self.actual.as_ref().is_some_and(|a| !elementos.iter().any(|e| &e.clave == a)) {
            self.actual = None;
        }
        let vencido = ahora - self.desde_ms >= periodo_ms;
        let actual_es_pronto =
            self.actual.as_ref().is_some_and(|a| elementos.iter().any(|e| &e.clave == a && es_pronto(e, ahora)));
        let urgente = self.pronto(elementos, ahora).map(|e| e.clave.clone());
        if let Some(clave) = urgente.filter(|_| vencido || self.actual.is_none() || !actual_es_pronto) {
            self.mostrar(clave, ahora);
        } else if self.actual.is_none() || vencido {
            let mut pendientes: Vec<&ElementoRotable> =
                elementos.iter().filter(|e| !self.vistos.contains(&e.clave) && Some(&e.clave) != self.actual.as_ref()).collect();
            if pendientes.is_empty() {
                self.vistos.clear();
                pendientes = elementos.iter().filter(|e| Some(&e.clave) != self.actual.as_ref()).collect();
            }
            if pendientes.is_empty() {
                if self.actual.is_some() {
                    self.desde_ms = ahora;
                }
            } else {
                let i = self.azar(pendientes.len());
                let clave = pendientes[i].clave.clone();
                self.mostrar(clave, ahora);
            }
        }
        if elementos.is_empty() {
            self.vistos.clear();
        }
        Alerta { clave: self.actual.clone(), desde_ms: self.desde_ms, proxima_ms: self.desde_ms + periodo_ms }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const MIN: i64 = 60_000;
    const PERIODO: i64 = 10 * MIN;

    fn correos(n: usize) -> Vec<ElementoRotable> {
        (0..n).map(|i| ElementoRotable { clave: format!("correo:cuenta-a:{i}"), inicio_ms: None }).collect()
    }

    fn evento(clave: &str, inicio_ms: i64) -> ElementoRotable {
        ElementoRotable { clave: clave.into(), inicio_ms: Some(inicio_ms) }
    }

    /// El widget consulta cada minuto; en 1 h hay 6 rotaciones de 10 min.
    fn simular(r: &mut Rotador, elementos: &[ElementoRotable], desde: i64, minutos: i64) -> Vec<String> {
        let mut vistos = Vec::new();
        for m in 0..minutos {
            if let Some(c) = r.avanzar(elementos, desde + m * MIN, PERIODO).clave {
                if vistos.last() != Some(&c) {
                    vistos.push(c);
                }
            }
        }
        vistos
    }

    #[test]
    fn en_una_hora_se_ven_al_menos_6_elementos_distintos() {
        let t0 = 1_800_000_000_000;
        for semilla in [1, 7, 42, 2026, u64::MAX] {
            let mut elementos = correos(5);
            elementos.push(evento("evento:cuenta-b:lejano", t0 + 20 * 60 * MIN));
            elementos.push(evento("evento:cuenta-b:otro", t0 + 30 * 60 * MIN));
            let mut r = Rotador::nuevo(semilla);
            let mostrados = simular(&mut r, &elementos, t0, 60);
            let distintos: HashSet<&String> = mostrados.iter().collect();
            assert!(distintos.len() >= 6, "semilla {semilla}: {mostrados:?}");
            assert_eq!(distintos.len(), mostrados.len(), "no se repite nada dentro de la misma vuelta");
        }
    }

    #[test]
    fn un_evento_que_empieza_en_menos_de_30_min_tiene_prioridad() {
        let t0 = 1_800_000_000_000;
        let mut elementos = correos(8);
        elementos.push(evento("evento:cuenta-b:junta", t0 + 45 * MIN));
        let mut r = Rotador::nuevo(3);
        let primero = r.avanzar(&elementos, t0, PERIODO).clave.unwrap();
        assert!(primero.starts_with("correo:"), "a 45 min todavía no es urgente");
        let a_los_15 = r.avanzar(&elementos, t0 + 15 * MIN, PERIODO);
        assert_eq!(a_los_15.clave.as_deref(), Some("evento:cuenta-b:junta"), "a 30 min del inicio pasa al frente");
        assert_eq!(a_los_15.proxima_ms, t0 + 25 * MIN);
        let siguiente = r.avanzar(&elementos, t0 + 25 * MIN, PERIODO).clave.unwrap();
        assert!(siguiente.starts_with("correo:"), "ya mostrado en esta vuelta, sigue la rotación");
    }

    #[test]
    fn el_mas_proximo_va_primero_y_uno_ya_iniciado_no_es_urgente() {
        let t0 = 1_800_000_000_000;
        let elementos = vec![
            evento("evento:a:en-curso", t0 - 5 * MIN),
            evento("evento:a:en-20", t0 + 20 * MIN),
            evento("evento:a:en-5", t0 + 5 * MIN),
        ];
        let mut r = Rotador::nuevo(9);
        assert_eq!(r.avanzar(&elementos, t0, PERIODO).clave.as_deref(), Some("evento:a:en-5"));
        assert_eq!(r.avanzar(&elementos, t0 + 10 * MIN, PERIODO).clave.as_deref(), Some("evento:a:en-20"));
    }

    #[test]
    fn un_correo_leido_se_reemplaza_sin_esperar_y_sin_datos_no_hay_alerta() {
        let t0 = 1_800_000_000_000;
        let mut r = Rotador::nuevo(5);
        let elementos = correos(3);
        let a = r.avanzar(&elementos, t0, PERIODO).clave.unwrap();
        let restantes: Vec<ElementoRotable> =
            elementos.into_iter().filter(|e| e.clave != a).collect();
        let b = r.avanzar(&restantes, t0 + MIN, PERIODO).clave.unwrap();
        assert_ne!(a, b);
        assert_eq!(r.avanzar(&[], t0 + 2 * MIN, PERIODO).clave, None);
    }
}

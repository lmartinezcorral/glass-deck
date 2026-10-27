use std::collections::VecDeque;

/// Capacidad del búfer: una hora al intervalo mínimo permitido (1 s).
pub const PUNTOS_UNA_HORA: usize = 3_600;

pub fn puntos_por_hora(intervalo_ms: u64) -> usize {
    ((3_600_000 / intervalo_ms.max(1)) as usize).clamp(1, PUNTOS_UNA_HORA)
}

#[derive(Debug, Clone)]
pub struct Anillo {
    capacidad: usize,
    datos: VecDeque<f32>,
}

impl Anillo {
    pub fn nuevo(capacidad: usize) -> Self {
        Self { capacidad, datos: VecDeque::with_capacity(capacidad.min(256)) }
    }

    pub fn agregar(&mut self, valor: f32) {
        if self.datos.len() == self.capacidad {
            self.datos.pop_front();
        }
        self.datos.push_back(valor);
    }

    #[cfg(test)]
    pub fn valores(&self) -> Vec<f32> {
        self.datos.iter().copied().collect()
    }

    pub fn ultimos(&self, n: usize) -> Vec<f32> {
        let inicio = self.datos.len().saturating_sub(n);
        self.datos.iter().skip(inicio).copied().collect()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.datos.len()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn descarta_los_mas_viejos_al_llenarse() {
        let mut a = Anillo::nuevo(3);
        for v in 1..=5 {
            a.agregar(v as f32);
        }
        assert_eq!(a.valores(), vec![3.0, 4.0, 5.0]);
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn una_hora_completa_cabe_en_el_bufer_y_lo_mas_viejo_se_descarta() {
        let mut a = Anillo::nuevo(PUNTOS_UNA_HORA);
        for v in 0..(PUNTOS_UNA_HORA + 5) {
            a.agregar(v as f32);
        }
        assert_eq!(a.len(), 3_600);
        assert_eq!(a.valores()[0], 5.0);
    }

    #[test]
    fn la_ultima_hora_depende_del_intervalo() {
        assert_eq!(puntos_por_hora(1_000), 3_600);
        assert_eq!(puntos_por_hora(2_000), 1_800);
        assert_eq!(puntos_por_hora(5_000), 720);
        assert_eq!(puntos_por_hora(500), 3_600);
        assert_eq!(puntos_por_hora(0), 3_600);
    }

    #[test]
    fn ultimos_regresa_la_cola() {
        let mut a = Anillo::nuevo(PUNTOS_UNA_HORA);
        for v in 0..10 {
            a.agregar(v as f32);
        }
        assert_eq!(a.ultimos(3), vec![7.0, 8.0, 9.0]);
        assert_eq!(a.ultimos(50).len(), 10);
    }
}

use std::collections::VecDeque;

pub const PUNTOS_UNA_HORA: usize = 3_600;

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

    pub fn valores(&self) -> Vec<f32> {
        self.datos.iter().copied().collect()
    }

    #[cfg(test)]
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
    fn ultimos_regresa_la_cola() {
        let mut a = Anillo::nuevo(PUNTOS_UNA_HORA);
        for v in 0..10 {
            a.agregar(v as f32);
        }
        assert_eq!(a.ultimos(3), vec![7.0, 8.0, 9.0]);
        assert_eq!(a.ultimos(50).len(), 10);
    }
}

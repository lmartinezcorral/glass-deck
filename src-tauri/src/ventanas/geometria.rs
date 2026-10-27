#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub ancho: i32,
    pub alto: i32,
}

impl Rect {
    pub fn nuevo(x: i32, y: i32, ancho: i32, alto: i32) -> Self {
        Self { x, y, ancho, alto }
    }

    #[cfg(test)]
    pub fn se_superpone(&self, otro: &Rect) -> bool {
        self.x < otro.x + otro.ancho && otro.x < self.x + self.ancho && self.y < otro.y + otro.alto && otro.y < self.y + self.alto
    }
}

const MARGEN: f64 = 24.0;
const COLUMNA: f64 = 320.0;
const FILA: f64 = 290.0;

/// Celda de la rejilla por defecto: columnas desde la derecha del área de trabajo, de arriba abajo.
pub fn celda_por_defecto(area: Rect, escala: f64, indice: usize) -> (i32, i32) {
    let margen = (MARGEN * escala) as i32;
    let columna = (COLUMNA * escala) as i32;
    let fila_alto = (FILA * escala) as i32;
    let filas = ((area.alto - margen) / fila_alto).max(1) as usize;
    let col = (indice / filas) as i32;
    let fila = (indice % filas) as i32;
    (area.x + area.ancho - margen - (col + 1) * columna, area.y + margen + fila * fila_alto)
}

/// Posición imantada a los bordes del área o a otros widgets si alguno queda a `distancia` o menos.
pub fn ajustar_iman(actual: Rect, area: Rect, otros: &[Rect], separacion: i32, distancia: i32) -> Option<(i32, i32)> {
    let (w, h) = (actual.ancho, actual.alto);
    let mut xs = vec![area.x + separacion, area.x + area.ancho - w - separacion];
    let mut ys = vec![area.y + separacion, area.y + area.alto - h - separacion];
    for o in otros {
        xs.extend([o.x + o.ancho + separacion, o.x - w - separacion, o.x]);
        ys.extend([o.y + o.alto + separacion, o.y - h - separacion, o.y]);
    }
    let cerca = |valor: i32, candidatos: &[i32]| {
        candidatos.iter().copied().filter(|c| (c - valor).abs() <= distancia).min_by_key(|c| (c - valor).abs())
    };
    let nx = cerca(actual.x, &xs);
    let ny = cerca(actual.y, &ys);
    if nx.is_none() && ny.is_none() {
        return None;
    }
    Some((nx.unwrap_or(actual.x), ny.unwrap_or(actual.y)))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventanas::catalogo::WIDGETS_FASE_1;

    #[test]
    fn la_rejilla_por_defecto_no_superpone_widgets() {
        for (area, escala) in [(Rect::nuevo(0, 0, 1920, 1040), 1.0), (Rect::nuevo(-1280, 0, 1920, 1032), 1.5)] {
            let fisica = |v: u32| (v as f64 * escala) as i32;
            let rects: Vec<Rect> = (0..11)
                .map(|i| {
                    let def = &WIDGETS_FASE_1[i % WIDGETS_FASE_1.len()];
                    let (x, y) = celda_por_defecto(area, escala, i);
                    Rect::nuevo(x, y, fisica(def.ancho), fisica(def.alto))
                })
                .collect();
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    assert!(!a.se_superpone(b), "{a:?} se superpone con {b:?}");
                }
            }
        }
    }

    #[test]
    fn los_widgets_de_fase_1_caben_en_el_area_del_principal() {
        let casos = [
            (Rect::nuevo(0, 0, 1920, 1040), 1.0),
            (Rect::nuevo(0, 0, 1920, 1032), 1.5),
            (Rect::nuevo(0, 0, 1366, 728), 1.0),
        ];
        for (area, escala) in casos {
            let fisica = |v: u32| (v as f64 * escala) as i32;
            for (i, def) in WIDGETS_FASE_1.iter().enumerate() {
                let (x, y) = celda_por_defecto(area, escala, i);
                let r = Rect::nuevo(x, y, fisica(def.ancho), fisica(def.alto));
                assert!(r.x >= area.x && r.y >= area.y, "{} sale por arriba o la izquierda: {r:?}", def.id);
                assert!(r.x + r.ancho <= area.x + area.ancho, "{} sale por la derecha: {r:?}", def.id);
                assert!(r.y + r.alto <= area.y + area.alto, "{} sale por abajo: {r:?}", def.id);
            }
        }
    }

    #[test]
    fn el_iman_alinea_a_11_px_y_no_a_13() {
        let area = Rect::nuevo(0, 0, 1920, 1040);
        let otro = Rect::nuevo(100, 400, 200, 200);
        let destino = otro.x + otro.ancho + 8;
        let cerca = Rect::nuevo(destino + 11, 500, 220, 210);
        assert_eq!(ajustar_iman(cerca, area, &[otro], 8, 12), Some((destino, 500)));
        let lejos = Rect::nuevo(destino + 13, 500, 220, 210);
        assert_eq!(ajustar_iman(lejos, area, &[otro], 8, 12), None);
    }

    #[test]
    fn el_iman_alinea_al_borde_del_area() {
        let area = Rect::nuevo(0, 0, 1920, 1040);
        let w = Rect::nuevo(1920 - 220 - 8 - 5, 300, 220, 210);
        assert_eq!(ajustar_iman(w, area, &[], 8, 12), Some((1920 - 220 - 8, 300)));
    }
}

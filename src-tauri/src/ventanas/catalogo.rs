#[derive(Debug, Clone, Copy)]
pub struct DefWidget {
    pub id: &'static str,
    pub titulo: &'static str,
    pub ancho: u32,
    pub alto: u32,
    pub ancho_max: u32,
    pub alto_max: u32,
}

pub const WIDGETS_FASE_1: &[DefWidget] = &[
    DefWidget { id: "cpu", titulo: "CPU", ancho: 220, alto: 210, ancho_max: 760, alto_max: 520 },
    DefWidget { id: "ram", titulo: "Memoria", ancho: 220, alto: 210, ancho_max: 680, alto_max: 480 },
    DefWidget { id: "disco", titulo: "Disco", ancho: 260, alto: 200, ancho_max: 680, alto_max: 480 },
    DefWidget { id: "red", titulo: "Red", ancho: 260, alto: 200, ancho_max: 680, alto_max: 460 },
    DefWidget { id: "procesos", titulo: "Procesos", ancho: 300, alto: 270, ancho_max: 860, alto_max: 600 },
];

pub const ANCHO_PILDORA: u32 = 148;
pub const ALTO_PILDORA: u32 = 40;
pub const RADIO_ESQUINA: f64 = 22.0;

pub fn buscar(id: &str) -> Option<&'static DefWidget> {
    WIDGETS_FASE_1.iter().find(|w| w.id == id)
}

pub fn etiqueta(id: &str) -> String {
    format!("w-{id}")
}

pub fn id_de_etiqueta(etiqueta: &str) -> Option<&str> {
    etiqueta.strip_prefix("w-")
}

#[cfg(test)]
mod pruebas {
    use super::RADIO_ESQUINA;

    #[test]
    fn la_region_de_clic_usa_el_mismo_radio_que_el_vidrio_del_widget() {
        let tokens = include_str!("../../../src/tema/tokens.css");
        let radio: f64 = tokens
            .split("--gd-radio-widget:")
            .nth(1)
            .and_then(|r| r.split("px").next())
            .and_then(|r| r.trim().parse().ok())
            .expect("--gd-radio-widget en tokens.css");
        assert_eq!(RADIO_ESQUINA, radio);
    }
}

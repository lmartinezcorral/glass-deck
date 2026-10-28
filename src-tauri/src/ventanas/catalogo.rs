use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DefWidget {
    pub id: &'static str,
    pub titulo: &'static str,
    pub subtitulo: &'static str,
    /// Los widgets de servicios arrancan apagados para no subir la RAM sin que Luis los active.
    pub activo_defecto: bool,
    #[serde(skip)]
    pub ancho: u32,
    #[serde(skip)]
    pub alto: u32,
    #[serde(skip)]
    pub ancho_max: u32,
    #[serde(skip)]
    pub alto_max: u32,
}

const fn def(id: &'static str, titulo: &'static str, subtitulo: &'static str, activo_defecto: bool, tam: [u32; 4]) -> DefWidget {
    DefWidget { id, titulo, subtitulo, activo_defecto, ancho: tam[0], alto: tam[1], ancho_max: tam[2], alto_max: tam[3] }
}

pub const WIDGETS: &[DefWidget] = &[
    def("cpu", "CPU", "Uso total del procesador", true, [220, 210, 760, 520]),
    def("ram", "Memoria", "Memoria RAM en uso (GB)", true, [220, 210, 680, 480]),
    def("disco", "Disco", "Espacio usado y actividad por unidad", true, [260, 200, 680, 480]),
    def("red", "Red", "Velocidad de bajada y subida", true, [260, 200, 680, 460]),
    def("procesos", "Procesos", "Los 5 que más consumen", true, [300, 270, 860, 600]),
    def("spending", "Spending", "Gasto y uso de Cursor y Claude", false, [300, 260, 860, 600]),
    def("clima", "Clima", "Clima y pronóstico de tu ubicación", false, [260, 220, 760, 520]),
];

pub const ANCHO_PILDORA: u32 = 148;
pub const ALTO_PILDORA: u32 = 40;
pub const RADIO_ESQUINA: f64 = 14.0;

pub fn buscar(id: &str) -> Option<&'static DefWidget> {
    WIDGETS.iter().find(|w| w.id == id)
}

pub fn etiqueta(id: &str) -> String {
    format!("w-{id}")
}

pub fn id_de_etiqueta(etiqueta: &str) -> Option<&str> {
    etiqueta.strip_prefix("w-")
}

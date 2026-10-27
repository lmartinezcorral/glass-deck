use crate::ventanas::aplicar_efecto;
use tauri::{AppHandle, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const TOTAL: usize = 11;
const QUITAR: usize = 6;

struct Opciones {
    modo: String,
    efecto: bool,
    argumentos: String,
}

fn crear(app: &AppHandle, o: &Opciones, etiqueta: &str, vista: &str, x: i32, y: i32, ancho: f64, alto: f64) -> Option<tauri::WebviewWindow> {
    let url = WebviewUrl::App(format!("index.html?vista={vista}&modo={}", o.modo).into());
    let v = WebviewWindowBuilder::new(app, etiqueta, url)
        .title("Glassdeck POC")
        .additional_browser_args(&o.argumentos)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .inner_size(ancho, alto)
        .focused(false)
        .build()
        .ok()?;
    let _ = v.set_position(PhysicalPosition::new(x, y));
    if o.efecto {
        aplicar_efecto(app, &v, false);
    }
    Some(v)
}

pub fn ejecutar(app: &AppHandle, variante: &str) {
    let o = Opciones {
        modo: std::env::var("GLASSDECK_POC_MODO").unwrap_or_default().trim().to_string(),
        efecto: std::env::var("GLASSDECK_POC_EFECTO").map(|v| v.trim() != "no").unwrap_or(true),
        argumentos: std::env::var("GLASSDECK_POC_ARGS").unwrap_or_else(|_| crate::ventanas::ARGUMENTOS_WEBVIEW.to_string()),
    };
    log::info!(
        "Prueba de concepto R-01, variante {variante}, modo '{}', acrylic {}, argumentos '{}'",
        o.modo,
        o.efecto,
        o.argumentos
    );
    let Some(m) = app.primary_monitor().ok().flatten() else { return };
    let a = m.work_area();
    let paso_x = (240.0 * m.scale_factor()) as i32;
    let paso_y = (220.0 * m.scale_factor()) as i32;
    let por_fila = ((a.size.width as i32 - 40) / paso_x).max(1) as usize;
    match variante {
        "C" => {
            let monitores = app.available_monitors().unwrap_or_default();
            let n = monitores.len().max(1);
            for (i, mon) in monitores.iter().enumerate() {
                let widgets = TOTAL / n + usize::from(i < TOTAL % n);
                let area = mon.work_area();
                crear(
                    app,
                    &o,
                    &format!("poc-c-{i}"),
                    &format!("poc&agrupados={widgets}"),
                    area.position.x + 20,
                    area.position.y + 20,
                    1200.0,
                    700.0,
                );
            }
        }
        _ => {
            let mut ventanas = Vec::new();
            for i in 0..TOTAL {
                let x = a.position.x + 20 + (i % por_fila) as i32 * paso_x;
                let y = a.position.y + 20 + (i / por_fila) as i32 * paso_y;
                if let Some(v) = crear(app, &o, &format!("poc-{i}"), &format!("poc&n={i}"), x, y, 220.0, 200.0) {
                    ventanas.push(v);
                }
            }
            match variante {
                "A6" => ventanas.iter().take(QUITAR).for_each(|v| {
                    let _ = v.hide();
                }),
                "B" => ventanas.iter().take(QUITAR).for_each(|v| {
                    let _ = v.destroy();
                }),
                _ => {}
            }
        }
    }
}

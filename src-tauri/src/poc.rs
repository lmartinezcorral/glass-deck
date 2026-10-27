use crate::ventanas::aplicar_efecto;
use tauri::{AppHandle, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const TOTAL: usize = 11;
const QUITAR: usize = 6;

fn crear(app: &AppHandle, etiqueta: &str, vista: &str, x: i32, y: i32, ancho: f64, alto: f64) -> Option<tauri::WebviewWindow> {
    let url = WebviewUrl::App(format!("index.html?vista={vista}").into());
    let v = WebviewWindowBuilder::new(app, etiqueta, url)
        .title("Glassdeck POC")
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
    aplicar_efecto(app, &v, false);
    Some(v)
}

pub fn ejecutar(app: &AppHandle, variante: &str) {
    log::info!("Prueba de concepto R-01, variante {variante}");
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
                if let Some(v) = crear(app, &format!("poc-{i}"), &format!("poc&n={i}"), x, y, 220.0, 200.0) {
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

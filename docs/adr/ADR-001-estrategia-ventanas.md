# ADR-001 — Estrategia de ventanas de los widgets

| Campo | Valor |
|---|---|
| Estado | **Aceptado** por Luis el 2026-10-27 |
| Fecha | 2026-10-27 |
| Riesgo | R-01 (`SPEC.md` §7) |
| Requerimientos | GD-NFR-PERF-02: RAM < 200 MB en total (proceso principal + WebView2). GD-NFR-PERF-01: CPU en reposo < 2 % |

## Contexto

Cada widget es una ventana flotante independiente (D-02). En Tauri 2, cada ventana es un WebView2, y WebView2 puede crear procesos de render por ventana. Con 11 ventanas (CPU, RAM, Disco, Red, Procesos, Spending, Correo/Agenda, Clima, Audio, Brillo, Atajos) más el dock, el consumo puede pasar del presupuesto.

## Variantes medidas

| Variante | Descripción |
|---|---|
| A | 11 ventanas visibles, entorno de WebView2 compartido (comportamiento por defecto de Tauri) |
| A6 | Igual que A, con 6 ventanas ocultas (no destruidas) |
| B | Igual que A, pero 6 ventanas se destruyen: quedan 5, como en la fase 1 |
| C | Una ventana transparente por monitor que dibuja todos los widgets (aquí, 1 monitor con 11 widgets) |

## Método

- Build release (`npx tauri build --no-bundle`), ThinkPad con Windows 10 Pro 22H2, 4 núcleos lógicos, **un solo monitor conectado** durante la prueba.
- Ventanas de 220×200 con *acrylic*, un medidor y un sparkline que se actualizan una vez por segundo (`?vista=poc`).
- `poc/medir.ps1`: suma de `Working Set - Private` del árbol (`glassdeck.exe` + `msedgewebview2.exe` hijos) cada 2 s; CPU como suma de tiempos de kernel + usuario sobre los 4 núcleos. 15 s de calentamiento y de 40 a 65 s de muestreo.
- Variables del modo POC: `GLASSDECK_POC` (variante), `GLASSDECK_POC_MODO` (`estatico` o `sin-transicion`), `GLASSDECK_POC_EFECTO=no` (sin *acrylic*) y `GLASSDECK_POC_ARGS` (argumentos del navegador).

## Resultados

### 1. Diagnóstico del CPU (variante A, 11 ventanas)

| Caso | RAM prom. | RAM máx. | CPU prom. |
|---|---|---|---|
| Transiciones CSS en cada dato (diseño original) | 428 MB | 454 MB | 44.3 % |
| Igual, sin *acrylic* | 431 MB | 441 MB | 50.9 % |
| Datos estáticos | 335 MB | 338 MB | 0.07 % |
| 1 dato/s sin transiciones | 341 MB | 361 MB | 4.1 % |

El *acrylic* no influye en el CPU. El costo está en el proceso GPU de WebView2 (34 % de los 4 núcleos), que recompone cada ventana transparente en cada cuadro mientras dura la transición del arco del medidor (`stroke-dashoffset` bajo `drop-shadow`) y de la barra (`width` con `box-shadow`). **Corrección ya aplicada:** las transiciones solo corren en la animación de entrada (1.2 s); después, cada dato se pinta en un solo cuadro (`useAsentado`, clases `gd-medidor--vivo` y `gd-uso--vivo`).

### 2. Variantes, 1 dato/s sin transiciones

| Variante | Ventanas | Procesos | RAM prom. | RAM máx. | CPU prom. |
|---|---|---|---|---|---|
| A6 | 11 (6 ocultas) | 17 | 346 MB | 366 MB | 4.6 % |
| B | 5 | 11 | 216 MB | 225 MB | 3.0 % |
| C | 1 | 7 | 195 MB | 213 MB | 2.2 % |

Ocultar ventanas no libera memoria; destruirlas sí. El costo se modela como una base fija de WebView2 de unos 110 MB más unos 21 MB por ventana.

### 3. Un solo proceso de render (`--renderer-process-limit=1`)

| Escenario | Procesos | RAM prom. | RAM máx. | CPU prom. |
|---|---|---|---|---|
| A, 11 ventanas | 9 | 212 MB | 219 MB | 3.6 % |
| C, 1 ventana con 11 widgets | 7 | 160 MB | 169 MB | 2.4 % |
| **App real de la fase 1** (5 widgets + dock, datos reales a 1 s) | 9 | 198 MB | 202 MB | 2.4 % |
| **App real de la fase 1**, CPU/RAM/red a 2 s | 9 | 197 MB | 202 MB | 1.7 % |

Antes de este ajuste, la app real de la fase 1 medía 233 MB promedio, 241 MB máx. y 2.4 % de CPU. **Corrección ya aplicada:** `ventanas::ARGUMENTOS_WEBVIEW` agrega `--renderer-process-limit=1` a todas las ventanas (widgets, paneles y POC), conservando los `--disable-features` por defecto de Tauri.

Desglose de CPU de la app real a 1 s (muestra de 30 s): recolector en Rust 0.49 %, renderizadores 0.79 %, proceso GPU 0.36 %, navegador 0.16 %.

## Decisión

Ninguna variante con ventanas independientes cumple los dos presupuestos con 11 widgets; la variante C cumple la RAM pero no el CPU, y obliga a rehacer el gestor de ventanas (zonas de clic transparentes y arrastre dentro de una sola ventana).

Luis eligió (2026-10-27):

1. **Una ventana por widget** (D-02) con la variante B y un solo proceso de render; las ventanas de widgets cerrados se destruyen.
2. **Presupuesto de RAM:** < 200 MB con los widgets de la fase 1 (se mide ~198 MB, con picos de 202 MB) y < 250 MB con los 11 widgets de la fase 4 (GD-NFR-PERF-02, SPEC v1.2).
3. **Intervalo por defecto de CPU, RAM y red: 2 s** (GD-NFR-PERF-05), para quedar bajo 2 % de CPU.

## Consecuencias

- Ya aplicadas en el código: transiciones solo de entrada en medidores y barras, y `--renderer-process-limit=1` en todas las ventanas.
- Cualquier componente nuevo que se actualice periódicamente debe pintarse sin transición; las animaciones quedan para la entrada y la interacción.
- Repetir la medición con los 3 monitores conectados antes del VoBo de la fase 1 (GD-NFR-PERF-02 en `QA_MATRIZ.md`).

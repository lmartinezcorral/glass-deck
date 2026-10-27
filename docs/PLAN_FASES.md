# Glassdeck — Plan de fases y backlog

Complementa a `SPEC.md` (sección 6). Cada bloque termina con un commit en `main`, `cargo test` y `npm run build` en verde, y sus filas actualizadas en `QA_MATRIZ.md`.

| Campo | Valor |
|---|---|
| Versión | 1.0 |
| Fecha | 2026-10-27 |
| Repo | Git local en `tools/glassdeck`, rama `main`, sin remoto |
| Ticket | `MOB-TOOL-GLASSDECK-01` (TASK.MD del multiverso y Notion) |

## Fase 0 — Preparación

| # | Entregable | Estado |
|---|---|---|
| 0.1 | Rust stable MSVC + Visual Studio Build Tools 2022 (`VCTools`) instalados y verificados | Hecho (Rust 1.99.1, MSVC 14.44, SDK 10.0.26100, WebView2 156) |
| 0.2 | Repo git local y proyecto Tauri 2 + React + TypeScript (Vite) con la estructura de `ARQUITECTURA.md` §6 | Hecho |
| 0.3 | Prueba de concepto R-01: 11 ventanas transparentes con *acrylic*, en 3 variantes, medidas con `poc/medir.ps1` | Hecho (1 monitor; repetir con 3) |
| 0.4 | `adr/ADR-001-estrategia-ventanas.md` con la medición y la estrategia elegida | Medición escrita; falta la decisión de Luis |

**Salida:** ADR-001 aceptado. Si ninguna variante baja de 200 MB, Luis decide entre agrupar widgets o subir el presupuesto.

## Fase 1 — Base

| Bloque | Contenido | Requerimientos | Estado |
|---|---|---|---|
| 1 · Núcleo | `settings.json` versionado con escritura atómica y respaldo; log rotativo con filtro; instancia única; bandeja; arranque con Windows | GD-DOCK-04/05, GD-CUST-07, GD-INST-02, GD-NFR-REL-03/04, GD-NFR-SEC-02 | Pendiente |
| 2 · Ventanas | Sin marco, sin barra de tareas, *acrylic* y desenfoque simple al arrastrar; capa de escritorio (R-05) y pin; posiciones por monitor; DPI; regreso al principal; píldora, maximizar y cerrar | GD-WIN-01..15 | Pendiente |
| 3 · Diseño | Tokens y componentes de vidrio del mockup: medidor, sparkline, gráfica, barra, píldora, diálogo, interruptor | GD-NFR-UX-01..05, GD-CUST-02 | Pendiente |
| 4 · Datos del sistema | Planificador (intervalos, batería, visibilidad), bus de eventos, recolector, widgets CPU/RAM/Disco/Red con historial de 1 h en memoria | GD-SYS-01..09, GD-NFR-PERF-01..05, GD-CUST-03 | Pendiente |
| 5 · Procesos | Política de 3 niveles revalidada en el backend; cierre amable y luego forzado; resumen por proceso; sin elevar | GD-PROC-01..09, GD-CUST-05 | Pendiente |
| 6 · Limpieza | Vista previa; solo archivos con más de 24 h; sin seguir uniones; `glassdeck-helper.exe` con acción cerrada vía UAC | GD-CLEAN-01..05, GD-NFR-SEC-05 | Pendiente |
| 7 · Dock, Customize y atajos | Dock sin cuentas; Customize; `Win+Alt+G` y `Win+Alt+P` con detección de conflictos; atajos de limpieza y procesos | GD-DOCK-01/03, GD-CUST-01..10, GD-HOT-01/02/04, GD-SHORT-01 | Pendiente |
| 8 · Instalador y cierre | NSIS por usuario sin UAC; desinstalación limpia; medición de rendimiento; comparación con el mockup; VoBo de Luis | GD-INST-01..05, GD-NFR-PERF-01/02/06, GD-NFR-SEC-01/03/06 | Pendiente |

**Salida:** instalador funcional, todos los CA de la fase en «Pasa» o «Diferido» con motivo en `QA_MATRIZ.md`, y VoBo de Luis.

## Fases 2 a 4

Se detallan al iniciar cada una, con el mismo formato. Contenido en `SPEC.md` §6.

## Puntos donde se detiene la construcción

- Instalaciones o acciones que piden UAC: requieren el clic de Luis.
- Prueba de RAM sin ninguna variante bajo 200 MB.
- Capa de escritorio que no resiste `Win+D` (R-05): se propone una alternativa antes de seguir.

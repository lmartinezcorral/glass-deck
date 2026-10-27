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
| 0.4 | `adr/ADR-001-estrategia-ventanas.md` con la medición y la estrategia elegida | Hecho: aceptado por Luis (ventana por widget; 200 MB en fase 1, 250 MB en fase 4; 2 s por defecto) |

**Salida:** ADR-001 aceptado. Si ninguna variante baja de 200 MB, Luis decide entre agrupar widgets o subir el presupuesto.

## Fase 1 — Base

| Bloque | Contenido | Requerimientos | Estado |
|---|---|---|---|
| 1 · Núcleo | `settings.json` versionado con escritura atómica y respaldo; log rotativo con filtro; instancia única; bandeja; arranque con Windows | GD-DOCK-04/05, GD-CUST-07, GD-INST-02, GD-NFR-REL-03/04, GD-NFR-SEC-02 | Implementado; pruebas automáticas pasan; pendientes manuales (bandeja, reinicio de Windows, revisiones de 1 día y 1 semana) |
| 2 · Ventanas | Sin marco, sin barra de tareas, *acrylic* y desenfoque simple al arrastrar; capa de escritorio (R-05) y pin; posiciones por monitor; DPI; regreso al principal; píldora, maximizar y cerrar | GD-WIN-01..15 | Implementado; pruebas automáticas pasan; corregidos el marco nativo visible y la salida de Alt+Tab; pendientes manuales con 3 monitores (R-05, DPI, desconexión) |
| 3 · Diseño | Tokens y componentes de vidrio del mockup: medidor, sparkline, gráfica, barra, píldora, diálogo, interruptor | GD-NFR-UX-01..05, GD-CUST-02 | Implementado; Vitest pasa (movimiento reducido, barra, píldora); vistas más fieles al mockup 01; pendientes manuales (contraste, teclado, tema en vivo, mockups 02 a 10) |
| 4 · Datos del sistema | Planificador (intervalos, batería, visibilidad), bus de eventos, recolector, widgets CPU/RAM/Disco/Red con historial de 1 h en memoria | GD-SYS-01..09, GD-NFR-PERF-01..05, GD-CUST-03 | Implementado; pruebas automáticas pasan (batería, pausa, valores por defecto, mínimos, búfer de 1 h, suma de RAM); corregido el historial que mostraba 2 h como «última hora»; pendientes manuales (comparar con el Administrador de tareas, cargador, pantalla completa) |
| 5 · Procesos | Política de 3 niveles revalidada en el backend; cierre amable y luego forzado; resumen por proceso; sin elevar | GD-PROC-01..09, GD-CUST-05 | Implementado; pruebas automáticas pasan; corregidos CERRAR exacto de punta a punta, columna Usuario y enlace de Explorer en su fila; Customize muestra los fijos con candado; pendientes manuales (Bloc de notas sin guardar, PowerShell de administrador, node) |
| 6 · Limpieza | Vista previa; solo archivos con más de 24 h; sin seguir uniones; `glassdeck-helper.exe` con acción cerrada vía UAC | GD-CLEAN-01..05, GD-NFR-SEC-05 | Implementado; pruebas automáticas pasan (vista previa, 24 h, archivo en uso, uniones en app y ayudante, acción cerrada); pendientes manuales (cancelar UAC, comparar espacio liberado, columna «Elevado») |
| 7 · Dock, Customize y atajos | Dock sin cuentas; Customize; `Win+Alt+G` y `Win+Alt+P` con detección de conflictos; atajos de limpieza y procesos | GD-DOCK-01/03, GD-CUST-01..10, GD-HOT-01/02/04, GD-SHORT-01 | Implementado; pruebas automáticas pasan (un widget desactivado no se recolecta, un atajo en conflicto no se guarda y un guardado fallido libera el atajo, la disposición por defecto cabe en el área de trabajo); GD-CUST-09 diferido a fase 2 junto con la bóveda; `Win+Alt+G` choca con la Xbox Game Bar (decisión D-22 de Luis); pendientes manuales (dock entre monitores, tema en vivo, atajos desde otras apps) |
| 8 · Instalador y cierre | NSIS por usuario sin UAC; desinstalación limpia; medición de rendimiento; comparación con el mockup; VoBo de Luis | GD-INST-01..05, GD-NFR-PERF-01/02/06, GD-NFR-SEC-01/03/06 | Pendiente |

**Salida:** instalador funcional, todos los CA de la fase en «Pasa» o «Diferido» con motivo en `QA_MATRIZ.md`, y VoBo de Luis.

## Fases 2 a 4

Se detallan al iniciar cada una, con el mismo formato. Contenido en `SPEC.md` §6.

## Puntos donde se detiene la construcción

- Instalaciones o acciones que piden UAC: requieren el clic de Luis.
- Prueba de RAM sin ninguna variante bajo 200 MB.
- Capa de escritorio que no resiste `Win+D` (R-05): se propone una alternativa antes de seguir.

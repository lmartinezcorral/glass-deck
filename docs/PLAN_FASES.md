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
| 7 · Dock, Customize y atajos | Dock sin cuentas; Customize; `Ctrl+Alt+M` (antes `Win+Alt+G`) y `Win+Alt+P` con detección de conflictos; atajos de limpieza y procesos | GD-DOCK-01/03, GD-CUST-01..10, GD-HOT-01/02/04, GD-SHORT-01 | Implementado; pruebas automáticas pasan (un widget desactivado no se recolecta, un atajo en conflicto no se guarda y un guardado fallido libera el atajo, la disposición por defecto cabe en el área de trabajo); GD-CUST-09 diferido a fase 2 junto con la bóveda; `Win+Alt+G` chocaba con la Xbox Game Bar y Luis lo cambió a `Ctrl+Alt+M` (SPEC 1.3, con migración de la configuración); pendientes manuales (dock entre monitores, tema en vivo, atajos desde otras apps) |
| 8 · Instalador y cierre | NSIS por usuario sin UAC; desinstalación limpia; medición de rendimiento; comparación con el mockup; VoBo de Luis | GD-INST-01..05, GD-NFR-PERF-01/02/06, GD-NFR-SEC-01/03/06 | Implementado; entregado para el VoBo de Luis. Instalador por usuario de 1.35 MB, sin UAC, con hook que borra configuración y logs al desinstalar si se elige; arranque hasta el primer widget de 1.5 a 3.4 s sin reiniciar Windows; CPU en reposo 1.47 %; RAM con memoria baja de WebView2 en promedio 178 MB y meseta ~205 MB contra < 200 MB (antes ~250 MB; Luis aceptó la excepción); Customize y Limpiar temporales con subtítulo; pendientes manuales (SmartScreen, reinicio de Windows, 30 min de red, mockups 03, 04 y 10) |

**Salida:** instalador funcional, todos los CA de la fase en «Pasa» o «Diferido» con motivo en `QA_MATRIZ.md`, y VoBo de Luis.

**VoBo condicionado (Luis, 2026-10-28):** la fase 2 arranca en la rama `fase-2` mientras la QA manual de la fase 1 se cierra en `main`. La fase 1 queda cerrada cuando sus filas estén en «Pasa» o «Diferido»; `fase-2` no se integra a `main` antes de eso.

## Fase 2 — Servicios

Se construye en la rama `fase-2` (worktree `tools/glassdeck-fase2`) mientras se cierra la QA de la fase 1 en `main`. Cada bloque termina con commit en `fase-2`, `cargo test` y `npm run build` en verde, y sus filas en `QA_MATRIZ.md`. Los bloques 9 a 12 no necesitan cuentas de Luis; los bloques 13 y 14 esperan su login y su cliente OAuth.

| Bloque | Contenido | Requerimientos | Necesita a Luis | Estado |
|---|---|---|---|---|
| 9 · Bóveda y red | Bóveda en el Administrador de credenciales (prefijo `Glassdeck/`, el valor nunca sale del backend ni llega a logs); cliente HTTP único con lista de dominios de SPEC §5.3; reintentos con espera de 1, 2, 5 y 15 min; `state.json` para último dato válido y avisos enviados | GD-NFR-SEC-01, GD-NFR-REL-01/02, GD-INST-03 (credenciales al desinstalar) | No | Implementado (2026-10-28); pruebas automáticas pasan (bóveda sobre keyring simulado, lista de dominios, reintentos, `state.json` con respaldo, fuentes aisladas); pendientes manuales (búsqueda de secretos en disco, red apagada, conexiones de 30 min, desinstalar con «borrar») |
| 10 · Claude Code local | Lectura incremental por posición de archivo de `~/.claude/projects/**/*.jsonl`; solo tokens, modelo, fecha y proyecto; suma por día, modelo y proyecto; tabla de precios editable; widget Spending con la sección de Claude Code | GD-CLA-01..03 | No | Backend implementado (2026-10-28): lector incremental con deduplicación, suma por día local, modelo y proyecto, costo con tabla editable, widget `spending` en el catálogo (apagado por defecto), configuración v3 con migración; pruebas automáticas pasan y la suma coincide al 100 % con una suma independiente de los registros reales; pendientes: vista del widget (subagente de interfaz) y comparación con `ccusage` |
| 11 · Clima | Open-Meteo actual, 24 h y 7 días; ubicación por `ipapi.co` (solo en modo automático) o ciudad manual con el buscador de geocodificación de Open-Meteo; último dato con su hora sin conexión; widget Clima | GD-WX-01..03, GD-CUST-08 | No | Backend implementado (2026-10-28): recolector Open-Meteo (actual, 24 h, 7 días) con iconos WMO en español, ubicación automática por `ipapi.co` (guardada 6 h en memoria) o ciudad manual, último dato con su hora en `state.json`, comandos de búsqueda y fijado, widget `clima` en el catálogo (apagado por defecto), intervalo de 30 min (mínimo 15); pruebas automáticas con respuestas grabadas pasan; pendientes: vista del widget, buscador en Customize y pruebas manuales |
| 12 · Avisos de umbral | Motor de umbrales (Cursor 50/80/100 %; Claude 80 % de 5 h y semanal), un aviso por ciclo, historial en `state.json`, notificación de Windows que abre Spending maximizado; umbrales editables | GD-ALERT-01..03, GD-CUST-04 | No (se prueba con datos simulados hasta el bloque 13) | Backend implementado (2026-10-28): motor puro con historial en `state.json` (un aviso por umbral, ciclo y medidor; se avisa el umbral más alto cruzado), umbrales y presupuesto de Cursor en la configuración, gancho `usos()` en las fuentes para que el bloque 13 solo traduzca sus datos, notificación de Windows con clic que abre Spending maximizado, comandos `avisos_probar` y `avisos_historial`; pruebas automáticas con datos simulados pasan; pendientes: prueba manual del clic con la app instalada (en Windows solo se reporta mientras el aviso está en pantalla) y datos reales |
| 13 · Cursor y claude.ai | Ventanas de login aisladas sin acceso a comandos; captura de cookie a la bóveda; adaptadores de solo lectura aislados; etiqueta «no oficial»; motivo del error y última lectura válida | GD-CUR-01..06, GD-CLA-04..06, GD-NFR-SEC-04 | Sí: iniciar sesión en cursor.com y claude.ai | Pendiente |
| 14 · Google | Importar el JSON del cliente OAuth (GUIA_GOOGLE_OAUTH.md); OAuth de escritorio con PKCE y varias cuentas; Gmail `gmail.metadata` solo no leídos de INBOX; Calendar de solo lectura; encabezados solo en memoria; alerta rotativa; modo privacidad (`Win+Alt+P`) | GD-MAIL-01..07, GD-CAL-01, GD-ROT-01..03, GD-NFR-SEC-07 | Sí: crear el cliente OAuth y conectar cuentas | Pendiente (la importación del JSON se adelanta en el bloque 15) |
| 15 · Dock y Customize | Estado de cuentas en el dock (verde, ámbar, gris; Conectar/Desconectar); Customize con precios, umbrales, ubicación, importación del cliente OAuth; exportar e importar configuración sin credenciales | GD-DOCK-02, GD-CUST-04/08/09 | Parcial (las cuentas reales llegan con 13 y 14) | Pendiente |

**Riesgos de la fase 2**

- Cursor y claude.ai pueden cambiar su web interna (R-02): adaptador aislado y aviso claro; nunca números inventados.
- La fase 2 abre conexiones salientes: GD-NFR-SEC-06 y GD-INST-04 pasan a «solo hacia SPEC §5.3»; el cliente HTTP rechaza cualquier otro dominio.
- El correo puede traer datos de pacientes (GD-NFR-SEC-07): encabezados solo en memoria, nunca en logs ni en `state.json`.
- Más widgets suben la RAM: el presupuesto de la fase 4 es < 250 MB (ADR-001); se mide al cerrar la fase.

**Salida:** instalador funcional, todos los CA de la fase en «Pasa» o «Diferido» con motivo, y VoBo de Luis. `fase-2` se integra a `main` solo después de cerrar la fase 1.

## Fases 3 y 4

Se detallan al iniciar cada una, con el mismo formato. Contenido en `SPEC.md` §6.

## Puntos donde se detiene la construcción

- Instalaciones o acciones que piden UAC: requieren el clic de Luis.
- Prueba de RAM sin ninguna variante bajo 200 MB.
- Capa de escritorio que no resiste `Win+D` (R-05): se propone una alternativa antes de seguir.

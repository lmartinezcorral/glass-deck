# Glassdeck — Arquitectura

Complementa a `SPEC.md`. Los identificadores `GD-*` remiten a sus requerimientos.

## 1. Vista general

Glassdeck es una sola app Tauri 2: un proceso principal en Rust, dueño de todos los datos y operaciones del sistema, y varias ventanas WebView2 (una por widget, más el dock, Customize y el editor de zonas) con la interfaz en React + TypeScript. La interfaz solo dibuja; nunca toca red ni disco directamente (GD-NFR-SEC-03).

```mermaid
flowchart LR
    subgraph UI["Interfaz (WebView2 · React + TS)"]
        W1["Ventanas de widgets<br/>CPU · RAM · Disco · Red · Procesos<br/>Spending · Correo/Agenda · Clima<br/>Audio · Brillo · Atajos"]
        DK["Dock"]
        CU["Customize"]
        ZE["Editor de zonas"]
        LG["Ventanas de login<br/>(cursor.com · claude.ai)<br/>aisladas, sin comandos"]
    end

    subgraph CORE["Núcleo (Rust · proceso principal)"]
        BUS["Bus de eventos<br/>(suscripciones por widget)"]
        SCH["Planificador<br/>(intervalos, batería, visibilidad)"]
        WM["Gestor de ventanas de widgets<br/>(capa, posiciones, DPI, monitores)"]
        CFG["Configuración<br/>(settings · zones · state)"]
        SEC["Bóveda de credenciales"]
        subgraph COL["Recolectores"]
            SYS["Sistema<br/>CPU · RAM · disco · red · procesos"]
            SPD["Spending<br/>Cursor · Claude Code · claude.ai · API"]
            GOO["Google<br/>Gmail · Calendar"]
            WX["Clima"]
        end
        subgraph ACT["Acciones"]
            KIL["Cierre de procesos<br/>(política 3 niveles)"]
            CLN["Limpieza de temporales"]
            SHC["Atajos del sistema"]
            AUD["Audio · micrófono · cámara"]
            BRI["Brillo"]
        end
        ZON["Motor de zonas"]
        HK["Atajos globales"]
        NTF["Notificaciones de Windows"]
    end

    subgraph WIN["Windows"]
        W32["Win32 · DWM · WinEvent"]
        PDH["Contadores de rendimiento"]
        CA["Core Audio · Media Foundation"]
        WMI["WMI · DDC/CI"]
        CM["Administrador de credenciales"]
        ELEV["Ayudante elevado<br/>(UAC, un solo uso)"]
    end

    EXT["Servicios externos<br/>Google · cursor.com · claude.ai<br/>Anthropic API · Open-Meteo"]

    W1 & DK & CU & ZE -- "comandos invoke" --> CORE
    BUS -- "eventos" --> W1 & DK
    SCH --> COL
    COL --> BUS
    SPD & GOO & WX --> EXT
    LG -. "solo sí/no: hay sesión" .-> CORE
    SEC --> CM
    SYS --> PDH & W32
    KIL & SHC & ZON & WM --> W32
    AUD --> CA
    BRI --> WMI
    CLN & AUD -- "operaciones de admin" --> ELEV
    SPD --> NTF
    HK --> W32
```

## 2. Componentes del núcleo (Rust)

| Componente | Responsabilidad | Requerimientos | Base técnica prevista |
|---|---|---|---|
| Gestor de ventanas de widgets | Crear y destruir ventanas, *acrylic* y desenfoque al arrastrar, capa de escritorio o siempre encima, posiciones por monitor, regreso al principal al desconectar | GD-WIN-*, GD-NFR-PERF-04 | `tauri` (ventanas sin marco, transparentes, `skipTaskbar`), `window-vibrancy` (`apply_acrylic` / `apply_blur`), `windows` crate (`SetWindowPos`, `WM_DISPLAYCHANGE`, `EnumDisplayMonitors`, `GetDpiForMonitor`) |
| Planificador | Un temporizador por fuente con su intervalo; pausa si nadie está suscrito, si el widget está oculto o hay app a pantalla completa; modo batería | GD-NFR-PERF-03/04/05 | `tokio` (intervalos), `GetSystemPowerStatus`, `SHQueryUserNotificationState` |
| Bus de eventos | Publica cada lectura solo a las ventanas suscritas; agrupa envíos a 1 por segundo | GD-NFR-PERF-01 | Eventos de Tauri por ventana |
| Configuración | Leer y validar `settings.json`, `zones.json`, `state.json`; migraciones por versión; escritura atómica con respaldo | GD-CUST-*, GD-NFR-REL-03 | `serde`, `serde_json` |
| Bóveda de credenciales | Guardar y leer secretos con prefijo `Glassdeck/`; nunca exponerlos a la interfaz | GD-NFR-SEC-01 | `keyring` (Administrador de credenciales) |
| Recolector de sistema | CPU total y por núcleo, RAM con desglose, disco (espacio y E/S), red por adaptador, lista de procesos con dueño | GD-SYS-*, GD-PROC-01/02 | `sysinfo` + contadores de rendimiento (PDH) para E/S de disco y caché de RAM; `GetTokenInformation` para el dueño del proceso |
| Recolector de spending | Adaptador Cursor (no oficial), lector de JSONL de Claude Code con tabla de precios, adaptador claude.ai (no oficial), cliente de la API de costos de Anthropic | GD-CUR-*, GD-CLA-* | Cursor, claude.ai y ChatGPT: WebView oculto y temporal con el perfil aislado del servicio (`fetch` same-origin por DevTools; Glassdeck no lee cookies); APIs de costos: `reqwest` con la clave de la bóveda; `serde_json` en modo flujo; lectura incremental de JSONL por posición de archivo |
| Recolector de Google | OAuth de escritorio con PKCE y redirección a `127.0.0.1`, varias cuentas, Gmail `messages.list` + `messages.get` con `format=metadata`, Calendar `events.list` | GD-MAIL-*, GD-CAL-01 | `oauth2`, `reqwest` |
| Recolector de clima | Open-Meteo actual, por hora y diario; geolocalización por IP o ciudad fija | GD-WX-* | `reqwest` |
| Rotador de alertas | Mezcla correos no leídos y eventos, prioridad a eventos en < 30 min, rotación configurable | GD-ROT-01 | En memoria |
| Política de procesos | Clasifica cada proceso en lista negra, reforzado o normal; el backend vuelve a validar al ejecutar | GD-PROC-03/04/05/08 | Lista fija compilada + lista del usuario |
| Cierre de procesos | `WM_CLOSE` a las ventanas del proceso, espera de 5 s y luego `TerminateProcess`; resultado por proceso | GD-PROC-06/07 | `EnumWindows`, `PostMessage`, `OpenProcess`, `TerminateProcess` |
| Limpieza | Vista previa (tamaños), borrado de archivos con > 24 h, omitir en uso, no seguir uniones | GD-CLEAN-* | `walkdir` sin seguir enlaces; ayudante elevado para `C:\Windows\Temp` |
| Atajos del sistema | Plan de energía, reiniciar Explorer, limpiar DNS, vaciar papelera, bloquear, suspender, Administrador de tareas | GD-SHORT-* | `PowerSetActiveScheme`, `DnsFlushResolverCache`, `SHEmptyRecycleBin`, `LockWorkStation`, `SetSuspendState` |
| Audio | Volumen y silencio general, dispositivo predeterminado, sesiones por app, salida por app, micrófono y medidor | GD-AUD-*, GD-MIC-01 | Core Audio (`IMMDeviceEnumerator`, `IAudioEndpointVolume`, `IAudioSessionManager2`, `IAudioMeterInformation`); `IPolicyConfig` para predeterminado y `IAudioPolicyConfigFactory` (no documentada) para salida por app |
| Cámara | Lista de cámaras, ajustes soportados, deshabilitar dispositivo | GD-CAM-* | Vista previa con `getUserMedia` en la ventana del widget; ajustes con `IAMVideoProcAmp` / `IAMCameraControl`; deshabilitar con SetupAPI vía ayudante elevado |
| Brillo | Pantalla interna y monitores externos | GD-BRI-* | WMI `WmiMonitorBrightnessMethods`; DDC/CI con `GetPhysicalMonitorsFromHMONITOR` + `SetVCPFeature(0x10)` (Dxva2) |
| Atajos globales | Registro y detección de conflictos | GD-HOT-*, GD-CUST-06 | `tauri-plugin-global-shortcut` (`RegisterHotKey`) |
| Notificaciones | Avisos de umbral con acción de abrir | GD-ALERT-* | `tauri-plugin-notification` |
| Motor de zonas | Huellas de monitor, cuadrículas, superposición, modificadores, memoria por app, recuperar todo | GD-ZONE-* | Ver sección 5 |
| Ciclo de vida | Instancia única, arranque con Windows, bandeja | GD-DOCK-04/05, GD-INST-* | `tauri-plugin-single-instance`; tarea programada al iniciar sesión registrada con la API COM del Programador de tareas (`ITaskService`, XML en memoria; `arranque.rs`) y `tauri-plugin-autostart` (clave Run) como respaldo; bandeja de Tauri |

## 3. Interfaz (React + TypeScript)

- **Una base de código y varios puntos de entrada:** cada ventana carga la misma app con un parámetro `?vista=cpu|ram|…|dock|customize|zonas`. Así se comparten componentes, tema y estado.
- **Sistema de diseño de vidrio:** variables CSS para el tema (oscuro y claro), el acento y el desenfoque; componentes de medidor circular, sparkline, barra, lista, píldora y diálogo de confirmación. El *acrylic* lo pone Windows; la interfaz dibuja el brillo superior, el borde de luz y el reflejo que sigue al mouse.
- **Gráficas:** SVG o canvas propios y ligeros, sin librerías pesadas, para cuidar la RAM.
- **Estado:** cada widget se suscribe solo a su fuente; el historial de 1 h lo guarda el núcleo en un búfer circular y la interfaz lo pide al maximizar.
- **Accesibilidad:** `aria-label` en botones de icono, foco visible, `prefers-reduced-motion`.

### 3.1 Contrato de la fase 2

Acuerdo entre el núcleo (bloques 9 a 12) y las vistas de Spending, Clima, Customize y dock. Los tipos de referencia viven en `src/api.ts`; este apartado resume qué existe, quién puede llamarlo y cómo se comporta. La interfaz no hace red: todo llega por comandos o eventos (CSP estricta).

**Fuentes de servicio.** Cada fuente corre en su propio hilo y solo recolecta mientras su widget está activo y la recolección no está en pausa. Encender el widget (o abrir Spending desde un aviso) fuerza una lectura inmediata.

| Fuente | Widget | Intervalo | Se guarda en `state.json` | Datos (`EstadoFuente<T>.datos`) |
|---|---|---|---|---|
| `claude_code` | `spending` | `intervalos.spending_ms` (15 min; mínimo 5) | No (los nombres de proyecto podrían ser personales; se recalcula al arrancar) | `ResumenClaudeCode` |
| `codex` | `spending` | `intervalos.spending_ms` | No (se recalcula al arrancar) | `ResumenCodex` |
| `clima` | `clima` | `intervalos.clima_ms` (30 min; mínimo 15) | Sí (sin coordenadas) | `Clima` |

**Comandos nuevos.** Todos devuelven error como texto en español listo para mostrar.

| Comando (`api.*`) | Ventanas | Parámetros | Respuesta | Notas |
|---|---|---|---|---|
| `catalogo_widgets` (`catalogoWidgets`) | paneles y widgets | — | `DefWidget[]` | Título, subtítulo y si arranca encendido; sustituye las listas fijas del dock y Customize |
| `servicio_ultimo` (`servicioUltimo`) | paneles y widgets | `fuente: 'claude_code' \| 'clima'` | `EstadoFuente<T> \| null` | Estado inicial al montar la vista; luego llegan eventos |
| `boveda_estado` (`bovedaEstado`) | paneles | — | `EstadoCredencial[]` | Solo `presente: boolean`; nunca el valor |
| `boveda_guardar` (`bovedaGuardar`) | paneles | `clave`, `valor` | `void` | Solo `anthropic_admin`; las demás se obtienen con su inicio de sesión |
| `boveda_borrar` (`bovedaBorrar`) | paneles | `clave` | `void` | Borrar una que no existe no es error |
| `clima_buscar_ciudad` (`climaBuscarCiudad`) | paneles | `texto` (2 a 80 caracteres) | `Ubicacion[]` (máximo 8) | Fuera de rango: error «Escribe entre 2 y 80 caracteres»; sin red: «Sin conexión» |
| `clima_fijar_ubicacion` (`climaFijarUbicacion`) | paneles | `ubicacion: Ubicacion \| null` | `Config` | `null` vuelve a la ubicación automática por IP; fuerza una lectura del clima y emite `gd://config` |
| `avisos_probar` (`avisosProbar`) | paneles | — | `void` | Notificación de muestra; su clic abre Spending maximizado; no entra al historial |
| `avisos_historial` (`avisosHistorial`) | paneles | — | `AvisoEnviado[]` | Del más reciente al más viejo; `clave` = `medidor:ciclo:umbral` |

«Paneles» son el dock, Customize y Limpieza (`capabilities/paneles.json`); «widgets» son las ventanas `w-*` (`capabilities/widgets.json`).

**Configuración v3.** `config_guardar` sigue siendo la vía para estos campos (Customize). Cambiar `precios_modelos` recalcula Claude Code y cambiar `clima` refresca el clima de inmediato; los umbrales se aplican en la siguiente evaluación.

```ts
intervalos: { …; spending_ms: number; clima_ms: number };  // opciones: 5/15/30/60 min y 15/30/60 min
precios_modelos: PrecioModelo[];   // USD por millón de tokens; patron en minúsculas
clima: { modo: 'automatica' | 'manual'; ubicacion: Ubicacion | null };
umbrales: {
  notificaciones: boolean;
  cursor: number[];          // [50, 80, 100]
  claude_5h: number[];       // [80]
  claude_semanal: number[];  // [80]
  presupuesto_cursor_usd: number | null;  // si existe, el % de Cursor se mide contra él
};
```

**Eventos.**

| Evento | Destino | Carga | Cuándo |
|---|---|---|---|
| `gd://datos` | Solo la ventana del widget de la fuente (`w-spending`, `w-clima`) | `LecturaServicio` = `{ fuente, datos: EstadoFuente<T> }` | Tras cada lectura, buena o fallida |
| `gd://aviso` | Todas las ventanas | `Aviso` | Al mandar una notificación de umbral |
| `gd://config` | Todas las ventanas | `null` | Igual que en la fase 1; también tras `clima_fijar_ubicacion` |

```ts
interface EstadoFuente<T> {
  datos: T | null;                 // último dato válido; se conserva aunque la última lectura falle
  hora_ms: number | null;          // hora de ese dato (epoch ms); mostrar «hace X min»
  error: Motivo | null;            // motivo de la última lectura fallida, o null
  texto_error: string | null;      // «Sin conexión», «Formato cambió», «Sesión expirada», «Servicio no disponible»,
                                   // «Dominio no permitido», «Sin ubicación», «Error interno»
  proximo_intento_ms: number | null;  // reintento programado (1, 2, 5 y 15 min, sin pasar del intervalo)
}
```

- **`ResumenClaudeCode`:** `hoy` (AAAA-MM-DD local), `total_hoy` y `total_mes` (`{ tokens, costo_usd }`), `dias` (siempre 30, del más viejo a hoy, para la gráfica), `filas` (día × modelo × proyecto de esos 30 días) y `sin_precio` (modelos sin precio en la tabla; Customize debería invitar a agregarlo).
- **`Clima`:** `lugar` (nombre, región, país, `automatica`), `actual` (temperatura, sensación, humedad, viento en km/h, `icono`, `descripcion`, `es_dia`, `maxima` y `minima` del día), `horas` (24 desde la hora en curso) y `dias` (7). `icono` es una clave fija: `despejado`, `parcial`, `nublado`, `niebla`, `llovizna`, `lluvia`, `chubascos`, `nieve`, `tormenta` o `desconocido`; con `es_dia: false` la vista elige la variante nocturna.
- **`Aviso`:** `medidor` (`cursor`, `claude_5h`, `claude_semanal`), `ciclo`, `umbral`, `porcentaje`, `titulo` y `cuerpo` ya redactados.

**Avisos (GD-ALERT-*).** Un aviso por umbral, ciclo y medidor; si una lectura cruza varios umbrales se avisa solo el más alto. El historial se escribe en `state.json` antes de notificar, así un reinicio no repite avisos. Las fuentes de gasto del bloque 13 solo implementan `Fuente::usos()`; el planificador evalúa cada lectura buena. En Windows el clic en la notificación abre Spending maximizado únicamente mientras el aviso está en pantalla; desde el Centro de actividades no se reporta (limitación de `tauri-plugin-notification`).

## 4. Seguridad

```mermaid
flowchart TB
    subgraph Bajo["Sin privilegios (siempre)"]
        APP["Glassdeck.exe<br/>núcleo Rust"]
        WV["WebView2 de widgets<br/>CSP estricta · solo comandos permitidos"]
        LW["WebView2 de login y lectura<br/>sin comandos · perfil aislado por servicio"]
    end
    subgraph Alto["Elevado (bajo demanda, un solo uso)"]
        H["glassdeck-helper.exe<br/>acciones cerradas:<br/>limpiar C:\\Windows\\Temp ·<br/>habilitar/deshabilitar cámara"]
    end
    WV -- "invoke validado" --> APP
    LW -. "solo sí/no y el JSON de uso, por DevTools" .-> APP
    APP -- "UAC + parámetros validados" --> H
    APP --> CM["Administrador de credenciales"]
```

- **Capacidades de Tauri por ventana:** los widgets solo pueden invocar los comandos de su módulo; Customize y el dock tienen los de configuración; las ventanas de login no tienen ninguno.
- **Ayudante elevado:** ejecutable aparte con un conjunto cerrado de acciones. No acepta rutas ni comandos arbitrarios: recibe un código de acción y el núcleo valida todo antes de pedir UAC.
- **Sesiones web sin cookies (SPEC 1.6, R-10):** Glassdeck no lee, copia ni guarda cookies. La sesión de Cursor, claude.ai y ChatGPT vive solo en `%LOCALAPPDATA%\Glassdeck\perfiles\<servicio>`. Durante el login, cada ~3 s y solo en el origen del servicio, el núcleo pregunta a la página con un `fetch` same-origin si ya hay sesión (la respuesta es solo `true`/`false`); al confirmar cierra la ventana y guarda en `state.json` una marca no secreta. Cada lectura abre un WebView oculto y temporal con ese perfil (aterriza en `favicon.ico`, una ronda por servicio y en serie), hace el `fetch` con `credentials: 'include'` y devuelve solo estado, tres encabezados y el JSON; se destruye al terminar la ronda. El token de ChatGPT se pide y se usa dentro de la página. «Desconectar» borra la marca y la carpeta del perfil cuando ningún WebView lo usa.
- **Logs:** pasan por un filtro que enmascara secretos, asuntos, títulos y rutas de usuario.

## 5. Motor de zonas (fase 4)

Diseño propio inspirado en el comportamiento de MaxTo (ver `referencias/MAXTO_ANALISIS.md`).

```mermaid
flowchart LR
    DM["Monitores<br/>huellas · DPI · área de trabajo<br/>(antirrebote 500 ms)"] --> ZS["Almacén de zonas<br/>cuadrícula por huella<br/>presets · reglas por app"]
    WE["Eventos de ventanas<br/>SetWinEventHook:<br/>mostrar · foco · inicio/fin de arrastre"] --> RULE["Reglas por app<br/>(ruta del ejecutable + clase)"]
    WE --> DRAG["Ajuste al arrastrar<br/>(Shift + barra de título)"]
    HK2["Atajos de zonas"] --> OV["Superposición de colocación"]
    OV & DRAG & RULE --> PL["Colocador<br/>compensación de bordes DWM<br/>SetWindowPlacement / SetWindowPos"]
    ZS --> OV & DRAG & RULE
    DM -- "reconexión" --> RA["Recuperar todo"] --> PL
    PL --> EL{"¿Ventana elegible<br/>y no elevada?"}
    EL -- "no" --> AV["Aviso a Luis"]
```

- **Superposición:** una ventana WebView2 transparente por monitor, creada al abrir la superposición y destruida al cerrarla para no consumir RAM en reposo. Los modificadores se leen con eventos de teclado de esa ventana, que tiene el foco mientras está abierta.
- **Ajuste al arrastrar:** `EVENT_SYSTEM_MOVESIZESTART` y `EVENT_SYSTEM_MOVESIZEEND`. Durante el arrastre, un temporizador de 16 ms lee la posición del cursor y el estado de `Shift` para iluminar la zona. No se usan ganchos de bajo nivel (GD-ZONE-14).
- **Compensación de bordes:** se calcula la diferencia entre `GetWindowRect` y los límites extendidos del marco de DWM y se ajusta el rectángulo destino.
- **Exclusiones:** ventanas de Glassdeck, del shell, herramientas (`WS_EX_TOOLWINDOW`), invisibles, minimizadas, sin título, a pantalla completa y de procesos elevados.

## 6. Estructura del repositorio prevista

```text
tools/glassdeck/
├── docs/                      # este paquete de especificación
├── src/                       # interfaz React + TypeScript
│   ├── vistas/                # cpu, ram, disco, red, procesos, spending, correo, clima, audio, brillo, atajos, dock, customize, zonas
│   ├── componentes/           # medidor, sparkline, pildora, dialogo, vidrio…
│   └── tema/
├── src-tauri/
│   ├── src/
│   │   ├── ventanas/          # gestor de ventanas de widgets
│   │   ├── recolectores/      # sistema, spending, google, clima
│   │   ├── acciones/          # procesos, limpieza, atajos, audio, camara, brillo
│   │   ├── zonas/
│   │   ├── config/
│   │   └── seguridad/         # bóveda, filtro de logs
│   ├── capabilities/          # permisos por ventana
│   └── tauri.conf.json
└── helper/                    # glassdeck-helper.exe (acciones elevadas)
```

## 7. Entorno de desarrollo (fase 0)

Estado revisado el 2026-10-27, antes de instalar (83 GB libres en `C:`).

| Requisito | Estado en el equipo de Luis | Acción |
|---|---|---|
| Node 24 + npm | Instalado (v24.14.1) | — |
| Rust (stable, `x86_64-pc-windows-msvc`) | No instalado | `winget install Rustlang.Rustup` (por usuario, sin UAC) |
| Visual Studio Build Tools 2022 con el workload `VCTools` | No instalado (~6 GB) | `winget install Microsoft.VisualStudio.2022.BuildTools` con `--add Microsoft.VisualStudio.Workload.VCTools` (pide UAC) |
| WebView2 Runtime | Incluido en Windows 10 actualizado | Verificar versión en el registro |
| Control de versiones | — | Git local en `tools/glassdeck`, rama `main`, sin remoto |

Los resultados de la prueba de RAM y la estrategia de ventanas elegida quedan en `adr/ADR-001-estrategia-ventanas.md`.

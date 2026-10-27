# Análisis de MaxTo 3 (referencia para el módulo Zonas de Glassdeck)

| Campo | Valor |
|---|---|
| Fecha | 2026-10-20 |
| Objeto | MaxTo para Windows, build portable x64 estable (`MaxTo-win-x64-stable-Portable.zip`, SHA-256 inicia `767B16791A06B5A6`) |
| Herramienta | REA (`rea-agents` 6.7.0), inspección estática .NET: `inspect-managed-artifact`, `inspect-managed-members`, `inspect-managed-native-boundaries` |
| Alcance | Comportamiento y arquitectura. Solo lectura de metadatos (tipos, firmas, textos, importaciones de Win32). Sin ejecutar ni modificar el binario |
| Exclusiones | No se analizó ni se tocó el licenciamiento (`Domino.Rett.dll`, `LicenseService`, `TrialService`). No se copia código: Glassdeck se implementa desde cero en Rust/TypeScript |
| Evidencia cruda | Local, fuera del repo: `%LOCALAPPDATA%\glassdeck-lab\maxto\rea\` |

## 1. Composición del producto

- **Stack:** .NET moderno + WPF, UI con `Wpf.Ui` (Fluent) e `iNKORE.UI.WPF.Modern`. MVVM con `CommunityToolkit.Mvvm`, inyección de dependencias y hosting de `Microsoft.Extensions`, logs con Serilog, actualizaciones con Velopack (fuente GitHub).
- **Ensamblados propios:**
  - `MaxTo.dll` (232 tipos, 1 705 métodos): UI, servicios, ajustes, colocación.
  - `MaxTo.Native.dll` (91 tipos, 577 métodos): toda la interacción con Win32 (ventanas, monitores, DPI), generada en parte con CsWin32 (`Windows.Win32.*`).
  - `Domino.Rett.dll`: licencias (fuera de alcance).
- **Atajos globales:** librería abierta NHotkey (envoltura de `RegisterHotKey`).
- **Persistencia:** `%AppData%\MaxTo\settings.json` (JSON con recarga en caliente por `FileSystemWatcher`: `ConfigurationStore`, `ReadAllTextWithRetry`), `state.db` y `maxto-log.json`.

## 2. Modelo funcional observado

| Concepto MaxTo | Evidencia | Qué hace |
|---|---|---|
| Superposición de redimensionado | `ResizeWindow`, `ResizeViewModel` (77 métodos), `OpenResizeAction`, textos `Resize_Legend_*` | Un atajo abre una superposición sobre el monitor; se elige un rectángulo de celdas de la cuadrícula arrastrando y la ventana activa se coloca ahí |
| Cuadrícula por monitor | `GridSettings`, `Columns`, `Rows`, `MaxColumns`, `MaxRows`, `Resize_Columns_Tooltip` | Filas y columnas configurables por monitor, ajustables desde la propia superposición (`Resize_QuickSettings_Header`) |
| Modos con modificador | `ResizeModifierMode`, `GetAsyncKeyState`, `IsRememberMode`, `IsCreatePresetMode`, `Resize_ModeBadge_*`, `ModifierBadgeWindow` | Mantener una tecla durante la selección cambia el modo: **Shift = recordar la ventana para esa app**, otro modificador = **crear preset**. Una insignia flotante muestra el modo activo |
| Cambiar de monitor | `Resize_Legend_SwitchDisplay`, `SwitchMonitorPage`, `ShowSwitchMonitorDot` | Con la superposición abierta, PageUp/PageDown mueve la selección al monitor siguiente/anterior |
| Presets | `RegionPreset`, `PresetPreviewViewModel`, `Applied region preset with key {Key} for display {Display}` | Rectángulos guardados por monitor, cada uno con atajo propio; se previsualizan antes de aplicar |
| Recordar por aplicación | `ApplicationRule`, `ApplicationRuleMatcher`, `ApplicationNameResolver`, `AutoPlacementService`, `PlacementTrigger` | Regla por app (resuelta por ruta de proceso/clase de ventana). Cuando aparece una ventana nueva de esa app, se coloca sola |
| Recuperar | `RecallAllAction`, `RecallWindowAction`, `Recall all: {Count} windows placed` | Recolocar todas las ventanas (o la activa) según sus reglas, p. ej. tras desconectar monitores |
| Ajustar a la cuadrícula | `FitWindowAction`, `GridFitMath`, `Fit {Class} to grid cells {GridRect} -> {Rect}` | Encaja la ventana activa en las celdas más cercanas a su posición actual |
| Mover desde la bandeja | `TrayMoveGroupViewModel`, `Tray Move-to preview overlay` | Menú de la bandeja con presets por monitor y vista previa antes de confirmar |
| Huella de monitores | `DisplayFingerprint`, `DisplayFingerprintMatcher`, `Display.SerialNumber`, `HorizontalDpi`, `ShareDisplaySettingsBy` | Cada monitor se reconoce por número de serie, nombre y DPI; la configuración se guarda por huella, no por posición |
| Ventanas elegibles | `WindowEligibility.IsPlacementCandidate`, `HasHigherPrivileges` | Filtra ventanas que no deben moverse; avisa si la ventana es de un proceso elevado (UIPI impide moverla) |
| Onboarding | `MoveWindowPage`, `SwitchMonitorPage`, `SavePresetPage` | Tutorial en 3 pasos: mover una ventana, cambiar de monitor, guardar un preset |

Atajos configurables: `Resize`, `FitWindow`, `RecallAll`, `RecallWindow` y uno por preset. No se encontraron combinaciones por defecto en los textos (se asignan en el onboarding/ajustes).

## 3. Técnica en Win32 (de `MaxTo.Native.dll`)

| Necesidad | APIs usadas | Nota para Glassdeck |
|---|---|---|
| Detectar ventanas nuevas y cambio de foco | `SetWinEventHook` / `UnhookWinEvent` (`ForegroundDetection`, "Window-shown tracking hook") | Hook de eventos fuera de proceso; no usa ganchos de teclado/mouse de bajo nivel |
| Enumerar y describir ventanas | `EnumWindows`, `EnumDesktopWindows`, `GetWindowText`, `GetClassName`, `GetWindowThreadProcessId`, `GetAncestor`, `GetShellWindow`, `IsWindowVisible`, `IsIconic`, `GetWindowLong` | Base para la elegibilidad (excluir shell, herramientas, invisibles, minimizadas) |
| Mover y restaurar | `SetWindowPos`, `GetWindowPlacement` / `SetWindowPlacement`, `ShowWindow`, `SetForegroundWindow` | `SetWindowPlacement` para respetar estados maximizado/normal |
| Bordes invisibles de Windows 10 | `DwmGetWindowAttribute` (límites extendidos del marco) | Compensar el borde invisible de ~7 px para que la ventana encaje exacto en la zona |
| Proceso y elevación | `OpenProcess`, `QueryFullProcessImageName`, `OpenProcessToken`, `GetTokenInformation` | Identificar la app (ruta del ejecutable) y saber si está elevada |
| Monitores | `EnumDisplayMonitors`, `GetMonitorInfo`, `MonitorFromWindow`, `MonitorFromPoint`, `EnumDisplayDevicesW`, `GetDpiForMonitor`, `GetSystemMetrics` | `DisplayManager` emite `Connected` / `Disconnected` / `Changed` con antirrebote (`DebounceDispatcher`) |
| Superposición de vista previa | `CreateWindowEx`, `UpdateLayeredWindow`, `CreateCompatibleDC`, `SelectObject`, `BlendFunction` | Ventana en capas con alfa por píxel para el resaltado de zonas sin robar el foco |
| Cursor | `GetCursorPos`, `ClientToScreen` | Posición para decidir el monitor de la superposición |
| Atajos | NHotkey → `RegisterHotKey` | En Rust: `tauri-plugin-global-shortcut` o `RegisterHotKey` directo |

## 4. Qué adoptamos y qué cambiamos en Glassdeck

**Adoptamos (comportamiento):**
- Superposición invocada por atajo, selección de celdas arrastrando, insignia de modo con modificadores, PageUp/PageDown entre monitores.
- Cuadrícula por monitor guardada por **huella** (serie + nombre + DPI), presets con atajo y vista previa, reglas por app con colocación automática al aparecer la ventana, "recuperar todo" y "ajustar a la cuadrícula".
- Detección por `SetWinEventHook`, compensación de bordes DWM, aviso de ventanas elevadas, antirrebote en cambios de monitor.

**Cambiamos / añadimos:**
- UI de vidrio oscuro de Glassdeck (React) en lugar de Fluent/WPF; el editor muestra los 3 monitores a la vez.
- Además de la superposición por atajo, **Shift + arrastrar la barra de título** de cualquier ventana muestra las zonas en vivo y la suelta en la zona bajo el cursor (MaxTo lo resuelve dentro de su superposición). Requiere seguir el arrastre con `SetWinEventHook(EVENT_SYSTEM_MOVESIZESTART/END)` + posición del cursor; sin ganchos de bajo nivel.
- Zonas libres (rectángulos arbitrarios) además de la cuadrícula uniforme.
- Integración con los widgets: los widgets de Glassdeck se excluyen del módulo Zonas y tienen su propio anclaje.

## 5. Pendiente para la fase 4

- Observación en vivo de MaxTo (versión de prueba de 14 días) para medir tiempos de la superposición, gestos exactos y comportamiento con escalados mixtos (100 % / 125 % / 150 %) en tus 3 monitores. Se hará al iniciar la fase 4 para que la prueba no venza antes.

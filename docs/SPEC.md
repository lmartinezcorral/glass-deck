# Glassdeck — Especificación de requerimientos

| Campo | Valor |
|---|---|
| Versión | 1.1 |
| Fecha | 2026-10-27 |
| Dueño | Luis Ángel Martínez Corral |
| Estado | **Aprobado por el CEO el 2026-10-27.** Construcción en curso (fases 0 y 1) |
| Paquete | `SPEC.md` (este), `ARQUITECTURA.md`, `FLUJOS.md`, `GUIA_GOOGLE_OAUTH.md`, `PLAN_FASES.md`, `QA_MATRIZ.md`, `adr/`, `mockups/glassdeck_mockup.html` + `mockups/capturas/`, `referencias/MAXTO_ANALISIS.md` |

### Historial de cambios

| Versión | Fecha | Cambio |
|---|---|---|
| 1.0 | 2026-10-25 | Borrador para VoBo, a partir del interrogatorio Q1–Q32 |
| 1.1 | 2026-10-27 | VoBo del CEO. Se fija `ipapi.co` como geolocalización por IP. La fase 0 entrega la decisión de R-01 en `adr/ADR-001-estrategia-ventanas.md`. Se agregan `PLAN_FASES.md` (backlog por bloques) y `QA_MATRIZ.md` (verificación de cada CA). El repo es git local en `tools/glassdeck`, rama `main`, sin remoto |

## 0. Resumen

Glassdeck es un panel de widgets flotantes de vidrio oscuro para el escritorio de Windows 10. Muestra el rendimiento del equipo, el gasto y uso de Cursor y Claude, una alerta rotativa de Gmail y Google Calendar, el clima, y controles de audio, cámara, brillo y atajos del sistema. Incluye un módulo **Zonas** para acomodar ventanas en 3 monitores, inspirado en MaxTo.

- **Usuario único:** Luis, en su ThinkPad con Windows 10 Pro 22H2 (build 19045) y 3 monitores.
- **Proyecto independiente** en `tools/glassdeck/`, fuera de las marcas Owame, Wellhu, Newachi y Somos Mobility. «Glassdeck» es nombre provisional.
- **Uso personal:** instalador sin firma digital, sin actualización automática, sin telemetría.

### Fuera de alcance (v1)

- Cambiar el límite de gasto de Cursor (se evalúa en v2, con confirmación).
- Avisos por Telegram (solo notificación de Windows).
- Temperatura de CPU y red por proceso.
- «Liberar RAM».
- Cuerpo de los correos; responder o archivar correos; crear eventos.
- Correo distinto de Gmail (Outlook, IMAP).
- macOS o Linux.

## 1. Decisiones de diseño (trazabilidad con el interrogatorio)

| ID | Decisión | Origen |
|---|---|---|
| D-01 | Tauri 2 + React + TypeScript; lado nativo en Rust | Q1 |
| D-02 | Cada widget es una ventana flotante independiente: minimizar a píldora, maximizar a vista detallada, cerrar oculta | Q2 |
| D-03 | Capa de escritorio por defecto (como Rainmeter), «siempre encima» opcional por widget, arranque con Windows | Q3 |
| D-04 | Dock flotante + icono en la bandeja de Windows como respaldo | Q4 |
| D-05 | Widgets en cualquiera de los 3 monitores con posición recordada; si se desconecta un monitor, vuelven al principal | Q5 |
| D-06 | Cursor: login web y cookie de sesión, solo lectura, marcado «no oficial» | Q6, Q18 |
| D-07 | Claude: registros locales de Claude Code + sesión de claude.ai; consola de la API de Anthropic opcional | Q7 |
| D-08 | Gmail + Calendar, varias cuentas de Google, proyecto propio de Google Cloud en «producción sin verificar» | Q8 |
| D-09 | Alerta sin cuerpo (remitente, asunto, hora), rotación cada 10 min, botón de privacidad | Q9 |
| D-10 | Cerrar procesos y limpiar temporales siempre con vista previa y confirmación | Q10 |
| D-11 | Clima con Open-Meteo, ciudad por IP o fijada a mano | Q11 |
| D-12 | Brillo independiente por monitor (DDC/CI en externos) | Q12 |
| D-13 | Mezclador por aplicación con salida propia por app, micrófono y cámara completos | Q13 (ampliada), Q22, Q23 |
| D-14 | Solo uso personal, instalador sin firma, sin actualización automática | Q14 |
| D-15 | Presupuesto: CPU < 2 % en reposo, RAM < 200 MB, animaciones pausadas con batería | Q16 |
| D-16 | Vidrio oscuro que sigue el modo de Windows, acento configurable | Q17 |
| D-17 | Avisos de umbral de gasto solo por notificación de Windows | Q19 |
| D-18 | Módulo Zonas tipo MaxTo, construido en la fase 4 | Q20, Q21 |
| D-19 | Protección de procesos en tres niveles; se pueden agregar procesos a la lista negra, no quitar los fijos | Q24 |
| D-20 | Contenido de widgets según la tabla de Q25 | Q25 |
| D-21 | Atajos del sistema de Q26 | Q26 |
| D-22 | Atajos globales `Win+Alt+G`, `Win+Alt+P`, `Win+Alt+Z` con detección de conflictos | Q27 |
| D-23 | Construcción en 4 fases, con instalador funcional al cierre de cada una | Q29 |

## 2. Convenciones

- Los requerimientos usan el formato `GD-<MÓDULO>-NN`. Cada uno trae un criterio de aceptación (**CA**) verificable.
- Prioridad: **M** = obligatorio en su fase, **D** = deseable (puede pasar a la fase siguiente sin bloquear la entrega).
- Fase: 1 a 4 (ver sección 6).
- Todo el texto de la interfaz está en español.

## 3. Requerimientos funcionales

### 3.1 Ventanas y comportamiento de widgets (`GD-WIN`) — Fase 1

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-WIN-01 | Cada widget es una ventana sin marco, transparente y sin botón en la barra de tareas | El widget no aparece en Alt+Tab ni en la barra de tareas; sus esquinas son redondeadas sin borde de Windows | M |
| GD-WIN-02 | Fondo de vidrio con *acrylic* de Windows 10 y capa de brillo y reflejo dibujada por la interfaz | Detrás del widget se ve el escritorio desenfocado; el brillo superior y el borde de luz son visibles en tema oscuro y claro | M |
| GD-WIN-03 | Mientras se arrastra un widget, el *acrylic* cambia a desenfoque simple y se restaura al soltar | Arrastrar un widget de lado a lado de un monitor no produce saltos visibles (> 100 ms por cuadro) | M |
| GD-WIN-04 | Arrastre libre con el mouse desde la barra superior del widget, incluso entre monitores | Se puede mover un widget a cualquiera de los 3 monitores; al soltarlo queda en su lugar | M |
| GD-WIN-05 | Barra superior con icono, título, **pin** (siempre encima), **minimizar**, **maximizar** y **cerrar**; los botones aparecen al pasar el mouse | Los 4 botones son accesibles con mouse y teclado y tienen `aria-label` | M |
| GD-WIN-06 | Minimizar convierte el widget en una **píldora** (icono + dato clave, p. ej. «CPU 23 %») en la misma posición | Clic en la píldora restaura el widget en su tamaño anterior | M |
| GD-WIN-07 | Maximizar abre la **vista detallada** del widget (tabla Q25), centrada en el monitor donde está | `Esc` o el botón restaurar vuelve a la vista normal en la posición original | M |
| GD-WIN-08 | Cerrar oculta el widget y lo marca como apagado en el dock y en Customize | El widget se reactiva desde el dock o Customize en su última posición | M |
| GD-WIN-09 | Capa de escritorio por defecto: el widget queda detrás de las ventanas normales y sigue visible con `Win+D` | Con una app maximizada, el widget no se ve; con `Win+D`, los widgets siguen visibles | M |
| GD-WIN-10 | «Siempre encima» por widget (pin) | Con el pin activo, el widget queda sobre las ventanas normales sin robar el foco | M |
| GD-WIN-11 | Posición, tamaño, estado (normal, píldora, oculto) y pin se recuerdan por widget y por monitor | Tras reiniciar Windows, todos los widgets aparecen donde estaban | M |
| GD-WIN-12 | Si se desconecta un monitor, sus widgets pasan al monitor principal sin superponerse; al reconectarlo regresan a su lugar | Desconectar y reconectar el monitor 3 mueve sus widgets al principal y luego de vuelta en < 3 s | M |
| GD-WIN-13 | Ajuste magnético opcional: los widgets se alinean a los bordes de la pantalla y entre sí (8 px) | Con la opción activa, soltar un widget a < 12 px de otro lo alinea | D |
| GD-WIN-14 | Escalado correcto con DPI distinto por monitor (100 %, 125 %, 150 %) | Un widget movido entre monitores con distinto escalado conserva su tamaño visual y su texto nítido | M |
| GD-WIN-15 | Los widgets no interceptan clics fuera de su área visible | Clic en el escritorio junto a un widget selecciona iconos del escritorio con normalidad | M |

### 3.2 Dock y bandeja (`GD-DOCK`) — Fase 1 (cuentas en fase 2)

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-DOCK-01 | Dock flotante de vidrio, arrastrable, con: interruptor por widget, estado de cuentas, Customize y Zonas | Cada acción del dock funciona y su estado se refleja en tiempo real | M |
| GD-DOCK-02 | Estado de cuentas: Google (una línea por cuenta), Cursor y Claude, con punto verde (conectada), ámbar (requiere reconexión) o gris (sin conectar) y botón Conectar/Desconectar | Al expirar una sesión, el punto pasa a ámbar en el siguiente ciclo de actualización | M (fase 2) |
| GD-DOCK-03 | El dock se puede ocultar y fijar a un borde de cualquier monitor | Con el dock oculto, el icono de la bandeja sigue disponible | M |
| GD-DOCK-04 | Icono en la bandeja de Windows con menú: Mostrar/ocultar todo, Customize, Zonas, Modo privacidad, Pausar, Salir | Todas las opciones del menú funcionan aunque el dock esté oculto | M |
| GD-DOCK-05 | Una sola instancia de Glassdeck | Abrir Glassdeck de nuevo enfoca la instancia existente en lugar de crear otra | M |

### 3.3 Customize (`GD-CUST`) — Fase 1 (se amplía en cada fase)

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-CUST-01 | Activar o desactivar cada widget y el módulo Zonas | Desactivar un widget detiene su recolección de datos (no solo lo oculta) | M |
| GD-CUST-02 | Tema: vidrio oscuro por defecto, seguir modo claro/oscuro de Windows, color de acento, intensidad del desenfoque y opacidad | El cambio se aplica en vivo a todos los widgets | M |
| GD-CUST-03 | Intervalos de actualización editables con los valores por defecto de la sección 4.1 y mínimos seguros | No se aceptan intervalos menores al mínimo (CPU/RAM/red 1 s, procesos 2 s, correo 2 min, spending 5 min) | M |
| GD-CUST-04 | Umbrales de gasto: Cursor 50/80/100 %; Claude 80 % del límite de 5 h y del semanal; presupuesto mensual de Cursor editable | Cambiar un umbral surte efecto en el siguiente ciclo | M (fase 2) |
| GD-CUST-05 | Lista negra de procesos: muestra los fijos con candado; permite agregar y quitar procesos personalizados | Los fijos no tienen botón de quitar; los agregados se respetan en GD-PROC | M |
| GD-CUST-06 | Atajos de teclado editables con detección de conflictos | Si `RegisterHotKey` falla, la interfaz muestra «Este atajo lo usa otro programa» y no lo guarda | M |
| GD-CUST-07 | Arranque con Windows (activado por defecto), capa por defecto (escritorio o siempre encima), pausar animaciones con batería (activado) | Las opciones persisten y se aplican al reiniciar | M |
| GD-CUST-08 | Ubicación del clima: automática por IP o ciudad manual con buscador | Al fijar «Chihuahua», el clima corresponde a esa ciudad | M (fase 2) |
| GD-CUST-09 | Exportar e importar la configuración (sin credenciales) a un archivo JSON | El archivo exportado no contiene tokens ni cookies | D |
| GD-CUST-10 | Restablecer posiciones de widgets | Todos los widgets vuelven a una disposición por defecto en el monitor principal | M |

### 3.4 Sistema: CPU, RAM, disco y red (`GD-SYS`) — Fase 1

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-SYS-01 | CPU normal: medidor circular animado con % total | El valor difiere < 5 puntos del Administrador de tareas en un muestreo de 1 min | M |
| GD-SYS-02 | CPU maximizado: gráfica por núcleo lógico e historial de 1 h | Se ven todos los núcleos lógicos del equipo y 3 600 puntos de historial | M |
| GD-SYS-03 | RAM normal: medidor y GB usados de GB totales | Coincide con el Administrador de tareas ± 0.3 GB | M |
| GD-SYS-04 | RAM maximizado: desglose (en uso, en caché/standby, libre, comprometida) e historial de 1 h | Los componentes suman el total ± 1 % | M |
| GD-SYS-05 | Disco normal: una barra por unidad fija con espacio usado/libre | Aparecen todas las unidades fijas; las USB extraíbles se marcan aparte | M |
| GD-SYS-06 | Disco maximizado: lectura y escritura en vivo (MB/s) por unidad | Copiar un archivo grande refleja la actividad en < 5 s | M |
| GD-SYS-07 | Red normal: subida y bajada en vivo con sparkline del adaptador activo | Una descarga se refleja en < 2 s | M |
| GD-SYS-08 | Red maximizado: historial de 1 h, nombre del adaptador, tipo (Wi-Fi/Ethernet) e IP local | Al cambiar de Wi-Fi a Ethernet, el adaptador mostrado cambia en < 10 s | M |
| GD-SYS-09 | El historial de 1 h vive en memoria y no se escribe a disco | Reiniciar Glassdeck vacía el historial | M |

### 3.5 Procesos (`GD-PROC`) — Fase 1

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-PROC-01 | Normal: los 5 procesos que más consumen, alternando entre CPU y RAM con un selector | La lista se actualiza cada 3 s sin parpadeo | M |
| GD-PROC-02 | Maximizado: lista completa con búsqueda, orden por columna (nombre, PID, CPU, RAM, usuario) y casillas de selección múltiple | Se pueden seleccionar varios y cerrarlos en una sola acción | M |
| GD-PROC-03 | **Nivel 1, lista negra fija:** `System`, `Idle`, `Registry`, `smss`, `csrss`, `wininit`, `winlogon`, `services`, `lsass`, `svchost`, `dwm`, `fontdrvhost`, `Memory Compression`, `MsMpEng`, `audiodg`, `spoolsv`, `sihost`, `ctfmon`, `conhost`, `explorer`; cualquier proceso de SYSTEM, LOCAL SERVICE, NETWORK SERVICE o de otro usuario; más los agregados por Luis | Estos procesos se muestran con candado, sin casilla, y el backend rechaza cerrarlos aunque reciba la orden | M |
| GD-PROC-04 | **Nivel 2, confirmación reforzada:** Cursor, Glassdeck, `claude`, `Code`, terminales (`WindowsTerminal`, `cmd`, `pwsh`, `powershell`), `node` | La confirmación muestra los nombres y exige escribir `CERRAR` | M |
| GD-PROC-05 | **Nivel 3, normal:** el resto, con una sola confirmación que lista los seleccionados | Sin confirmar, no se cierra nada | M |
| GD-PROC-06 | Cierre en dos pasos: primero cierre amable (`WM_CLOSE` a sus ventanas), y si en 5 s sigue vivo, terminación forzada | Una app con documento sin guardar muestra su propio diálogo de guardar | M |
| GD-PROC-07 | Resultado por proceso: cerrado, rechazado (protegido), sin permiso o ya no existía | El resumen final muestra cada proceso y su resultado | M |
| GD-PROC-08 | Glassdeck nunca se eleva para cerrar procesos | Un proceso de administrador muestra «Sin permiso» y no dispara UAC | M |
| GD-PROC-09 | `explorer` no se cierra desde la lista; se ofrece el atajo «Reiniciar Explorer» (GD-SHORT) | La fila de `explorer` muestra un enlace a «Reiniciar Explorer» | M |

### 3.6 Limpieza de temporales (`GD-CLEAN`) — Fase 1

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-CLEAN-01 | Vista previa con espacio a liberar por origen: `%TEMP%` del usuario, `C:\Windows\Temp` (con escudo UAC) y papelera (opcional, desmarcada) | Los tamaños mostrados difieren < 5 % del resultado real | M |
| GD-CLEAN-02 | Solo se borran archivos con más de 24 h sin modificarse; los que están en uso se omiten sin error | Un archivo abierto por otra app no se borra y no interrumpe la limpieza | M |
| GD-CLEAN-03 | `C:\Windows\Temp` se limpia con un ayudante que se eleva con UAC solo para esa operación | Si Luis cancela el UAC, el resto de la limpieza continúa y el resumen lo indica | M |
| GD-CLEAN-04 | Nunca se siguen enlaces simbólicos ni uniones fuera de las carpetas permitidas | Una unión dentro de `%TEMP%` que apunta a otra carpeta no borra el destino | M |
| GD-CLEAN-05 | Resumen final: espacio liberado, archivos omitidos y errores | El resumen coincide con la diferencia de espacio libre ± 5 % | M |

### 3.7 Spending y uso de Cursor (`GD-CUR`) — Fase 2

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-CUR-01 | Login desde el dock en una ventana web de cursor.com; Glassdeck captura la cookie de sesión del dashboard al terminar el login | Tras iniciar sesión, la ventana se cierra sola y el dock marca Cursor en verde | M |
| GD-CUR-02 | La cookie se guarda en el Administrador de credenciales de Windows, nunca en archivos ni en logs | Buscar el valor de la cookie en `%AppData%\Glassdeck` y en los logs no devuelve resultados | M |
| GD-CUR-03 | Solo lectura: gasto del ciclo actual, presupuesto o límite, uso incluido en el plan y fecha de corte | No existe en el código ninguna llamada de escritura a cursor.com | M |
| GD-CUR-04 | Las direcciones exactas del dashboard se identifican en la fase 2 observando el dashboard con sesión iniciada, y se aíslan en un único adaptador | Un cambio en la web de Cursor solo requiere modificar ese adaptador | M |
| GD-CUR-05 | Etiqueta visible «no oficial» en el widget | La etiqueta es visible en vista normal y maximizada | M |
| GD-CUR-06 | Si la lectura falla, el widget muestra el motivo («Sesión expirada», «Formato cambió», «Sin conexión») y la última lectura válida con su hora, nunca números inventados | Con la cookie borrada, el widget muestra «Sesión expirada» y el dock pasa a ámbar | M |

### 3.8 Spending y uso de Claude (`GD-CLA`) — Fase 2

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-CLA-01 | **Claude Code (local, sin login):** lee los registros JSONL de `~/.claude/projects/` y suma tokens de entrada, salida y caché por día, modelo y proyecto | Los totales del día coinciden con una herramienta de referencia (p. ej. `ccusage`) ± 2 % | M |
| GD-CLA-02 | Costo equivalente en USD con una tabla de precios por modelo editable en Customize | Cambiar un precio recalcula el costo al instante | M |
| GD-CLA-03 | Solo se leen los campos de uso (tokens, modelo, fecha); el texto de los mensajes no se carga ni se guarda | Una revisión del código confirma que solo se extraen esos campos | M |
| GD-CLA-04 | **Claude.ai (no oficial):** login web a claude.ai; muestra el % usado del límite de 5 h y del semanal y su hora de reinicio | Con sesión válida, los % coinciden con los que muestra claude.ai | M |
| GD-CLA-05 | **Consola de la API (opcional):** clave de administrador de organización guardada en el Administrador de credenciales; muestra el costo real del mes | Sin clave configurada, la sección no aparece | D |
| GD-CLA-06 | Mismo manejo de errores que GD-CUR-06 | Con la sesión de claude.ai expirada, el widget lo indica y conserva los datos locales de Claude Code | M |

### 3.9 Avisos de umbral (`GD-ALERT`) — Fase 2

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-ALERT-01 | Notificación de Windows al cruzar cada umbral (Cursor 50/80/100 %; Claude 80 % de 5 h y semanal) | Cada umbral avisa una sola vez por ciclo de facturación o ventana de límite | M |
| GD-ALERT-02 | Clic en la notificación abre el widget de Spending maximizado | Verificado en las dos fuentes | M |
| GD-ALERT-03 | El historial de avisos enviados persiste para no repetirlos tras reiniciar | Reiniciar Glassdeck no repite un aviso ya enviado | M |

### 3.10 Correo y agenda (`GD-MAIL`, `GD-CAL`, `GD-ROT`) — Fase 2

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-MAIL-01 | Login de Google con OAuth 2.0 para apps de escritorio (navegador del sistema, redirección a `127.0.0.1`, PKCE), con varias cuentas | Se pueden conectar 2 o más cuentas y cada una aparece en el dock | M |
| GD-MAIL-02 | Permiso de Gmail mínimo: `gmail.metadata` (solo encabezados, sin cuerpo) | La pantalla de consentimiento de Google no menciona leer el contenido de los correos | M |
| GD-MAIL-03 | Lee correos no leídos de la bandeja de entrada (etiquetas `INBOX` + `UNREAD`), solo remitente, asunto y fecha | Ningún cuerpo de correo pasa por la app | M |
| GD-CAL-01 | Permiso de Calendar de solo lectura de eventos; lee los eventos de hoy y de las próximas 24 h de los calendarios seleccionados | Un evento creado en Google Calendar aparece en ≤ 10 min | M |
| GD-ROT-01 | Alerta rotativa: cada 10 min (configurable) muestra al azar un correo no leído o un evento próximo, priorizando eventos que empiezan en < 30 min | En 1 h se ven al menos 6 elementos distintos si existen | M |
| GD-ROT-02 | Botón de privacidad (ojo) y atajo `Win+Alt+P` que desenfocan el contenido de correo, agenda y spending | Con privacidad activa, ningún asunto es legible en una captura de pantalla | M |
| GD-ROT-03 | Clic en un correo o evento lo abre en el navegador (Gmail o Calendar) | El enlace abre el elemento en la cuenta correcta | M |
| GD-MAIL-04 | Maximizado: lista de no leídos por cuenta y eventos del día | Se distingue la cuenta de cada elemento | M |
| GD-MAIL-05 | Los encabezados se mantienen solo en memoria; no se escriben a disco ni a logs | Tras cerrar Glassdeck no queda ningún asunto en disco | M |
| GD-MAIL-06 | Si una cuenta pierde el permiso, el dock la marca en ámbar y la alerta sigue con las demás | Revocar el acceso en la cuenta de Google refleja el estado en ≤ 5 min | M |
| GD-MAIL-07 | Customize permite importar el archivo JSON del cliente OAuth de escritorio del proyecto propio de Luis (ver `GUIA_GOOGLE_OAUTH.md`); el ID y el secreto del cliente se guardan en la bóveda | Sin cliente importado, el botón «Conectar Google» lleva a la guía | M |

### 3.11 Clima (`GD-WX`) — Fase 2

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-WX-01 | Normal: temperatura actual, icono, máxima y mínima del día | Coincide con Open-Meteo para la ubicación configurada | M |
| GD-WX-02 | Maximizado: pronóstico de 24 h por hora y de 7 días | Muestra 24 y 7 elementos | M |
| GD-WX-03 | Ubicación por IP (aproximada a ciudad) o manual (GD-CUST-08) | Sin conexión, muestra el último dato con su hora | M |

### 3.12 Audio, micrófono y cámara (`GD-AUD`, `GD-MIC`, `GD-CAM`) — Fase 3

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-AUD-01 | Normal: volumen general, silencio y selector del dispositivo de salida predeterminado | Los cambios se reflejan en el mezclador de Windows al instante y viceversa | M |
| GD-AUD-02 | Maximizado: mezclador por aplicación con volumen, silencio y **dispositivo de salida por app** (p. ej. Spotify → bocinas del equipo, Edge → altavoces USB) | La asignación se conserva al reiniciar la app y coincide con «Volumen de aplicaciones y preferencias de dispositivos» de Windows | M |
| GD-AUD-03 | El ruteo por app usa una interfaz no documentada de Windows (riesgo aceptado, Q22). Si falla, el widget lo indica y ofrece abrir la configuración de Windows | Simular el fallo muestra el aviso y el botón | M |
| GD-MIC-01 | Micrófono: dispositivo predeterminado, volumen de entrada, silencio y medidor de nivel en vivo | Hablar mueve el medidor; silenciar lo detiene | M |
| GD-CAM-01 | Cámara: vista previa bajo demanda (solo mientras el panel está abierto) y selector de cámara predeterminada | Al cerrar el panel, se apaga la luz de la cámara | M |
| GD-CAM-02 | Ajustes de brillo, contraste, exposición, enfoque y zoom, mostrando solo los que soporte la cámara | Un ajuste no soportado no aparece | M |
| GD-CAM-03 | Interruptor de privacidad que deshabilita la cámara a nivel de dispositivo, con UAC | Con la cámara deshabilitada, ninguna app la detecta; reactivarla vuelve a pedir UAC | M |

### 3.13 Brillo (`GD-BRI`) — Fase 3

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-BRI-01 | Un control por monitor: pantalla interna por WMI, externos por DDC/CI | Cada control cambia solo su monitor | M |
| GD-BRI-02 | Si un monitor no soporta DDC/CI, el control aparece deshabilitado con la leyenda «Este monitor no permite control por software» | Verificado con los 3 monitores de Luis | M |
| GD-BRI-03 | Los cambios por DDC/CI se agrupan (máx. 1 escritura cada 200 ms por monitor) | Arrastrar el control no congela el monitor | M |

### 3.14 Atajos del sistema (`GD-SHORT`) — Fase 1 (limpieza y procesos) y Fase 3 (resto)

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-SHORT-01 | Botones: limpiar temporales (abre GD-CLEAN), cerrar procesos pesados (abre GD-PROC maximizado) | Cada botón abre su flujo con vista previa | M |
| GD-SHORT-02 | Plan de energía: Alto rendimiento, Equilibrado, Ahorro, marcando el activo | El plan cambia y Windows lo refleja | M |
| GD-SHORT-03 | Reiniciar Explorer, con confirmación | La barra de tareas vuelve en < 10 s | M |
| GD-SHORT-04 | Limpiar caché de DNS | Muestra confirmación de éxito o el error | M |
| GD-SHORT-05 | Vaciar papelera, con tamaño previo y confirmación | La papelera queda vacía | M |
| GD-SHORT-06 | Bloquear la PC, suspender (con confirmación) y abrir el Administrador de tareas | Cada acción funciona | M |

### 3.15 Atajos de teclado globales (`GD-HOT`) — Fase 1 (Zonas en fase 4)

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-HOT-01 | `Win+Alt+G`: mostrar u ocultar todos los widgets | Funciona desde cualquier app | M |
| GD-HOT-02 | `Win+Alt+P`: modo privacidad (GD-ROT-02) | Funciona desde cualquier app | M |
| GD-HOT-03 | `Win+Alt+Z`: abrir el editor de zonas | Funciona con el módulo Zonas activo | M (fase 4) |
| GD-HOT-04 | Detección de conflictos al registrar (GD-CUST-06) | Ver GD-CUST-06 | M |

### 3.16 Zonas tipo MaxTo (`GD-ZONE`) — Fase 4

Basado en `referencias/MAXTO_ANALISIS.md`. Se aplica a las ventanas de **todas** las apps; los widgets de Glassdeck se excluyen.

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-ZONE-01 | Cada monitor tiene su cuadrícula (filas × columnas, 1–12) y, opcionalmente, zonas libres dibujadas a mano | Se guardan cuadrículas distintas para los 3 monitores | M |
| GD-ZONE-02 | La configuración se guarda por **huella del monitor** (número de serie, nombre, resolución, DPI), no por su posición | Cambiar el orden de los monitores en Windows conserva la cuadrícula de cada uno | M |
| GD-ZONE-03 | Editor de zonas (`Win+Alt+Z` o dock): vista de los 3 monitores a escala, edición de cuadrícula, presets y reglas por app | Los cambios se aplican sin reiniciar | M |
| GD-ZONE-04 | Superposición de colocación: un atajo (por defecto `Win+Alt+Espacio`) abre la cuadrícula sobre el monitor de la ventana activa; arrastrar sobre las celdas elige el rectángulo, con resaltado en vivo, y al soltar la ventana se coloca ahí | La ventana queda alineada a las celdas con error ≤ 1 px | M |
| GD-ZONE-05 | Modificadores en la superposición: `Shift` = recordar esa zona para la app, `Ctrl` = guardar como preset, `PageUp`/`PageDown` = cambiar de monitor, `Esc` = cancelar. Una insignia muestra el modo activo | Cada modificador produce su efecto y la insignia lo muestra | M |
| GD-ZONE-06 | Ajuste al arrastrar: con `Shift` presionado mientras se arrastra la barra de título de cualquier ventana, se iluminan las zonas del monitor bajo el cursor y al soltar la ventana encaja en la zona resaltada | Funciona en los 3 monitores con escalados distintos | M |
| GD-ZONE-07 | Presets: rectángulos guardados por monitor, cada uno con atajo propio y vista previa | Pulsar el atajo coloca la ventana activa en el preset | M |
| GD-ZONE-08 | Mover la ventana activa al monitor siguiente/anterior conservando su zona relativa (atajos configurables) | La ventana pasa al otro monitor en la zona equivalente | M |
| GD-ZONE-09 | Memoria por app: las ventanas nuevas de una app recordada (identificada por ruta del ejecutable y clase de ventana) se colocan solas en su zona | Abrir Spotify lo coloca en su zona en < 1 s | M |
| GD-ZONE-10 | «Ajustar a cuadrícula»: encaja la ventana activa en las celdas más cercanas a su posición | Atajo configurable | M |
| GD-ZONE-11 | «Recuperar todo»: recoloca todas las ventanas según sus reglas (manual y automático al reconectar monitores) | Tras reconectar el monitor 3, las ventanas recordadas vuelven a él en < 5 s | M |
| GD-ZONE-12 | Se compensan los bordes invisibles de Windows 10 para que las ventanas encajen exacto | No quedan huecos de ~7 px entre ventanas contiguas | M |
| GD-ZONE-13 | Ventanas no elegibles (elevadas, herramientas, sin título, invisibles, de pantalla completa): se omiten; si Luis intenta mover una elevada, se le avisa que Windows no lo permite | El aviso aparece y no hay error silencioso | M |
| GD-ZONE-14 | No se usan ganchos globales de teclado o mouse de bajo nivel; solo eventos de ventanas y atajos registrados | Revisión de código | M |
| GD-ZONE-15 | Menú de bandeja «Mover a» con presets por monitor y vista previa al pasar el mouse | La vista previa se muestra antes de confirmar | D |

### 3.17 Instalación y ciclo de vida (`GD-INST`) — Fase 1

| ID | Requerimiento | CA | Prio |
|---|---|---|---|
| GD-INST-01 | Instalador `.exe` (NSIS) por usuario, sin firma digital, sin permisos de administrador | Instala en `%LocalAppData%\Programs\Glassdeck` sin UAC | M |
| GD-INST-02 | Arranque con Windows (GD-CUST-07) | Tras iniciar sesión, los widgets aparecen en < 10 s | M |
| GD-INST-03 | Desinstalación limpia, con opción de conservar o borrar la configuración y las credenciales | Al elegir borrar, no quedan entradas de Glassdeck en el Administrador de credenciales | M |
| GD-INST-04 | Sin actualización automática ni telemetría | Fuera de las fuentes de datos (sección 5.3), Glassdeck no hace conexiones de red | M |
| GD-INST-05 | Requiere WebView2 (incluido en Windows 10 actualizado); si falta, el instalador lo descarga | Instalación probada en el equipo de Luis | M |

## 4. Requerimientos no funcionales

### 4.1 Rendimiento (`GD-NFR-PERF`)

| ID | Requerimiento | CA |
|---|---|---|
| GD-NFR-PERF-01 | CPU < 2 % en reposo con todos los widgets visibles | Promedio de 10 min medido con el Monitor de rendimiento, sin interactuar |
| GD-NFR-PERF-02 | RAM < 200 MB en total (proceso principal + WebView2) | Suma del conjunto de trabajo privado de todos los procesos de Glassdeck. Ver riesgo R-01 |
| GD-NFR-PERF-03 | Con batería, las animaciones se pausan y los intervalos de 1 s pasan a 3 s | Desconectar el cargador cambia el comportamiento en < 5 s |
| GD-NFR-PERF-04 | Los widgets ocultos, cerrados o tapados por una app a pantalla completa no recolectan ni dibujan | Con un juego o presentación a pantalla completa, CPU < 0.5 % |
| GD-NFR-PERF-05 | Intervalos por defecto: CPU, RAM y red 1 s; procesos 3 s; disco 5 s; correo 5 min; calendario 10 min; spending 15 min; clima 30 min | Configuración inicial |
| GD-NFR-PERF-06 | Arranque en frío < 3 s hasta ver los widgets | Medido desde el inicio del proceso |

### 4.2 Seguridad y privacidad (`GD-NFR-SEC`)

| ID | Requerimiento | CA |
|---|---|---|
| GD-NFR-SEC-01 | Tokens, cookies y claves solo en el Administrador de credenciales de Windows | Revisión de código + búsqueda en disco (GD-CUR-02) |
| GD-NFR-SEC-02 | Los logs nunca contienen tokens, cookies, asuntos de correo, títulos de eventos ni rutas personales | Revisión de un log de 1 día de uso |
| GD-NFR-SEC-03 | El frontend no tiene acceso directo a red ni a disco: todo pasa por comandos del backend con permisos de Tauri restringidos (CSP estricta) | Revisión de la configuración de capacidades |
| GD-NFR-SEC-04 | Las ventanas de login de terceros están aisladas y no pueden invocar comandos de Glassdeck | Revisión de capacidades por ventana |
| GD-NFR-SEC-05 | Glassdeck corre sin privilegios; las operaciones de administrador (limpiar `C:\Windows\Temp`, deshabilitar cámara) usan un ayudante elevado de un solo uso con parámetros validados | No hay procesos de Glassdeck elevados en reposo |
| GD-NFR-SEC-06 | Ningún dato sale del equipo salvo hacia las fuentes listadas en 5.3 | Revisión de conexiones con el Monitor de recursos |
| GD-NFR-SEC-07 | El correo puede contener datos de pacientes: solo encabezados, solo en memoria, con modo privacidad | GD-MAIL-02, -05 y GD-ROT-02 |

### 4.3 Confiabilidad (`GD-NFR-REL`)

| ID | Requerimiento | CA |
|---|---|---|
| GD-NFR-REL-01 | La falla de una fuente (Cursor, Claude, Google, clima) no afecta a los demás widgets | Desconectar la red deja funcionando los widgets de sistema |
| GD-NFR-REL-02 | Reintentos con espera creciente (1, 2, 5, 15 min) ante errores de red | Sin ráfagas de peticiones tras volver la conexión |
| GD-NFR-REL-03 | Escrituras de configuración atómicas (archivo temporal + renombrar) con respaldo de la versión anterior | Apagar el equipo durante un guardado no corrompe la configuración |
| GD-NFR-REL-04 | Log rotativo local (máx. 5 archivos de 2 MB) en `%LocalAppData%\Glassdeck\logs` | Verificado tras 1 semana |

### 4.4 Accesibilidad y diseño (`GD-NFR-UX`)

| ID | Requerimiento | CA |
|---|---|---|
| GD-NFR-UX-01 | Contraste mínimo 4.5:1 para texto sobre vidrio, en tema oscuro y claro | Verificado con fondos de escritorio claros y oscuros |
| GD-NFR-UX-02 | Todo es operable con teclado, con foco visible | Recorrido completo con Tab |
| GD-NFR-UX-03 | Se respeta «Mostrar animaciones en Windows» y `prefers-reduced-motion` | Con animaciones desactivadas en Windows, no hay transiciones |
| GD-NFR-UX-04 | Números con ancho fijo (`tabular-nums`) para que no salten | Revisión visual |
| GD-NFR-UX-05 | El diseño sigue el mockup aprobado (`mockups/glassdeck_mockup.html`) | Comparación visual al cierre de cada fase |

## 5. Datos, almacenamiento y fuentes externas

### 5.1 Almacenamiento local

| Qué | Dónde | Formato |
|---|---|---|
| Configuración (widgets, posiciones, tema, intervalos, umbrales, lista negra, atajos) | `%AppData%\Glassdeck\settings.json` | JSON versionado, escritura atómica |
| Zonas (cuadrículas por huella, presets, reglas por app) | `%AppData%\Glassdeck\zones.json` | JSON versionado |
| Estado (avisos enviados, último dato válido por fuente sin datos personales) | `%AppData%\Glassdeck\state.json` | JSON |
| Credenciales | Administrador de credenciales de Windows, prefijo `Glassdeck/` | Cifrado por Windows (DPAPI) |
| Logs | `%LocalAppData%\Glassdeck\logs\` | Texto, rotativo, sin datos sensibles |

### 5.2 Credenciales

| Servicio | Qué se guarda | Cómo se obtiene |
|---|---|---|
| Google (por cuenta) | Token de actualización OAuth | OAuth de escritorio con PKCE |
| Cursor | Cookie de sesión del dashboard | Ventana de login a cursor.com |
| Claude.ai | Cookie de sesión | Ventana de login a claude.ai |
| Consola de Anthropic (opcional) | Clave de administrador | Captura manual en Customize |

### 5.3 Fuentes externas permitidas

| Fuente | Uso | Oficial |
|---|---|---|
| `oauth2.googleapis.com`, `accounts.google.com` | Login de Google | Sí |
| `gmail.googleapis.com` | Encabezados de no leídos | Sí |
| `www.googleapis.com/calendar/v3` | Eventos | Sí |
| `cursor.com` | Dashboard de gasto | **No** |
| `claude.ai` | Límites de uso de la suscripción | **No** |
| `api.anthropic.com` (opcional) | Informe de costos de la organización | Sí |
| `api.open-meteo.com`, `geocoding-api.open-meteo.com` | Clima y búsqueda de ciudad | Sí |
| `ipapi.co` (geolocalización por IP; se consulta solo con ubicación automática) | Ciudad aproximada | Sí (de terceros) |

## 6. Fases y criterios de salida

| Fase | Contenido | Criterio de salida |
|---|---|---|
| 0 — Preparación | Instalar Rust y las herramientas de compilación de C++ de Visual Studio (~6 GB); crear el repo `tools/glassdeck`; **prueba de concepto de RAM** con 11 ventanas WebView2 en 3 variantes (riesgo R-01), medida con `poc/medir.ps1` | La prueba mide la RAM real y `adr/ADR-001-estrategia-ventanas.md` registra la estrategia elegida. Si ninguna variante baja de 200 MB, decide Luis antes de la fase 1 |
| 1 — Base | GD-WIN, GD-DOCK (sin cuentas), GD-CUST, GD-SYS, GD-PROC, GD-CLEAN, GD-SHORT-01, GD-HOT-01/02/04, GD-INST, GD-NFR | Instalador funcional; todos los CA de la fase cumplidos; VoBo de Luis |
| 2 — Servicios | GD-DOCK-02, GD-CUR, GD-CLA, GD-ALERT, GD-MAIL, GD-CAL, GD-ROT, GD-WX | Igual |
| 3 — Controles | GD-AUD, GD-MIC, GD-CAM, GD-BRI, GD-SHORT-02 a 06 | Igual |
| 4 — Zonas | GD-ZONE, GD-HOT-03; observación en vivo de la prueba de MaxTo al iniciar la fase | Igual |

## 7. Riesgos y supuestos

| ID | Riesgo | Impacto | Mitigación |
|---|---|---|---|
| R-01 | Cada widget como ventana WebView2 independiente puede superar los 200 MB (WebView2 crea procesos de render por ventana) | Incumplir GD-NFR-PERF-02 | Prueba de concepto en la fase 0. Alternativas en orden: (1) compartir el entorno de WebView2 entre ventanas; (2) destruir la ventana de los widgets cerrados; (3) agrupar widgets en una ventana transparente por monitor con zonas de clic. Si ninguna basta, Luis decide entre subir el presupuesto o agrupar |
| R-02 | Cursor y claude.ai cambian su web interna | El widget deja de leer gasto | Adaptador aislado (GD-CUR-04), aviso claro (GD-CUR-06) |
| R-03 | La interfaz no documentada del ruteo por app cambia con una actualización de Windows | El mezclador por app deja de asignar salida | GD-AUD-03; el resto del audio sigue funcionando |
| R-04 | El *acrylic* de Windows 10 se traba al arrastrar | Experiencia de arrastre pobre | GD-WIN-03 |
| R-05 | Mantener los widgets en la capa del escritorio frente a `Win+D` es frágil en Windows 10 | Los widgets desaparecen con `Win+D` | Técnica tipo Rainmeter (fondo + detección de «Mostrar escritorio»); probar en fase 1 |
| R-06 | Monitores externos sin DDC/CI | Sin control de brillo en ese monitor | GD-BRI-02 |
| R-07 | Google limita apps sin verificar a 100 usuarios y muestra advertencia | Ninguno para uso personal | Guía paso a paso |
| R-08 | Ventanas elevadas no se pueden mover (UIPI) | Zonas no aplica a esas apps | GD-ZONE-13 |
| R-09 | El instalador sin firma muestra la advertencia de SmartScreen | Un clic extra al instalar | Aceptado (D-14) |

Supuestos: Windows 10 Pro 22H2 con WebView2 actualizado; Luis tiene plan individual de Cursor y suscripción de Claude; Claude Code guarda registros en `~/.claude/projects/`.

## 8. Glosario

- **Acrylic:** efecto de Windows 10 que desenfoca lo que está detrás de una ventana.
- **DDC/CI:** protocolo para controlar ajustes de un monitor externo desde la PC.
- **Huella del monitor:** combinación de serie, nombre, resolución y DPI que identifica un monitor aunque cambie de posición.
- **Píldora:** forma minimizada de un widget.
- **UIPI:** protección de Windows que impide a un proceso normal manipular ventanas de procesos elevados.
- **UAC:** confirmación de administrador de Windows.

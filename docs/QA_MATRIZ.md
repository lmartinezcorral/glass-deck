# Glassdeck — Matriz de QA

| Campo | Valor |
|---|---|
| Documento | Matriz de QA de Glassdeck (`QA_MATRIZ.md`) |
| Versión | 1.0 |
| Fecha | 2026-10-27 |
| Fuente | `SPEC.md` v1.1 (requerimientos `GD-*`, sección 6 para fases) y `ARQUITECTURA.md` |
| Estado | Borrador — pendiente de ejecución |

## Cómo usar

Cada fila verifica un requerimiento de la SPEC con su criterio de aceptación (**CA**). Se ejecuta al cerrar la fase; una fase solo sale cuando todas sus filas **M** están en **Pasa** (las **D** pueden quedar en **Diferido**).

**Métodos**

| Método | Qué es |
|---|---|
| **Auto-Rust** | Prueba unitaria o de integración con `cargo test` en `src-tauri/` o `helper/` |
| **Auto-Vitest** | Prueba unitaria del frontend (React + TS) con Vitest |
| **Script** | Script de PowerShell que mide y deja evidencia (p. ej. `poc/medir.ps1` para RAM/CPU; se crea en la fase 0) |
| **Manual** | Revisión paso a paso por Luis o el agente en el ThinkPad con 3 monitores, Windows 10 Pro 22H2 |
| **Revisión** | Revisión de código o de configuración (`tauri.conf.json`, `capabilities/`, CSS) |

**Estados:** Pendiente / En prueba / Pasa / Falla / Diferido.

**Notas**

- Monitores: «principal», «monitor 2» y «monitor 3», según la numeración de Windows.
- «Procesos de Glassdeck» = `glassdeck.exe` más todos sus `msedgewebview2.exe` hijos (filtrar por árbol de procesos, no por nombre).
- **M\***: los requerimientos no funcionales (`GD-NFR-*`) no traen prioridad en la SPEC; se tratan como M porque el criterio de salida de la fase 1 exige todos los CA.
- Evidencia: guardar capturas, grabaciones y salidas de script en `qa/evidencia/<fase>/<ID>.*` y anotar la ruta en la fila.

## Fase 0 — Preparación

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| R-01 (GD-NFR-PERF-02) | Prueba de concepto de RAM con 11 ventanas WebView2 | Se mide la RAM real de 3 variantes contra el presupuesto < 200 MB de conjunto de trabajo privado y se decide la estrategia de ventanas | M | Script | 1. Línea base: 11 ventanas con entorno WebView2 propio.<br>2. Variante 1: 11 ventanas con entorno compartido.<br>3. Variante 2: entorno compartido + destruir ventanas cerradas (medir con 11 abiertas y con 5 cerradas).<br>4. Variante 3: una ventana transparente por monitor (3 ventanas).<br>5. Por variante: `poc/medir.ps1` suma `Working Set - Private` del árbol de procesos cada 5 s durante 5 min.<br>Evidencia: tabla MB promedio y máx. por variante + decisión de Luis | **Pasa** (ADR-001 aceptado 2026-10-27; medido con 1 monitor) |

## Fase 1 — Base

### GD-WIN — Ventanas y comportamiento de widgets

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-WIN-01 | Ventana sin marco, transparente, sin botón en barra de tareas | No sale en Alt+Tab ni en la barra; esquinas redondeadas sin borde de Windows | M | Revisión + Manual | 1. Revisar `tauri.conf.json`: `decorations:false`, `transparent:true`, `skipTaskbar:true`.<br>2. Abrir todos los widgets, pulsar Alt+Tab y revisar la barra: ningún widget.<br>3. Captura con lupa de Windows al 400 % en una esquina | En prueba (`poc/capturar-widgets.ps1` 2026-10-27: 6 ventanas fuera de Alt+Tab, sin barra de título y sin marco nativo en la captura, también después de activar y retitular cada ventana; Auto-Rust cubre el filtro de estilos; falta la lupa al 400 %) |
| GD-WIN-02 | Vidrio *acrylic* + brillo y reflejo dibujados | Escritorio desenfocado detrás; brillo y borde de luz visibles en oscuro y claro | M | Manual | 1. Poner fondo de escritorio con mucho contraste.<br>2. Confirmar desenfoque detrás del widget.<br>3. Cambiar Windows a modo claro y repetir.<br>Evidencia: 2 capturas | Pendiente |
| GD-WIN-03 | Desenfoque simple al arrastrar, *acrylic* al soltar | Sin saltos > 100 ms por cuadro al cruzar un monitor | M | Manual | 1. Grabar a 60 fps con `Win+Alt+R`.<br>2. Arrastrar un widget de borde a borde 5 veces por monitor.<br>3. Revisar cuadro a cuadro: ningún congelamiento > 6 cuadros; el *acrylic* vuelve al soltar | Pendiente |
| GD-WIN-04 | Arrastre libre desde la barra superior, entre monitores | El widget llega a los 3 monitores y queda donde se suelta | M | Manual | 1. Arrastrar un widget al monitor 2, luego al 3.<br>2. Soltar y esperar 10 s: no se mueve.<br>3. Regresar al principal | Pendiente |
| GD-WIN-05 | Barra con icono, título, pin, minimizar, maximizar, cerrar (al pasar el mouse) | Los 4 botones funcionan con mouse y teclado y tienen `aria-label` | M | Auto-Vitest + Manual | 1. Vitest: la barra renderiza 4 botones con `aria-label` no vacío.<br>2. Manual: pasar el mouse muestra los botones.<br>3. Tab hasta cada botón y Enter lo activa | En prueba (Vitest pasa; falta hover y teclado) |
| GD-WIN-06 | Minimizar a píldora (icono + dato clave) en la misma posición | Clic en la píldora restaura el tamaño anterior | M | Auto-Vitest + Manual | 1. Vitest: la píldora muestra icono y dato (p. ej. «CPU 23 %»).<br>2. Manual: redimensionar, minimizar, anotar posición.<br>3. Clic en píldora: mismo tamaño y posición | En prueba (Vitest pasa; falta la prueba manual de posición) |
| GD-WIN-07 | Maximizar abre vista detallada centrada en su monitor | `Esc` o restaurar vuelve a la vista normal en su posición | M | Manual | 1. Widget en el monitor 3; maximizar: queda centrado en el monitor 3.<br>2. `Esc`: vuelve a su posición original.<br>3. Repetir con el botón restaurar | Pendiente |
| GD-WIN-08 | Cerrar oculta el widget y lo marca apagado | Se reactiva desde dock o Customize en su última posición | M | Manual | 1. Cerrar un widget: interruptor apagado en dock y Customize.<br>2. Reactivar desde el dock: misma posición.<br>3. Repetir reactivando desde Customize | Pendiente |
| GD-WIN-09 | Capa de escritorio por defecto, visible con `Win+D` | Con app maximizada no se ve; con `Win+D` sigue visible | M | Manual | 1. Maximizar Edge sobre el widget: el widget no se ve.<br>2. `Win+D`: los widgets siguen visibles.<br>3. Repetir 5 veces en los 3 monitores (riesgo R-05) | Pendiente |
| GD-WIN-10 | «Siempre encima» por widget (pin) | Queda sobre ventanas normales sin robar el foco | M | Manual | 1. Activar pin.<br>2. Bloc de notas maximizado y escribir: el widget queda encima.<br>3. El texto sigue entrando en Bloc de notas | Pendiente |
| GD-WIN-11 | Recordar posición, tamaño, estado y pin por widget y monitor | Tras reiniciar Windows, todo aparece donde estaba | M | Auto-Rust + Manual | 1. Auto-Rust: guardar y leer configuración conserva posición, tamaño, estado, pin y monitor.<br>2. Manual: acomodar en 3 monitores (uno en píldora, uno oculto, uno con pin) y capturar.<br>3. Reiniciar Windows y comparar con la captura | En prueba (Auto-Rust pasa; falta el reinicio con 3 monitores) |
| GD-WIN-12 | Monitor desconectado: widgets al principal sin superponerse; regresan al reconectar | Ida y vuelta en < 3 s | M | Auto-Rust + Manual | 1. Auto-Rust: reubicar con monitor ausente no superpone rectángulos.<br>2. Manual: desconectar el cable del monitor 3 y cronometrar.<br>3. Reconectar y cronometrar el regreso | En prueba (Auto-Rust pasa: la rejilla por defecto no superpone 11 widgets a 100 % y 150 %; falta desconectar el monitor) |
| GD-WIN-13 | Ajuste magnético opcional (8 px) a bordes y entre widgets | Soltar a < 12 px de otro lo alinea | D | Auto-Rust + Manual | 1. Auto-Rust: el cálculo de imán alinea a 11 px y no a 13 px.<br>2. Manual: activar la opción, soltar un widget cerca de otro y del borde | En prueba (Auto-Rust pasa: alinea a 11 px y no a 13 px; falta la prueba manual) |
| GD-WIN-14 | Escalado correcto con DPI distinto por monitor | Conserva tamaño visual y texto nítido al cambiar de monitor | M | Manual | 1. Escalado 100 %, 125 % y 150 % en los 3 monitores.<br>2. Mover un widget por los 3 y capturar en cada uno.<br>3. Comparar ancho físico y nitidez del texto | Pendiente |
| GD-WIN-15 | No intercepta clics fuera del área visible | Clic junto al widget selecciona iconos del escritorio | M | Manual | 1. Poner un icono a 5 px de la esquina redondeada.<br>2. Clic en el icono: se selecciona.<br>3. Arrastrar selección rectangular junto al widget | Pendiente |

### GD-DOCK — Dock y bandeja (GD-DOCK-02 pasa a fase 2)

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-DOCK-01 | Dock de vidrio arrastrable: interruptores, Customize, Zonas (cuentas en fase 2) | Cada acción funciona y su estado se ve en tiempo real | M | Manual | 1. Arrastrar el dock entre monitores.<br>2. Apagar y encender cada widget: aparece o desaparece en < 1 s.<br>3. Abrir Customize; cambiar algo ahí y ver el dock actualizado.<br>4. Botón Zonas: anotar comportamiento (módulo en fase 4) | Pendiente |
| GD-DOCK-03 | Ocultar el dock y fijarlo a un borde de cualquier monitor | Con dock oculto, el icono de bandeja sigue disponible | M | Manual | 1. Fijar el dock a un borde de cada monitor.<br>2. Ocultarlo.<br>3. Confirmar el icono en la bandeja | Pendiente |
| GD-DOCK-04 | Menú de bandeja: Mostrar/ocultar todo, Customize, Zonas, Privacidad, Pausar, Salir | Todo funciona con el dock oculto | M | Manual | 1. Ocultar el dock.<br>2. Probar cada opción del menú y anotar resultado por opción.<br>3. «Salir» al final: no quedan procesos de Glassdeck | Pendiente |
| GD-DOCK-05 | Una sola instancia | Abrir de nuevo enfoca la existente | M | Manual + Script | 1. Lanzar Glassdeck 3 veces seguidas.<br>2. `Get-Process glassdeck` devuelve 1 proceso.<br>3. El dock queda al frente | En prueba (`poc/probar-instancia-unica.ps1`: 0 fallas en 11 intentos tras corregir un cuelgue al llegar la 2.ª instancia durante el arranque; falta ver el dock al frente) |

### GD-CUST — Customize (CUST-04 y CUST-08 pasan a fase 2)

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-CUST-01 | Activar o desactivar cada widget y Zonas | Desactivar detiene la recolección, no solo oculta | M | Auto-Rust + Manual | 1. Auto-Rust: con el widget desactivado, el planificador no llama a su recolector (contador = 0).<br>2. Manual: desactivar Procesos y revisar el log de depuración: sin lecturas de procesos | Pendiente |
| GD-CUST-02 | Tema: oscuro, seguir Windows, acento, desenfoque, opacidad | Se aplica en vivo a todos los widgets | M | Manual | 1. Cambiar acento, desenfoque y opacidad.<br>2. Activar «seguir Windows» y cambiar el modo de Windows.<br>3. Todos los widgets cambian sin reiniciar | Pendiente |
| GD-CUST-03 | Intervalos editables con mínimos seguros | Rechaza valores bajo el mínimo (CPU/RAM/red 1 s, procesos 2 s, correo 2 min, spending 5 min) | M | Auto-Rust + Auto-Vitest | 1. Auto-Rust: validación rechaza 0.5 s, 1 s procesos, 1 min correo, 4 min spending y acepta los mínimos.<br>2. Vitest: el formulario muestra error y no guarda | En prueba (Auto-Rust pasa: rechaza 0.5 s, procesos 1 s y disco 0.999 s, acepta los mínimos, y `reemplazar` valida antes de guardar; Vitest pasa: el formulario usa selectores que solo ofrecen valores en o sobre el mínimo; correo y spending llegan en fase 2) |
| GD-CUST-05 | Lista negra: fijos con candado; agregar y quitar personalizados | Fijos sin botón de quitar; agregados respetados en GD-PROC | M | Auto-Rust + Manual | 1. Auto-Rust: quitar un fijo da error; un agregado se clasifica en nivel 1.<br>2. Manual: agregar `notepad`; en Procesos su fila sale con candado.<br>3. Quitarlo: vuelve a tener casilla | Pendiente |
| GD-CUST-06 | Atajos editables con detección de conflictos | Si `RegisterHotKey` falla: «Este atajo lo usa otro programa» y no se guarda | M | Auto-Rust + Manual | 1. Auto-Rust: con registro simulado que falla, no se escribe la configuración.<br>2. Manual: ocupar un atajo con otra app (p. ej. script de AutoHotkey), asignarlo en Customize.<br>3. Ver el mensaje y confirmar que `settings.json` no cambió | Pendiente |
| GD-CUST-07 | Arranque con Windows, capa por defecto, pausa con batería | Las opciones persisten y se aplican al reiniciar | M | Auto-Rust + Manual | 1. Auto-Rust: las 3 opciones sobreviven guardar y leer; valores por defecto correctos.<br>2. Manual: cambiar las 3, reiniciar Windows y verificar.<br>3. Revisar la entrada de arranque en `HKCU\...\Run` | En prueba (Auto-Rust pasa; un binario de `target\` nunca escribe en Run; falta el reinicio con el instalador) |
| GD-CUST-09 | Exportar e importar configuración a JSON sin credenciales | El archivo exportado no tiene tokens ni cookies | D | Auto-Rust + Manual | 1. Auto-Rust: con un secreto falso en la bóveda, el JSON exportado no lo contiene.<br>2. Manual: exportar, `Select-String -Pattern 'token\|cookie\|secret'` sin resultados.<br>3. Importar en instalación limpia: misma configuración | Pendiente |
| GD-CUST-10 | Restablecer posiciones | Todos vuelven a la disposición por defecto en el principal | M | Auto-Rust + Manual | 1. Auto-Rust: la disposición por defecto cabe en el área de trabajo del principal sin superposición.<br>2. Manual: repartir widgets en 3 monitores, restablecer y capturar | Pendiente |

### GD-SYS — CPU, RAM, disco y red

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-SYS-01 | Medidor circular de CPU total | Diferencia < 5 puntos vs Administrador de tareas en 1 min | M | Manual | 1. Widget y Administrador de tareas lado a lado; grabar 60 s.<br>2. Anotar 12 pares de valores (cada 5 s).<br>3. Anotar la diferencia máx. | Pendiente |
| GD-SYS-02 | CPU maximizado: gráfica por núcleo lógico + historial 1 h | Todos los núcleos lógicos y 3 600 puntos | M | Auto-Rust + Manual | 1. Auto-Rust: el búfer circular guarda 3 600 puntos y descarta el más viejo.<br>2. Manual: comparar número de núcleos con «Procesadores lógicos» del Administrador de tareas.<br>3. Tras 1 h, la gráfica está completa | En prueba (Auto-Rust pasa: el búfer guarda 3 600 puntos y descarta el más viejo; la serie entrega la última hora real según el intervalo: 1 800 puntos a 2 s; falta la comparación manual de núcleos) |
| GD-SYS-03 | RAM: medidor y GB usados de GB totales | Coincide con el Administrador de tareas ± 0.3 GB | M | Manual | 1. Anotar 5 lecturas del widget y del Administrador de tareas con 10 s de separación.<br>2. Abrir una app pesada y repetir.<br>3. Anotar la diferencia máx. | Pendiente |
| GD-SYS-04 | RAM maximizado: desglose + historial 1 h | Los componentes suman el total ± 1 % | M | Auto-Rust + Manual | 1. Auto-Rust (integración): en el equipo real, en uso + caché + libre = total ± 1 %.<br>2. Manual: comparar con Monitor de recursos, pestaña Memoria | En prueba (Auto-Rust de integración pasa en este equipo: en uso + caché + libre = total ± 1 %; falta comparar con el Monitor de recursos) |
| GD-SYS-05 | Disco: barra por unidad fija | Salen todas las fijas; las USB extraíbles se marcan aparte | M | Manual | 1. Comparar unidades con «Este equipo».<br>2. Conectar una USB: aparece marcada aparte.<br>3. Retirarla: desaparece | Pendiente |
| GD-SYS-06 | Disco maximizado: lectura y escritura MB/s por unidad | Copiar un archivo grande se refleja en < 5 s | M | Manual | 1. Copiar un archivo de ≥ 4 GB entre carpetas.<br>2. Cronometrar hasta que el widget muestre actividad.<br>3. Comparar MB/s con el Administrador de tareas | Pendiente |
| GD-SYS-07 | Red: subida y bajada en vivo con sparkline | Una descarga se refleja en < 2 s | M | Manual | 1. Iniciar una descarga grande.<br>2. Cronometrar hasta que suba la gráfica.<br>3. Detenerla y ver la caída | Pendiente |
| GD-SYS-08 | Red maximizado: historial, adaptador, tipo e IP local | Al cambiar Wi-Fi → Ethernet el adaptador cambia en < 10 s | M | Manual | 1. Con Wi-Fi, anotar adaptador e IP (comparar con `ipconfig`).<br>2. Conectar Ethernet y apagar Wi-Fi.<br>3. Cronometrar el cambio | Pendiente |
| GD-SYS-09 | Historial de 1 h solo en memoria | Reiniciar Glassdeck vacía el historial | M | Revisión + Manual | 1. Revisión: el búfer no se serializa a disco.<br>2. Manual: usar 10 min, reiniciar Glassdeck: historial vacío.<br>3. `%AppData%\Glassdeck` sin archivos nuevos de historial | En prueba (revisión pasa: `Historiales` no se serializa ni se escribe a disco; falta reiniciar tras 10 min) |

### GD-PROC — Procesos

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-PROC-01 | Top 5 procesos por CPU o RAM con selector | Se actualiza cada 3 s sin parpadeo | M | Auto-Vitest + Manual | 1. Vitest: las filas usan clave estable y no se vuelven a montar al actualizar.<br>2. Manual: grabar 30 s; contar ~10 actualizaciones, sin parpadeo.<br>3. Cambiar el selector CPU/RAM | Pendiente |
| GD-PROC-02 | Lista completa: búsqueda, orden por columna, selección múltiple | Se seleccionan varios y se cierran en una acción | M | Auto-Vitest + Manual | 1. Vitest: búsqueda y orden por nombre, PID, CPU, RAM, usuario.<br>2. Manual: abrir 3 Bloc de notas, buscar «notepad», marcar los 3, cerrar y confirmar | Pendiente |
| GD-PROC-03 | Nivel 1: lista negra fija, SYSTEM/servicios/otro usuario y agregados | Candado, sin casilla; el backend rechaza cerrarlos aunque reciba la orden | M | Auto-Rust + Manual | 1. Auto-Rust: cada nombre fijo y un proceso de SYSTEM se clasifican nivel 1; el comando de cierre con PID de `lsass` devuelve «rechazado».<br>2. Manual: las filas de la lista fija muestran candado y no tienen casilla | Pendiente |
| GD-PROC-04 | Nivel 2: confirmación reforzada (Cursor, Glassdeck, terminales, `node`…) | Muestra nombres y exige escribir `CERRAR` | M | Auto-Rust + Auto-Vitest + Manual | 1. Auto-Rust: la lista de nivel 2 se clasifica bien.<br>2. Vitest: el botón sigue deshabilitado hasta escribir `CERRAR`.<br>3. Manual: abrir un `node` de prueba e intentar cerrarlo | Pendiente |
| GD-PROC-05 | Nivel 3: una confirmación con lista de seleccionados | Sin confirmar no se cierra nada | M | Auto-Vitest + Manual | 1. Vitest: el diálogo lista los seleccionados.<br>2. Manual: seleccionar Bloc de notas, cancelar el diálogo: sigue vivo | Pendiente |
| GD-PROC-06 | Cierre en dos pasos: `WM_CLOSE` y forzado a los 5 s | Una app con documento sin guardar muestra su diálogo de guardar | M | Auto-Rust + Manual | 1. Auto-Rust: un proceso de prueba que ignora `WM_CLOSE` se termina tras 5 s.<br>2. Manual: Bloc de notas con texto sin guardar, cerrar desde Glassdeck: sale «¿Desea guardar?».<br>3. Anotar qué pasa si no se responde en 5 s | Pendiente |
| GD-PROC-07 | Resultado por proceso | El resumen muestra cerrado, rechazado, sin permiso o ya no existía | M | Auto-Rust + Manual | 1. Auto-Rust: cada caso produce su resultado.<br>2. Manual: seleccionar un Bloc de notas, uno cerrado a mano antes de confirmar y una PowerShell de administrador.<br>3. Captura del resumen | Pendiente |
| GD-PROC-08 | Glassdeck nunca se eleva para cerrar procesos | Proceso de admin: «Sin permiso» y sin UAC | M | Revisión + Manual | 1. Revisión: manifiesto `asInvoker`; sin `runas` en la ruta de procesos.<br>2. Manual: abrir PowerShell como administrador, intentar cerrarla: «Sin permiso», sin UAC | Pendiente |
| GD-PROC-09 | `explorer` no se cierra; enlace a «Reiniciar Explorer» | La fila de `explorer` muestra el enlace | M | Auto-Vitest + Manual | 1. Vitest: la fila de `explorer` renderiza el enlace y no la casilla.<br>2. Manual: verificar la fila en la lista completa | Pendiente |

### GD-CLEAN — Limpieza de temporales

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-CLEAN-01 | Vista previa por origen: `%TEMP%`, `C:\Windows\Temp` (UAC), papelera (desmarcada) | Tamaños < 5 % de diferencia vs lo real | M | Auto-Rust + Manual | 1. Auto-Rust: carpeta de prueba con archivos de tamaño conocido; la vista previa suma igual.<br>2. Manual: la papelera sale desmarcada; anotar vista previa, limpiar y comparar con lo liberado | Pendiente |
| GD-CLEAN-02 | Solo archivos con > 24 h; los que están en uso se omiten | Un archivo abierto no se borra ni detiene la limpieza | M | Auto-Rust + Manual | 1. Auto-Rust: archivo de 2 días se borra, de 1 h no, bloqueado se omite.<br>2. Manual: crear archivo en `%TEMP%` con fecha de hace 2 días (`(Get-Item f).LastWriteTime = (Get-Date).AddDays(-2)`) y abrirlo en otra app; limpiar | Pendiente |
| GD-CLEAN-03 | `C:\Windows\Temp` con ayudante elevado solo para esa operación | Si se cancela UAC, el resto sigue y el resumen lo dice | M | Manual | 1. Marcar `%TEMP%` y `C:\Windows\Temp`.<br>2. Cancelar el UAC.<br>3. `%TEMP%` se limpia y el resumen indica «cancelado» en Windows\Temp | Pendiente |
| GD-CLEAN-04 | Nunca seguir enlaces simbólicos ni uniones | Una unión en `%TEMP%` no borra su destino | M | Auto-Rust + Manual | 1. Auto-Rust: recorrido con unión de prueba no entra al destino.<br>2. Manual: `mklink /J %TEMP%\qa_union C:\QA_destino` con archivo de hace 2 días en el destino; limpiar; el destino sigue intacto | Pendiente |
| GD-CLEAN-05 | Resumen: espacio liberado, omitidos y errores | Coincide con la diferencia de espacio libre ± 5 % | M | Script + Manual | 1. Script: anotar `(Get-PSDrive C).Free` antes y después.<br>2. Cerrar apps que escriben a disco durante la prueba.<br>3. Comparar con el resumen | Pendiente |

### GD-SHORT-01 — Atajos de limpieza y procesos

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-SHORT-01 | Botones «Limpiar temporales» y «Cerrar procesos pesados» | Cada botón abre su flujo con vista previa | M | Manual | 1. «Limpiar temporales»: abre la vista previa de GD-CLEAN.<br>2. «Cerrar procesos pesados»: abre GD-PROC maximizado.<br>3. Ninguno ejecuta nada sin confirmar | Pendiente |

### GD-HOT — Atajos globales (GD-HOT-03 pasa a fase 4)

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-HOT-01 | `Win+Alt+G`: mostrar u ocultar todos los widgets | Funciona desde cualquier app | M | Manual | 1. Probar con el foco en Edge, Cursor, Explorador y Bloc de notas.<br>2. Repetir con el dock oculto | Pendiente |
| GD-HOT-02 | `Win+Alt+P`: modo privacidad | Funciona desde cualquier app | M | Manual | 1. Probar desde las mismas 4 apps.<br>2. Confirmar el cambio del indicador de privacidad en dock y bandeja (contenido sensible llega en fase 2) | Pendiente |
| GD-HOT-04 | Detección de conflictos al registrar | Igual que GD-CUST-06 | M | Manual | 1. Ocupar `Win+Alt+G` con otra app antes de abrir Glassdeck.<br>2. Abrir Glassdeck: aviso de conflicto y el resto de atajos funciona | Pendiente |

### GD-INST — Instalación y ciclo de vida

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-INST-01 | Instalador NSIS por usuario, sin firma, sin admin | Instala en `%LocalAppData%\Programs\Glassdeck` sin UAC | M | Revisión + Manual | 1. Revisión: NSIS con instalación por usuario en `tauri.conf.json`.<br>2. Manual: instalar; aceptar SmartScreen (R-09); no aparece UAC.<br>3. Verificar la carpeta | Pendiente |
| GD-INST-02 | Arranque con Windows | Widgets visibles < 10 s tras iniciar sesión | M | Manual | 1. Reiniciar Windows.<br>2. Cronometrar desde que aparece el escritorio hasta ver los widgets.<br>3. Repetir 3 veces | Pendiente |
| GD-INST-03 | Desinstalación limpia; conservar o borrar configuración y credenciales | Al borrar, no quedan entradas de Glassdeck en el Administrador de credenciales | M | Manual + Script | 1. Crear entrada de prueba: `cmdkey /generic:Glassdeck/qa /user:qa /pass:x`.<br>2. Desinstalar eligiendo borrar.<br>3. `cmdkey /list \| Select-String Glassdeck` vacío; `%AppData%\Glassdeck` borrada | Pendiente |
| GD-INST-04 | Sin actualización automática ni telemetría | Sin conexiones de red fuera de la sección 5.3 | M | Revisión + Manual | 1. Revisión: sin plugin de actualización; `connect-src` de la CSP limitado.<br>2. Manual: Monitor de recursos > Red, filtrar procesos de Glassdeck 30 min.<br>3. Esperado en fase 1: cero conexiones | Pendiente |
| GD-INST-05 | Requiere WebView2; el instalador lo descarga si falta | Instalación probada en el equipo de Luis | M | Revisión + Manual | 1. Revisión: modo de instalación de WebView2 por descarga en `tauri.conf.json`.<br>2. Manual: instalar en el ThinkPad y abrir todos los widgets | Pendiente |

### GD-NFR-PERF — Rendimiento

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-NFR-PERF-01 | CPU < 2 % en reposo, todos los widgets visibles | Promedio de 10 min sin interactuar | M\* | Script | 1. Todos los widgets visibles, sin tocar el equipo.<br>2. `poc/medir.ps1` (o Monitor de rendimiento) toma `% de tiempo de procesador` del árbol de Glassdeck cada 1 s por 10 min, dividido entre núcleos lógicos.<br>3. Anotar promedio | Pendiente |
| GD-NFR-PERF-02 | RAM < 200 MB total en fase 1; < 250 MB con 11 widgets (ADR-001) | Suma del conjunto de trabajo privado de todos los procesos | M\* | Script | 1. Todos los widgets visibles 10 min.<br>2. `poc/medir.ps1` suma `Working Set - Private` del árbol cada 5 s.<br>3. Anotar promedio y máx. | Pendiente |
| GD-NFR-PERF-03 | Con batería: animaciones pausadas e intervalos < 3 s → 3 s | Cambia en < 5 s al desconectar el cargador | M\* | Auto-Rust + Manual | 1. Auto-Rust: el planificador con batería simulada devuelve 3 s.<br>2. Manual: desconectar el cargador y cronometrar.<br>3. Contar actualizaciones de CPU en 30 s (~10) y ver animaciones detenidas | En prueba (Auto-Rust pasa: con batería CPU, RAM y red no bajan de 3 s; `App.tsx` apaga animaciones con batería; falta desconectar el cargador) |
| GD-NFR-PERF-04 | Ocultos, cerrados o tapados por pantalla completa no recolectan ni dibujan | Con app a pantalla completa, CPU < 0.5 % | M\* | Auto-Rust + Script | 1. Auto-Rust: con estado «pantalla completa» o widget oculto, el planificador pausa.<br>2. Script: video o presentación a pantalla completa 5 min, medir CPU del árbol.<br>3. Anotar promedio | En prueba (Auto-Rust pasa: con pantalla completa o pausa no vence ninguna fuente; widgets ocultos o cerrados no se suscriben; falta el script de 5 min) |
| GD-NFR-PERF-05 | Intervalos por defecto (CPU/RAM/red 2 s, procesos 3 s, disco 5 s…) | Valores de la configuración inicial | M\* | Auto-Rust | 1. Auto-Rust: la configuración por defecto tiene exactamente los valores de la SPEC.<br>2. Con instalación limpia, Customize muestra esos valores | En prueba (Auto-Rust pasa: valores por defecto 2 s, 3 s y 5 s; falta revisar Customize con instalación limpia) |
| GD-NFR-PERF-06 | Arranque en frío < 3 s | Medido desde el inicio del proceso hasta ver los widgets | M\* | Script | 1. Reiniciar Windows (arranque en frío) con autoarranque apagado.<br>2. Script: `Start-Process` y esperar a que todas las ventanas de widgets estén visibles.<br>3. 5 corridas; anotar máx. | Pendiente |

### GD-NFR-SEC — Seguridad y privacidad

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-NFR-SEC-01 | Secretos solo en el Administrador de credenciales | Revisión de código + búsqueda en disco | M\* | Revisión + Script | 1. Revisión: solo `seguridad/` usa `keyring`; ningún secreto se escribe en JSON ni log.<br>2. Script: buscar valor de prueba de la bóveda en `%AppData%\Glassdeck` y logs: sin resultados.<br>3. Búsqueda completa con cookies reales en fase 2 (GD-CUR-02) | Pendiente |
| GD-NFR-SEC-02 | Logs sin tokens, cookies, asuntos, títulos ni rutas personales | Revisión de un log de 1 día | M\* | Auto-Rust + Revisión | 1. Auto-Rust: el filtro enmascara token, cookie y `C:\Users\hulkmtz`.<br>2. Revisión: tras 1 día, `Select-String -Pattern 'hulkmtz\|Bearer\|token\|cookie'` en logs sin resultados | En prueba (Auto-Rust pasa; log actual sin coincidencias; falta la revisión de 1 día) |
| GD-NFR-SEC-03 | Frontend sin red ni disco; capacidades restringidas y CSP estricta | Revisión de capacidades | M\* | Revisión + Manual | 1. Revisión: `capabilities/*.json` sin permisos de fs ni http; CSP en `tauri.conf.json`.<br>2. Manual: en DevTools de un widget, `fetch('https://example.com')` queda bloqueado | Pendiente |
| GD-NFR-SEC-05 | Sin privilegios; ayudante elevado de un solo uso con parámetros validados | No hay procesos de Glassdeck elevados en reposo | M\* | Revisión + Manual | 1. Revisión: el ayudante solo acepta códigos de acción cerrados.<br>2. Manual: Administrador de tareas > Detalles, columna «Elevado» = No.<br>3. Tras limpiar `C:\Windows\Temp`, `glassdeck-helper.exe` ya no existe | Pendiente |
| GD-NFR-SEC-06 | Ningún dato sale salvo hacia fuentes de 5.3 | Revisión de conexiones con Monitor de recursos | M\* | Manual | 1. Monitor de recursos > Red, filtrar procesos de Glassdeck 30 min con uso normal.<br>2. Esperado en fase 1: cero conexiones (misma evidencia que GD-INST-04) | Pendiente |

Aplazados a **Fase 2** (dependen de módulos de esa fase):

- **GD-NFR-SEC-04** — Ventanas de login aisladas sin comandos. Se verifica con GD-CUR-01 y GD-CLA-04.
- **GD-NFR-SEC-07** — Correo con posibles datos de pacientes. Se verifica con GD-MAIL-02, GD-MAIL-05 y GD-ROT-02.

### GD-NFR-REL — Confiabilidad

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-NFR-REL-01 | La falla de una fuente no afecta a los demás widgets | Sin red, los widgets de sistema siguen funcionando | M\* | Manual | 1. Apagar Wi-Fi y desconectar Ethernet 5 min.<br>2. CPU, RAM, disco y procesos siguen actualizando; sin errores.<br>3. Verificación con fuentes externas en fase 2 | Pendiente |
| GD-NFR-REL-02 | Reintentos con espera creciente (1, 2, 5, 15 min) | Sin ráfagas de peticiones al volver la red | M\* | Auto-Rust | 1. Auto-Rust: la función de espera devuelve 1, 2, 5, 15 y se queda en 15.<br>2. Si el módulo no existe en fase 1, marcar Diferido a fase 2 | Diferido (fase 2: la fase 1 no tiene fuentes de red) |
| GD-NFR-REL-03 | Escritura atómica de configuración con respaldo | Apagar durante un guardado no corrompe | M\* | Auto-Rust + Manual | 1. Auto-Rust: falla simulada entre escribir el temporal y renombrar deja el original válido y el respaldo.<br>2. Manual: cambiar opciones en bucle y matar el proceso con `Stop-Process -Force` 20 veces; `settings.json` siempre abre bien | En prueba (Auto-Rust pasa, incluye BOM y copia `.corrupto`; falta el bucle de 20 `Stop-Process`) |
| GD-NFR-REL-04 | Log rotativo: máx. 5 archivos de 2 MB | Verificado tras 1 semana | M\* | Auto-Rust + Revisión | 1. Auto-Rust: con límite pequeño, rota y conserva 5 archivos.<br>2. Revisión tras 1 semana: `Get-ChildItem %LocalAppData%\Glassdeck\logs` ≤ 5 archivos de ≤ 2 MB | En prueba (Auto-Rust pasa: rota y conserva 5; falta la revisión de 1 semana) |

### GD-NFR-UX — Accesibilidad y diseño

| ID | Requerimiento (resumen corto) | CA (resumen verificable) | Prio | Método | Pasos / evidencia esperada | Estado |
|---|---|---|---|---|---|---|
| GD-NFR-UX-01 | Contraste ≥ 4.5:1 sobre vidrio, oscuro y claro | Verificado con fondos claros y oscuros | M\* | Manual | 1. Capturas con fondo claro y oscuro, en tema oscuro y claro.<br>2. Medir el texto más tenue con Colour Contrast Analyser.<br>3. Anotar la razón mínima | Pendiente |
| GD-NFR-UX-02 | Todo operable con teclado y foco visible | Recorrido completo con Tab | M\* | Manual | 1. Recorrer con Tab cada widget, el dock y Customize.<br>2. Anotar todo elemento sin foco visible o inalcanzable | Pendiente |
| GD-NFR-UX-03 | Respetar «Mostrar animaciones en Windows» y `prefers-reduced-motion` | Con animaciones apagadas, no hay transiciones | M\* | Auto-Vitest + Manual | 1. Vitest: con `prefers-reduced-motion` las clases de transición no se aplican.<br>2. Manual: apagar «Mostrar animaciones en Windows», minimizar y maximizar: sin transiciones | En prueba (Vitest pasa: `tema.test.ts` comprueba que `prefers-reduced-motion` y `.sin-animacion` ponen todas las duraciones en 0; falta la prueba manual) |
| GD-NFR-UX-04 | Números con `tabular-nums` | Revisión visual | M\* | Revisión | 1. Revisar que los estilos de números usan `font-variant-numeric: tabular-nums`.<br>2. Grabar el medidor de CPU 30 s: el texto no salta de ancho | En prueba (revisión pasa: `.num` y `.gd-kv dd` usan `tabular-nums` en medidor, barras, píldora, gráficas, listas y diálogo; falta la grabación de 30 s) |
| GD-NFR-UX-05 | Diseño fiel al mockup aprobado | Comparación visual al cierre de la fase | M\* | Revisión | 1. Capturar la app en las mismas vistas que `mockups/capturas/` 01, 02, 03, 04, 05 y 10.<br>2. Comparar lado a lado y anotar diferencias | En prueba (comparación de la captura 01 del 2026-10-27: se agregaron la gráfica de CPU a todo lo ancho, barras de uso en Procesos y ventana real de «Último minuto»; quedan las capturas 02, 03, 04, 05 y 10 para el bloque 8) |
| GD-NFR-UX-06 | Título y subtítulo visibles en cada widget; la píldora incluye el nombre | Luis identifica cada widget sin abrir menús | M\* | Revisión + Manual | 1. Revisión: `MarcoWidget` exige `titulo` y `subtitulo`; la píldora antepone el título.<br>2. Manual: con los 5 widgets en el escritorio y uno en píldora, Luis nombra cada uno | En prueba (revisión pasa: `MarcoWidget` exige `titulo` y `subtitulo`; Vitest comprueba que la píldora muestra «CPU 23 %»; falta que Luis nombre cada widget) |

## Fases 2 a 4 (esqueleto)

### Fase 2 — Servicios

GD-DOCK-02, GD-CUST-04, GD-CUST-08, GD-CUR-01 a 06, GD-CLA-01 a 06, GD-ALERT-01 a 03, GD-MAIL-01 a 07, GD-CAL-01, GD-ROT-01 a 03, GD-WX-01 a 03, GD-NFR-SEC-04, GD-NFR-SEC-07.

Se detalla al iniciar la fase.

### Fase 3 — Controles

GD-AUD-01 a 03, GD-MIC-01, GD-CAM-01 a 03, GD-BRI-01 a 03, GD-SHORT-02 a 06.

Se detalla al iniciar la fase.

### Fase 4 — Zonas

GD-ZONE-01 a 15, GD-HOT-03.

Se detalla al iniciar la fase.

## Resumen de cobertura

«Auto» cuenta las filas con al menos una prueba Auto-Rust o Auto-Vitest; «Manual/Script/Revisión» cuenta las que solo usan esos métodos.

| Fase | Requerimientos | Auto | Manual/Script/Revisión |
|---|---|---|---|
| 0 | 1 (R-01) | 0 | 1 |
| 1 | 79 | 33 | 46 |
| 2 | 34 | Por definir | Por definir |
| 3 | 15 | Por definir | Por definir |
| 4 | 16 | Por definir | Por definir |
| **Total SPEC** | **144** (+ R-01) | — | — |

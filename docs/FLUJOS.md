# Glassdeck — Diagramas de flujo

Complementa a `SPEC.md` y `ARQUITECTURA.md`.

## 1. Arranque

```mermaid
flowchart TD
    A([Inicio de sesión de Windows<br/>o doble clic]) --> B{¿Ya hay una instancia?}
    B -- Sí --> B1[Enfocar la instancia existente] --> Z([Fin])
    B -- No --> C[Leer settings.json]
    C --> C1{¿Válido?}
    C1 -- No --> C2[Restaurar respaldo o valores por defecto<br/>y registrar aviso] --> D
    C1 -- Sí --> D[Enumerar monitores y sus huellas]
    D --> E[Crear el icono de la bandeja y el dock]
    E --> F[Por cada widget activo]
    F --> G{¿Su monitor está conectado?}
    G -- Sí --> H[Colocar en su posición guardada]
    G -- No --> I[Colocar en el monitor principal<br/>sin superponerse]
    H & I --> J[Aplicar acrylic y capa<br/>escritorio o siempre encima]
    J --> K[Registrar atajos globales]
    K --> K1{¿Algún atajo en conflicto?}
    K1 -- Sí --> K2[Aviso en el dock<br/>y marca en Customize]
    K1 -- No --> L
    K2 --> L[Iniciar el planificador<br/>según batería y visibilidad]
    L --> M[Leer credenciales de la bóveda<br/>y validar sesiones en segundo plano]
    M --> N[Actualizar los puntos de estado del dock]
    N --> Z
```

## 2. Login de Google (una o varias cuentas)

```mermaid
sequenceDiagram
    actor L as Luis
    participant D as Dock
    participant N as Núcleo
    participant B as Navegador del sistema
    participant G as Google
    participant V as Bóveda (Credenciales de Windows)

    L->>D: Conectar cuenta de Google
    D->>N: iniciar_login_google()
    N->>N: Generar PKCE y state<br/>abrir un puerto local aleatorio en 127.0.0.1
    N->>B: Abrir URL de autorización<br/>(gmail.metadata + calendar.events.readonly)
    B->>G: Elegir cuenta y consentir
    G-->>B: Advertencia «Google no verificó esta app»
    L->>B: Configuración avanzada → Ir a Glassdeck
    G-->>N: Redirección con código (al puerto local)
    N->>N: Validar state
    N->>G: Intercambiar código + PKCE por tokens
    G-->>N: access_token + refresh_token
    N->>V: Guardar refresh_token (Glassdeck/google/<correo>)
    N-->>D: Cuenta conectada (punto verde)
    N->>N: Programar correo (5 min) y calendario (10 min)
```

## 3. Login de Cursor y de claude.ai (no oficial)

```mermaid
flowchart TD
    A([Luis pulsa Conectar Cursor o Claude.ai]) --> B[Crear ventana de login aislada<br/>perfil WebView2 temporal, sin comandos]
    B --> C[Cargar cursor.com o claude.ai]
    C --> D[Luis inicia sesión]
    D --> E{¿Navegó al dashboard<br/>o a la página principal con sesión?}
    E -- No, cerró la ventana --> X[Cancelado, punto gris] --> Z([Fin])
    E -- Sí --> F[Leer la cookie de sesión<br/>del almacén de esa ventana]
    F --> G[Guardar en la bóveda]
    G --> H[Destruir ventana y perfil temporal]
    H --> I[Lectura de prueba con el adaptador]
    I --> J{¿Respuesta con el formato esperado?}
    J -- Sí --> K[Punto verde y primer dato en Spending]
    J -- No --> L[Punto ámbar: «Formato cambió»<br/>sin mostrar números]
    K & L --> Z
```

Claude Code no requiere login: el recolector lee los registros locales al arrancar y cada 15 min de forma incremental.

## 4. Cerrar procesos

```mermaid
flowchart TD
    A([Luis abre Procesos maximizado<br/>o el atajo «Cerrar procesos pesados»]) --> B[Lista completa clasificada<br/>por la política de 3 niveles]
    B --> C[Lista negra: candado, sin casilla<br/>explorer: enlace a Reiniciar Explorer]
    B --> D[Luis marca uno o varios]
    D --> E[Cerrar seleccionados]
    E --> F[El núcleo vuelve a clasificar cada PID]
    F --> G{¿Alguno en lista negra<br/>o ya no existe?}
    G -- Sí --> G1[Se excluye y se marca<br/>«rechazado» o «ya no existía»]
    G -- No --> H
    G1 --> H{¿Alguno de confirmación reforzada?<br/>Cursor, Glassdeck, claude, Code,<br/>terminales, node}
    H -- Sí --> I[Diálogo con los nombres<br/>escribir CERRAR]
    H -- No --> J[Diálogo de confirmación única]
    I --> K{¿Confirmó?}
    J --> K
    K -- No --> Z([Nada se cierra])
    K -- Sí --> L[Por proceso: WM_CLOSE a sus ventanas]
    L --> M{¿Terminó en 5 s?}
    M -- Sí --> R[cerrado]
    M -- No --> N[Terminación forzada]
    N --> O{¿Acceso denegado?}
    O -- Sí --> P[«Sin permiso»<br/>sin pedir UAC]
    O -- No --> R
    R & P --> Q[Resumen por proceso]
    Q --> Z2([Fin])
```

## 5. Limpiar temporales

```mermaid
flowchart TD
    A([Luis pulsa Limpiar temporales]) --> B[Calcular vista previa<br/>sin seguir uniones ni enlaces]
    B --> C["%TEMP% del usuario<br/>archivos con más de 24 h"]
    B --> D["C:\Windows\Temp<br/>tamaño estimado · escudo UAC"]
    B --> E[Papelera<br/>desmarcada por defecto]
    C & D & E --> F[Mostrar tamaños y casillas]
    F --> G{¿Luis confirma?}
    G -- No --> Z([Nada se borra])
    G -- Sí --> H[Borrar %TEMP%<br/>omitir archivos en uso]
    H --> I{¿C:\Windows\Temp marcado?}
    I -- Sí --> J[Pedir UAC para el ayudante<br/>acción cerrada: limpiar_windows_temp]
    J --> K{¿Aceptó UAC?}
    K -- Sí --> L[El ayudante limpia y devuelve el resultado]
    K -- No --> M[Se omite y se anota]
    I -- No --> N
    L & M --> N{¿Papelera marcada?}
    N -- Sí --> O[Vaciar papelera]
    N -- No --> P
    O --> P[Resumen: liberado, omitidos, errores]
    P --> Z2([Fin])
```

## 6. Rotación de la alerta de correo y agenda

```mermaid
flowchart TD
    A([Temporizador de correo 5 min<br/>y de calendario 10 min]) --> B[Por cada cuenta conectada]
    B --> C[Gmail: no leídos en INBOX<br/>solo From, Subject, Date]
    B --> D[Calendar: eventos de hoy<br/>y próximas 24 h]
    C & D --> E{¿Error de permiso?}
    E -- Sí --> E1[Cuenta en ámbar<br/>se sigue con las demás]
    E -- No --> F[Actualizar la cola en memoria]
    E1 --> F
    F --> G([Tic de rotación cada 10 min])
    G --> H{¿Hay un evento que empieza<br/>en menos de 30 min?}
    H -- Sí --> I[Mostrar ese evento]
    H -- No --> J[Elegir al azar un correo o evento<br/>no mostrado en esta vuelta]
    I & J --> K{¿Modo privacidad activo?}
    K -- Sí --> L[Mostrar desenfocado]
    K -- No --> M[Mostrar remitente o título, asunto y hora]
    L & M --> N{¿Luis hace clic?}
    N -- Sí --> O[Abrir en Gmail o Calendar<br/>en la cuenta correcta]
    N -- No --> G
```

## 7. Aviso de umbral de gasto

```mermaid
flowchart TD
    A([Lectura de spending cada 15 min]) --> B{¿Lectura válida?}
    B -- No --> B1[Mostrar motivo y última lectura válida<br/>sin evaluar umbrales] --> Z([Fin])
    B -- Sí --> C[Calcular % de Cursor sobre el presupuesto<br/>y % de Claude en 5 h y semanal]
    C --> D{¿Cruzó un umbral nuevo<br/>50/80/100 % Cursor · 80 % Claude?}
    D -- No --> Z
    D -- Sí --> E{¿Ya se avisó en este ciclo<br/>o ventana de límite?}
    E -- Sí --> Z
    E -- No --> F[Notificación de Windows]
    F --> G[Guardar en state.json]
    G --> H{¿Luis hace clic?}
    H -- Sí --> I[Abrir Spending maximizado]
    H -- No --> Z
```

## 8. Colocar una ventana en una zona

### 8.1 Con la superposición (atajo)

```mermaid
flowchart TD
    A([Atajo de zonas, por defecto Win+Alt+Espacio]) --> B[Tomar la ventana en primer plano]
    B --> C{¿Es elegible?}
    C -- No, es elevada --> C1[Aviso: Windows no permite<br/>mover esta ventana] --> Z([Fin])
    C -- No, otra causa --> Z
    C -- Sí --> D[Abrir superposición en el monitor<br/>de la ventana con su cuadrícula]
    D --> E[Luis arrastra sobre las celdas<br/>resaltado en vivo]
    E --> F{Tecla durante la selección}
    F -- PageUp / PageDown --> G[Mover la superposición<br/>al monitor anterior o siguiente] --> E
    F -- Esc --> Z
    F -- Ninguna --> H[Soltar: colocar la ventana]
    F -- Shift --> I[Soltar: colocar y guardar regla<br/>para esa app]
    F -- Ctrl --> J[Soltar: guardar como preset<br/>y pedir atajo]
    H & I & J --> K[Compensar bordes DWM<br/>y aplicar posición]
    K --> L[Cerrar superposición]
    L --> Z
```

### 8.2 Al arrastrar con Shift

```mermaid
sequenceDiagram
    actor L as Luis
    participant W as Ventana de otra app
    participant H as Eventos de ventanas
    participant Z as Motor de zonas
    participant O as Resaltado de zonas

    L->>W: Empieza a arrastrar la barra de título
    W-->>H: EVENT_SYSTEM_MOVESIZESTART
    H->>Z: Inicio de arrastre (ventana elegible)
    loop Cada 16 ms mientras arrastra
        Z->>Z: Leer cursor y estado de Shift
        alt Shift presionado
            Z->>O: Mostrar zonas del monitor bajo el cursor<br/>y resaltar la zona bajo el cursor
        else Shift suelto
            Z->>O: Ocultar
        end
    end
    L->>W: Suelta
    W-->>H: EVENT_SYSTEM_MOVESIZEEND
    H->>Z: Fin de arrastre
    alt Había zona resaltada
        Z->>W: Ajustar a la zona (con compensación DWM)
    end
    Z->>O: Ocultar
```

## 9. Cambio de monitores

```mermaid
flowchart TD
    A([WM_DISPLAYCHANGE o<br/>cambio de dispositivos]) --> B[Esperar 500 ms sin más cambios]
    B --> C[Volver a enumerar monitores<br/>y calcular huellas]
    C --> D{¿Qué cambió?}
    D -- Se desconectó un monitor --> E[Widgets de ese monitor →<br/>monitor principal, en cascada sin superponerse]
    E --> E1[Recordar su monitor original]
    D -- Se conectó un monitor --> F{¿Su huella es conocida?}
    F -- Sí --> G[Devolver sus widgets<br/>a su posición guardada]
    G --> H[Si Zonas está activo:<br/>Recuperar todo]
    F -- No --> I[Crear cuadrícula por defecto<br/>para esa huella]
    D -- Cambió resolución o escala --> J[Reescalar posiciones proporcionalmente<br/>y recalcular zonas]
    E1 & H & I & J --> K[Guardar estado]
    K --> Z([Fin])
```

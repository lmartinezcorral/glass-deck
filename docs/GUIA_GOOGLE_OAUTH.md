# Guía paso a paso: proyecto de Google Cloud para Glassdeck

Glassdeck lee tus correos no leídos (solo remitente, asunto y fecha) y tus eventos de Google Calendar. Para eso necesita un **proyecto propio de Google Cloud** con un cliente OAuth de escritorio. Lo haces una sola vez; después conectas todas las cuentas de Google que quieras desde **Customize › Cuentas**.

Tiempo estimado: 15 minutos. Costo: gratis.

## Antes de empezar

- Usa la cuenta de Google con la que quieras administrar el proyecto. Puede ser distinta de las cuentas cuyos correos vas a leer.
- **Cuentas de Google Workspace** (dominio propio): si alguna de tus cuentas es de un dominio de empresa, su administrador puede bloquear apps no verificadas. En ese caso ve al paso 8.
- La consola de Google cambia su interfaz con frecuencia. Si un nombre no coincide exactamente, busca la opción equivalente en el menú **Google Auth Platform**.

## Paso 1. Crear el proyecto

1. Abre [console.cloud.google.com](https://console.cloud.google.com/) e inicia sesión.
2. En el selector de proyectos (arriba a la izquierda), elige **Proyecto nuevo**.
3. Nombre: `Glassdeck personal`. Organización: «Sin organización» (si aparece).
4. Pulsa **Crear** y selecciona el proyecto cuando termine.

## Paso 2. Activar las APIs

1. Menú ☰ → **APIs y servicios** → **Biblioteca**.
2. Busca **Gmail API** → **Habilitar**.
3. Regresa a la biblioteca, busca **Google Calendar API** → **Habilitar**.

## Paso 3. Configurar la pantalla de consentimiento

1. Menú ☰ → **Google Auth Platform** → **Descripción general** → **Comenzar** (o **Branding**).
2. Información de la app:
   - Nombre de la app: `Glassdeck`
   - Correo de asistencia: tu correo.
3. Público (**Audience**): elige **Externo**.
4. Información de contacto: tu correo.
5. Acepta la política de datos de usuario y pulsa **Crear**.
6. **Información de la marca** → **Dominio de la app** (sin esto Google no deja publicar en el paso 5):
   - Página principal: `https://glassdeck.newachi.mx/`
   - Política de privacidad: `https://glassdeck.newachi.mx/aviso_privacidad`
   - Condiciones del servicio: `https://glassdeck.newachi.mx/condiciones_servicio`
   - **Dominios autorizados** → **Agregar dominio**: `newachi.mx`
   - Pulsa **Guardar**.

El sitio vive en el repo `site-newachi`, carpeta `glassdeck/` (deploy en el V110 con `scripts/deploy-glassdeck.sh`).

## Paso 4. Declarar los permisos (scopes)

1. **Google Auth Platform** → **Acceso a los datos** (*Data Access*) → **Agregar o quitar permisos**.
2. Agrega solo estos tres:

| Permiso | Para qué | Tipo según Google |
|---|---|---|
| `https://www.googleapis.com/auth/gmail.metadata` | Leer encabezados (remitente, asunto, fecha) y etiquetas. **No permite leer el cuerpo de los correos** | Restringido |
| `https://www.googleapis.com/auth/calendar.events.readonly` | Leer eventos de tus calendarios | Sensible |
| `https://www.googleapis.com/auth/calendar.calendarlist.readonly` | Saber qué calendarios tienes marcados como visibles | Sensible |

3. Pulsa **Actualizar** y luego **Guardar**.

Glassdeck además pide `openid` y `email`, que Google siempre permite y no hace falta declarar: sirven para saber qué cuenta conectaste y mostrar su correo en Customize y en el dock.

No agregues permisos de escritura ni `gmail.readonly`: Glassdeck no los necesita y ampliarías lo que la app puede ver.

## Paso 5. Publicar como «producción sin verificar»

Esto evita que Google te pida volver a iniciar sesión cada 7 días, que es lo que pasa en modo de prueba.

1. **Google Auth Platform** → **Público** (*Audience*).
2. En **Estado de publicación** pulsa **Publicar app** → **Confirmar**.
3. Google mostrará que la app necesita verificación. **No la envíes a verificar.** Para uso personal (menos de 100 usuarios) no es obligatorio: la app funciona y solo verás una advertencia al iniciar sesión.
4. Comprueba que el estado diga **En producción**. Si se queda en **Prueba**, Google bloquea cualquier cuenta que no esté en **Usuarios de prueba** con el mensaje «Acceso bloqueado».

## Paso 6. Crear el cliente OAuth de escritorio

1. **Google Auth Platform** → **Clientes** → **Crear cliente**.
2. Tipo de aplicación: **App de escritorio**.
3. Nombre: `Glassdeck escritorio`.
4. Pulsa **Crear** y luego **Descargar JSON**.
5. Guarda el archivo temporalmente, por ejemplo en Descargas. Contiene el ID y el secreto del cliente.

En una app de escritorio, el «secreto» del cliente no es realmente secreto (Google lo sabe y por eso exige PKCE). Aun así, Glassdeck lo guarda en el Administrador de credenciales de Windows y no lo deja en archivos.

## Paso 7. Conectar Glassdeck

1. En Glassdeck: **Customize** → **Cuentas** → **Importar cliente de Google…** → elige el JSON del paso 6.
2. **Borra el JSON** de Descargas: Glassdeck ya lo guardó en la bóveda.
3. En la misma sección pulsa **Conectar Google**. Se abre tu navegador; tienes 5 minutos para terminar (o pulsa **Cancelar conexión**).
4. Elige la cuenta. Aparecerá **«Google no verificó esta app»**:
   - Pulsa **Configuración avanzada** (*Advanced*).
   - Pulsa **Ir a Glassdeck (no seguro)**. Es tu propia app.
5. Revisa los permisos: ver metadatos de Gmail y leer tu calendario. Deja **marcadas** las casillas de Gmail y de eventos (sin ellas Glassdeck no conecta la cuenta) y pulsa **Continuar**.
6. El navegador muestra «Glassdeck conectado. Puedes cerrar esta ventana.». En Customize y en el dock, la cuenta aparece en verde.
7. Para cada cuenta adicional pulsa **Conectar otra cuenta** y repite los pasos 4 a 6.
8. En **Customize** → **Widgets** activa **Correo y agenda** (viene apagado).

Glassdeck guarda en la bóveda solo el token de renovación de cada cuenta y la lista de cuentas conectadas; los correos y eventos viven en memoria y nunca se escriben en disco.

## Paso 8. Solo si una cuenta es de Google Workspace

Si al conectar una cuenta de dominio propio ves «Acceso bloqueado» o «Esta app está bloqueada»:

1. Inicia sesión en [admin.google.com](https://admin.google.com/) como administrador del dominio.
2. **Seguridad** → **Acceso y control de datos** → **Controles de API** → **Administrar acceso de apps de terceros**.
3. **Configurar app nueva** → busca por el **ID del cliente** del paso 6.
4. Marca la app como **Confiable** para tu usuario o unidad organizativa.
5. Vuelve a intentar el paso 7.

Si no eres administrador del dominio, esa cuenta no se podrá conectar sin la ayuda de quien lo sea.

## Cómo revocar el acceso

- **Una cuenta:** en **Customize** → **Cuentas**, **Desconectar**. Glassdeck revoca el acceso en Google y borra su token de la bóveda.
- **Desde Google:** [myaccount.google.com/permissions](https://myaccount.google.com/permissions) → Glassdeck → **Quitar acceso**. El dock mostrará esa cuenta en ámbar en el siguiente ciclo.
- **Todo:** borra el proyecto en la consola de Google Cloud.

## Problemas frecuentes

| Síntoma | Causa probable | Solución |
|---|---|---|
| **Publicar app** aparece en gris («Se requieren un nombre de app, un correo electrónico de asistencia, una URL de página principal…») | Faltan las URLs del dominio de la app | Paso 3, punto 6 |
| Te pide iniciar sesión cada 7 días | La app quedó en modo de prueba | Paso 5 |
| «Acceso bloqueado: … no completó el proceso de verificación» o «solo pueden acceder los testers aprobados» | La app sigue en modo de prueba y tu cuenta no está en la lista de usuarios de prueba | Publica la app (paso 5) o, en **Público** → **Usuarios de prueba**, pulsa **Agregar usuarios** y escribe cada cuenta que vas a conectar |
| `access_denied` o «app bloqueada» en una cuenta de dominio propio | Cuenta de Workspace con apps no verificadas bloqueadas | Paso 8 |
| `redirect_uri_mismatch` | El cliente no es de tipo «App de escritorio» | Crea otro cliente, paso 6 |
| La cuenta queda en ámbar | Revocaste el acceso o cambiaste la contraseña | **Reconectar** en Customize › Cuentas |
| «Deja marcados los permisos de Gmail y de Calendar» | Desmarcaste una casilla en la pantalla de consentimiento | Conecta de nuevo y deja las casillas marcadas |
| «Esta API no se usó antes o está deshabilitada» | Falta habilitar Gmail API o Calendar API | Paso 2 |

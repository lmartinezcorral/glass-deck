import { useCallback, useEffect, useState } from 'react';
import { api, enTauri, escuchar, EVENTOS, mensajeError, type CuentaGoogle, type EstadoCredencial } from '../api';
import { Dialogo } from '../componentes';
import { credencialPresente, PuntoCuenta, textoEstadoCuenta, type EstadoCuenta, type ResumenCuenta } from './Cuentas';

const REVISION_MS = 5 * 60_000;

const PASOS_GUIA: readonly string[] = [
  'En console.cloud.google.com crea el proyecto «Glassdeck personal».',
  'Habilita Gmail API y Google Calendar API.',
  'Configura la pantalla de consentimiento: app «Glassdeck», público Externo.',
  'Agrega solo los permisos gmail.metadata, calendar.events.readonly y calendar.calendarlist.readonly.',
  'Publica la app como producción sin enviarla a verificar.',
  'Crea un cliente OAuth de tipo «App de escritorio» y descarga su JSON.',
  'Importa ese JSON aquí y después bórralo de Descargas.',
];

/** Cuentas conectadas: al montar, con `gd://google` y `gd://config`, al enfocar la ventana y cada 5 min. */
export function useCuentasGoogle(): [CuentaGoogle[] | null, () => void] {
  const [cuentas, setCuentas] = useState<CuentaGoogle[] | null>(null);
  const recargar = useCallback(() => {
    if (!enTauri()) return;
    api
      .googleCuentas()
      .then((c) => setCuentas(c ?? []))
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    recargar();
    const quitarGoogle = escuchar<null>(EVENTOS.google, recargar);
    const quitarConfig = escuchar<null>(EVENTOS.config, recargar);
    const id = window.setInterval(recargar, REVISION_MS);
    window.addEventListener('focus', recargar);
    return () => {
      quitarGoogle.then((f) => f());
      quitarConfig.then((f) => f());
      window.clearInterval(id);
      window.removeEventListener('focus', recargar);
    };
  }, [recargar]);
  return [cuentas, recargar];
}

/** GD-DOCK-02: una entrada por cuenta de Google (verde o ámbar); sin cuentas queda la entrada gris. */
export function resumenConGoogle(resumen: readonly ResumenCuenta[], cuentas: readonly CuentaGoogle[] | null): ResumenCuenta[] {
  if (!cuentas?.length) return [...resumen];
  const base = resumen.find((c) => c.id === 'google');
  if (!base) return [...resumen];
  const porCuenta: ResumenCuenta[] = cuentas.map((c) => ({
    ...base,
    clave: `google:${c.id}`,
    nombre: `Google (${c.correo})`,
    estado: c.estado,
    detalle: c.estado === 'conectada' ? 'Gmail y Calendar al día' : 'Google retiró el acceso · reconecta la cuenta',
  }));
  return resumen.flatMap((c) => (c.id === 'google' ? porCuenta : [c]));
}

function estadoGeneral(cuentas: readonly CuentaGoogle[] | null): EstadoCuenta {
  if (!cuentas?.length) return 'sin_conectar';
  return cuentas.some((c) => c.estado === 'reconectar') ? 'reconectar' : 'conectada';
}

export function CuentasGoogle({ credenciales, recargar }: { credenciales: readonly EstadoCredencial[] | null; recargar: () => void }) {
  const [cuentas, recargarCuentas] = useCuentasGoogle();
  const [guia, setGuia] = useState(false);
  const [conectando, setConectando] = useState(false);
  const [mensaje, setMensaje] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const cliente = credencialPresente(credenciales, 'google_cliente');
  const estado = estadoGeneral(cuentas);

  const resultado = (p: Promise<unknown>, exito: string): void => {
    p.then(() => {
      setError(null);
      setMensaje(exito);
    })
      .catch((e) => {
        setMensaje(null);
        setError(mensajeError(e));
      })
      .finally(() => {
        recargar();
        recargarCuentas();
      });
  };

  const importar = (): void => {
    setGuia(false);
    api
      .googleClienteImportar()
      .then((importado) => {
        setError(null);
        if (importado) setMensaje('Cliente importado. Ya puedes borrar el JSON de Descargas: quedó en el Administrador de credenciales.');
        recargar();
      })
      .catch((e) => {
        setMensaje(null);
        setError(mensajeError(e));
      });
  };

  const conectar = (): void => {
    if (!cliente) {
      setGuia(true);
      return;
    }
    setConectando(true);
    setError(null);
    setMensaje('Autoriza en la ventana del navegador que se abrió (tienes 5 minutos).');
    resultado(
      api.googleConectar().finally(() => setConectando(false)),
      'Cuenta conectada. El widget Correo y agenda ya puede leerla.',
    );
  };

  return (
    <section className="gd-tarjeta gd-cuenta" aria-label="Google">
      <h3>
        Google
        <span className="gd-cuenta__estado gd-etiqueta">
          <PuntoCuenta estado={estado} />
          {textoEstadoCuenta(estado)}
        </span>
      </h3>
      <p className="gd-etiqueta">Gmail y Calendar, solo lectura de encabezados y eventos, varias cuentas</p>
      {cliente ? (
        <p className="gd-aviso-linea">
          <span className="gd-chip gd-chip--ok">Cliente importado</span> Conecta cada cuenta con el botón; Google te pedirá elegirla y aprobar los
          permisos.
        </p>
      ) : (
        <p className="gd-aviso-linea">Falta importar el cliente OAuth de tu proyecto de Google.</p>
      )}
      {cuentas && cuentas.length > 0 && (
        <ul className="gd-lista-editable" aria-label="Cuentas de Google conectadas">
          {cuentas.map((c) => (
            <li key={c.id} className="gd-cuenta__fila">
              <PuntoCuenta estado={c.estado} />
              <b>{c.correo}</b>
              <span className="gd-etiqueta">{c.estado === 'conectada' ? 'Conectada' : 'Google retiró el acceso'}</span>
              {c.estado === 'reconectar' && (
                <button type="button" className="gd-boton gd-boton--sm" disabled={conectando} onClick={conectar}>
                  Reconectar
                </button>
              )}
              <button
                type="button"
                className="gd-boton gd-boton--sm"
                aria-label={`Desconectar ${c.correo}`}
                onClick={() => resultado(api.googleDesconectar(c.id), 'Cuenta desconectada y acceso revocado en Google.')}
              >
                Desconectar
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="gd-panel__acciones">
        {cliente ? (
          <>
            <button type="button" className="gd-boton gd-boton--sm" onClick={() => resultado(api.bovedaBorrar('google_cliente'), 'Cliente de Google quitado.')}>
              Quitar cliente
            </button>
            <button type="button" className="gd-boton gd-boton--sm" onClick={importar}>
              Reemplazar cliente…
            </button>
          </>
        ) : (
          <button type="button" className="gd-boton gd-boton--sm" onClick={importar}>
            Importar cliente de Google…
          </button>
        )}
        {conectando ? (
          <button type="button" className="gd-boton gd-boton--sm" onClick={() => api.googleCancelar().catch(() => undefined)}>
            Cancelar conexión
          </button>
        ) : (
          <button type="button" className="gd-boton gd-boton--sm gd-boton--primario" onClick={conectar}>
            {cuentas?.length ? 'Conectar otra cuenta' : 'Conectar Google'}
          </button>
        )}
      </div>
      <p className="gd-etiqueta" role="status" aria-live="polite">
        {mensaje}
      </p>
      {error && (
        <p className="gd-widget__error" role="alert">
          {error}
        </p>
      )}
      <Dialogo
        abierto={guia}
        titulo="Conecta Google en 7 pasos"
        descripcion="Lo haces una sola vez: unos 15 minutos y sin costo. La guía completa está en docs/GUIA_GOOGLE_OAUTH.md."
        textoConfirmar="Importar cliente de Google…"
        textoCancelar="Cerrar"
        onConfirmar={importar}
        onCancelar={() => setGuia(false)}
      >
        <ol className="gd-guia">
          {PASOS_GUIA.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ol>
      </Dialogo>
    </section>
  );
}

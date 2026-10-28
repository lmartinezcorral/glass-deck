import { useState, type ReactNode } from 'react';
import { api, mensajeError } from '../api';
import { Vidrio } from '../componentes';
import { useCatalogo, useConfig, useEvento } from '../hooks';
import {
  IconoCartera,
  IconoCorreo,
  IconoCpu,
  IconoDisco,
  IconoEngrane,
  IconoEscoba,
  IconoOcultar,
  IconoOjo,
  IconoOjoTachado,
  IconoProcesos,
  IconoRam,
  IconoRed,
  IconoSolNube,
  IconoZonas,
} from '../iconos';
import { cuentaPorConectar, NOMBRE_SERVICIO, PuntoCuenta, resumenCuentas, textoEstadoCuenta, useCredenciales, useCuentasWeb } from './Cuentas';
import { resumenConGoogle, useCuentasGoogle } from './CuentasGoogle';

const ICONOS_WIDGET: Record<string, ReactNode> = {
  cpu: <IconoCpu />,
  ram: <IconoRam />,
  disco: <IconoDisco />,
  red: <IconoRed />,
  procesos: <IconoProcesos />,
  spending: <IconoCartera />,
  clima: <IconoSolNube />,
  correo: <IconoCorreo />,
};

export function VistaDock() {
  const [config, setConfig] = useConfig();
  const catalogo = useCatalogo();
  const [credenciales] = useCredenciales();
  const [web, recargarWeb] = useCuentasWeb();
  const [cuentasGoogle] = useCuentasGoogle();
  const privacidadEvento = useEvento<boolean | null>('gd://privacidad', null);
  const privacidad = privacidadEvento ?? config?.privacidad ?? false;
  const [error, setError] = useState<string | null>(null);

  const ejecutar = (p: Promise<unknown>): void => {
    p.then(() => setError(null)).catch((e) => setError(mensajeError(e)));
  };

  const alternarWidget = (id: string): void => {
    if (!config) return;
    const actual = config.widgets[id];
    const activo = !(actual?.activo ?? false);
    if (actual) setConfig({ ...config, widgets: { ...config.widgets, [id]: { ...actual, activo } } });
    ejecutar(api.widgetActivar(id, activo).then(() => api.configLeer().then(setConfig)));
  };

  const boton = (etiqueta: string, icono: ReactNode, accion: () => void, extra: { activo?: boolean; deshabilitado?: boolean; titulo?: string; clave?: string } = {}) => (
    <button
      key={extra.clave}
      type="button"
      className={`gd-dock__boton${extra.activo ? ' gd-dock__boton--activo' : ''}`}
      aria-label={etiqueta}
      aria-pressed={extra.activo}
      title={extra.titulo ?? etiqueta}
      disabled={extra.deshabilitado}
      onClick={accion}
    >
      {icono}
    </button>
  );

  let indiceGoogle = 0;
  const cuentas = resumenConGoogle(resumenCuentas(credenciales, web), cuentasGoogle).map((c) =>
    privacidad && c.clave?.startsWith('google:') ? { ...c, nombre: `Google ${++indiceGoogle}` } : c,
  );
  const conectadas = cuentas.filter((c) => c.estado === 'conectada').length;
  const descripcionCuentas = cuentas.map((c) => `${c.nombre}: ${textoEstadoCuenta(c.estado).toLowerCase()}`).join(', ');
  const porConectar = cuentaPorConectar(credenciales, web);
  const iconoPorConectar = porConectar && cuentas.find((c) => c.id === (porConectar.servicio === 'claude_ai' ? 'claude' : porConectar.servicio))?.icono;

  return (
    <Vidrio elemento="main" lleno className="gd-dock" aria-label="Dock de Glassdeck">
      <span className="gd-dock__asa" data-tauri-drag-region="" aria-hidden="true" />
      <div className="gd-dock__grupo" role="group" aria-label="Widgets">
        {catalogo.map((w) => {
          const activo = config?.widgets[w.id]?.activo ?? false;
          return boton(`${w.titulo}: ${activo ? 'activo' : 'inactivo'}`, ICONOS_WIDGET[w.id] ?? <IconoZonas />, () => alternarWidget(w.id), {
            activo,
            titulo: w.titulo,
            clave: w.id,
          });
        })}
      </div>
      <span className="gd-dock__separador" aria-hidden="true" />
      <div className="gd-dock__grupo" role="group" aria-label="Acciones">
        {boton('Limpiar temporales', <IconoEscoba />, () => ejecutar(api.panelAbrir('limpieza')))}
        {boton('Procesos pesados', <IconoProcesos />, () => ejecutar(api.panelAbrir('procesos')))}
        {boton('Zonas (llega en la fase 4)', <IconoZonas />, () => undefined, { deshabilitado: true })}
        {boton(privacidad ? 'Quitar modo privacidad' : 'Modo privacidad', privacidad ? <IconoOjoTachado /> : <IconoOjo />, () =>
          ejecutar(api.privacidadAlternar()),
          { activo: privacidad },
        )}
        {boton('Customize', <IconoEngrane />, () => ejecutar(api.panelAbrir('customize')))}
        {boton('Ocultar dock (vuelve desde la bandeja)', <IconoOcultar />, () => ejecutar(api.panelCerrar()))}
      </div>
      <span className="gd-dock__separador" aria-hidden="true" />
      <button
        type="button"
        className="gd-dock__cuentas"
        aria-label={`Cuentas, ${conectadas} de ${cuentas.length} conectadas. ${descripcionCuentas}. Abrir Customize, Cuentas`}
        title={descripcionCuentas}
        onClick={() => ejecutar(api.panelAbrir('cuentas'))}
      >
        <span className="gd-dock__avatares" aria-hidden="true">
          {cuentas.map((c) => (
            <span key={c.clave ?? c.id} className="gd-dock__avatar" data-cuenta={c.id}>
              {c.icono}
              <PuntoCuenta estado={c.estado} />
            </span>
          ))}
        </span>
        <span className="gd-dock__cuentas-texto" aria-hidden="true">
          <b className="num">
            {conectadas} de {cuentas.length}
          </b>
          <small>{conectadas === cuentas.length ? 'Conectadas' : 'Conectar'}</small>
        </span>
      </button>
      {porConectar && (
        <button
          type="button"
          className={`gd-dock__boton gd-dock__conectar gd-dock__conectar--${porConectar.estado}`}
          aria-label={`${porConectar.estado === 'reconectar' ? 'Reconectar' : 'Conectar'} ${NOMBRE_SERVICIO[porConectar.servicio]}`}
          title={`${porConectar.estado === 'reconectar' ? 'Reconectar' : 'Conectar'} ${NOMBRE_SERVICIO[porConectar.servicio]}`}
          onClick={() => ejecutar(api.cuentaConectar(porConectar.servicio).then(recargarWeb))}
        >
          {iconoPorConectar}
          <PuntoCuenta estado={porConectar.estado} />
        </button>
      )}
      {error && (
        <span className="gd-dock__error" role="alert" title={error}>
          !
        </span>
      )}
    </Vidrio>
  );
}

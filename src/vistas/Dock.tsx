import { useState, type ReactNode } from 'react';
import { api, mensajeError } from '../api';
import { Vidrio } from '../componentes';
import { useConfig, useEvento } from '../hooks';
import {
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
  IconoZonas,
} from '../iconos';

const WIDGETS: ReadonlyArray<{ id: string; nombre: string; icono: ReactNode }> = [
  { id: 'cpu', nombre: 'CPU', icono: <IconoCpu /> },
  { id: 'ram', nombre: 'Memoria', icono: <IconoRam /> },
  { id: 'disco', nombre: 'Disco', icono: <IconoDisco /> },
  { id: 'red', nombre: 'Red', icono: <IconoRed /> },
  { id: 'procesos', nombre: 'Procesos', icono: <IconoProcesos /> },
];

export function VistaDock() {
  const [config, setConfig] = useConfig();
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

  return (
    <Vidrio elemento="main" lleno className="gd-dock" aria-label="Dock de Glassdeck">
      <span className="gd-dock__asa" data-tauri-drag-region="" aria-hidden="true" />
      <div className="gd-dock__grupo" role="group" aria-label="Widgets">
        {WIDGETS.map((w) =>
          boton(`${w.nombre}: ${config?.widgets[w.id]?.activo ? 'activo' : 'inactivo'}`, w.icono, () => alternarWidget(w.id), {
            activo: config?.widgets[w.id]?.activo ?? false,
            titulo: w.nombre,
            clave: w.id,
          }),
        )}
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
      {error && (
        <span className="gd-dock__error" role="alert" title={error}>
          !
        </span>
      )}
    </Vidrio>
  );
}

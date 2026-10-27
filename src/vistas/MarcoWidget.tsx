import { useEffect, useState, type ReactNode } from 'react';
import { api, escuchar, EVENTOS, mensajeError, type EstadoWidget } from '../api';
import { BarraWidget, Pildora, Vidrio } from '../componentes';
import { useConfig, useEvento } from '../hooks';

export function useMaximizado(): boolean {
  return useEvento<boolean>(EVENTOS.maximizado, false);
}

interface Props {
  id: string;
  titulo: string;
  icono: ReactNode;
  textoPildora: string;
  maximizado: boolean;
  children: ReactNode;
}

export function MarcoWidget({ id, titulo, icono, textoPildora, maximizado, children }: Props) {
  const [config] = useConfig();
  const [estado, setEstado] = useState<EstadoWidget>('normal');
  const [fijado, setFijado] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cfg = config?.widgets[id];

  useEffect(() => {
    if (!cfg) return;
    setEstado(cfg.estado);
    setFijado(cfg.fijado);
  }, [cfg?.estado, cfg?.fijado]);

  useEffect(() => {
    const quitar = escuchar<EstadoWidget>(EVENTOS.estado, setEstado);
    return () => {
      quitar.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!maximizado) return;
    const alTeclear = (e: KeyboardEvent): void => {
      if (e.key === 'Escape' && !document.querySelector('.gd-dialogo-velo')) api.widgetMaximizar(false).catch(() => undefined);
    };
    window.addEventListener('keydown', alTeclear);
    return () => window.removeEventListener('keydown', alTeclear);
  }, [maximizado]);

  const ejecutar = (p: Promise<unknown>): void => {
    p.then(() => setError(null)).catch((e) => setError(mensajeError(e)));
  };

  const cambiarEstado = (nuevo: EstadoWidget): void => {
    setEstado(nuevo);
    ejecutar(api.widgetEstado(nuevo));
  };

  if (estado === 'pildora' && !maximizado) {
    return (
      <Pildora
        icono={icono}
        texto={textoPildora}
        etiqueta={`Restaurar ${titulo}: ${textoPildora}`}
        onRestaurar={() => cambiarEstado('normal')}
      />
    );
  }

  return (
    <Vidrio elemento="section" lleno entrada fijado={fijado} aria-label={titulo} className="gd-widget">
      <BarraWidget
        titulo={titulo}
        icono={icono}
        fijado={fijado}
        maximizado={maximizado}
        onPin={() => {
          setFijado(!fijado);
          ejecutar(api.widgetFijar(!fijado));
        }}
        onMinimizar={() => {
          if (maximizado) ejecutar(api.widgetMaximizar(false).then(() => cambiarEstado('pildora')));
          else cambiarEstado('pildora');
        }}
        onMaximizar={() => ejecutar(api.widgetMaximizar(!maximizado))}
        onCerrar={() => ejecutar(api.widgetCerrar())}
      />
      <div className={`gd-widget__cuerpo${maximizado ? ' gd-widget__cuerpo--detalle' : ''}`}>{children}</div>
      {error && (
        <p className="gd-widget__error" role="alert">
          {error}
        </p>
      )}
    </Vidrio>
  );
}

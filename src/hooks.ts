import { useEffect, useRef, useState } from 'react';
import { api, enTauri, escuchar, EVENTOS, type Config, type DefWidget, type FuenteServicio, type Lectura, type LecturaServicio } from './api';

export function useConfig(): [Config | null, (c: Config) => void] {
  const [config, setConfig] = useState<Config | null>(null);
  useEffect(() => {
    if (!enTauri()) return;
    let vivo = true;
    const cargar = (): void => {
      api.configLeer().then((c) => vivo && setConfig(c)).catch(() => undefined);
    };
    cargar();
    const quitar = escuchar<null>(EVENTOS.config, cargar);
    return () => {
      vivo = false;
      quitar.then((f) => f());
    };
  }, []);
  return [config, setConfig];
}

type Fuente = Lectura['fuente'];
type MapaDatos = { [L in Lectura as L['fuente']]: L['datos'] };
type DatosDe<F extends Fuente> = MapaDatos[F];

export function useLectura<F extends Fuente>(fuente: F, alRecibir?: (d: DatosDe<F>) => void): DatosDe<F> | null {
  const [datos, setDatos] = useState<DatosDe<F> | null>(null);
  const callback = useRef(alRecibir);
  callback.current = alRecibir;
  useEffect(() => {
    const quitar = escuchar<Lectura>(EVENTOS.datos, (l) => {
      if (l.fuente !== fuente) return;
      const d = l.datos as DatosDe<F>;
      callback.current?.(d);
      setDatos(d);
    });
    return () => {
      quitar.then((f) => f());
    };
  }, [fuente]);
  return datos;
}

export type EstadoServicio<F extends FuenteServicio> = Extract<LecturaServicio, { fuente: F }>['datos'];

/** Estado inicial con `servicio_ultimo` y después cada `gd://datos` de esa fuente. */
export function useServicio<F extends FuenteServicio>(fuente: F): EstadoServicio<F> | null {
  const [inicial, setInicial] = useState<EstadoServicio<F> | null>(null);
  const ultimo = useLectura(fuente) as EstadoServicio<F> | null;
  useEffect(() => {
    if (!enTauri()) return;
    let vivo = true;
    api
      .servicioUltimo(fuente)
      .then((e) => vivo && setInicial(() => e as EstadoServicio<F> | null))
      .catch(() => undefined);
    return () => {
      vivo = false;
    };
  }, [fuente]);
  return ultimo ?? inicial;
}

export function useEvento<T>(evento: string, inicial: T): T {
  const [valor, setValor] = useState<T>(inicial);
  useEffect(() => {
    const quitar = escuchar<T>(evento, setValor);
    return () => {
      quitar.then((f) => f());
    };
  }, [evento]);
  return valor;
}

/** Widgets que existen (título, subtítulo); sustituye las listas fijas del dock y de Customize. */
export function useCatalogo(): DefWidget[] {
  const [catalogo, setCatalogo] = useState<DefWidget[]>([]);
  useEffect(() => {
    if (!enTauri()) return;
    let vivo = true;
    api
      .catalogoWidgets()
      .then((c) => vivo && setCatalogo(c))
      .catch(() => undefined);
    return () => {
      vivo = false;
    };
  }, []);
  return catalogo;
}

export function usePrivacidad(config: Config | null): boolean {
  const evento = useEvento<boolean | null>(EVENTOS.privacidad, null);
  return evento ?? config?.privacidad ?? false;
}

/** Reloj para textos como «hace 3 min»; 30 s basta y no despierta la ventana de más. */
export function useAhora(cadaMs = 30_000): number {
  const [ahora, setAhora] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setAhora(Date.now()), cadaMs);
    return () => window.clearInterval(id);
  }, [cadaMs]);
  return ahora;
}

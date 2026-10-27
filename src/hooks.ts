import { useEffect, useRef, useState } from 'react';
import { api, enTauri, escuchar, EVENTOS, type Config, type Lectura } from './api';

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

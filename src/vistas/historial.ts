import { useEffect, useRef, useState } from 'react';
import { api, enTauri, type Config } from '../api';
import { agregarAcotado, puntosPorHora, puntosPorMinuto } from '../formato';
import { useConfig } from '../hooks';

type FuenteHistorial = 'cpu' | 'ram' | 'red' | 'disco';

function intervaloDe(config: Config | null, fuente: FuenteHistorial): number {
  if (!config) return fuente === 'disco' ? 5000 : 2000;
  return fuente === 'disco' ? config.intervalos.disco_ms : config.intervalos.cpu_ram_red_ms;
}

export function usePuntosUltimoMinuto(): number {
  const [config] = useConfig();
  return puntosPorMinuto(intervaloDe(config, 'cpu'));
}

/** Devuelve la última hora de cada serie y cuántos puntos caben en ella con el intervalo configurado. */
export function useHistorial(
  fuente: FuenteHistorial,
  activo: boolean,
): [Record<string, number[]>, (muestras: Record<string, number>) => void, number] {
  const [config] = useConfig();
  const capacidad = puntosPorHora(intervaloDe(config, fuente));
  const [series, setSeries] = useState<Record<string, number[]>>({});
  const activoRef = useRef(activo);
  activoRef.current = activo;
  const capacidadRef = useRef(capacidad);
  capacidadRef.current = capacidad;

  useEffect(() => {
    if (!activo || !enTauri()) return;
    let vivo = true;
    api
      .historial(fuente)
      .then((r) => {
        if (!vivo) return;
        setSeries(Object.fromEntries(r.map((s) => [s.nombre, s.valores])));
      })
      .catch(() => undefined);
    return () => {
      vivo = false;
    };
  }, [fuente, activo]);

  const agregar = (muestras: Record<string, number>): void => {
    if (!activoRef.current) return;
    setSeries((previas) => {
      const siguientes: Record<string, number[]> = { ...previas };
      for (const [nombre, valor] of Object.entries(muestras)) {
        siguientes[nombre] = agregarAcotado(previas[nombre] ?? [], valor, capacidadRef.current);
      }
      return siguientes;
    });
  };

  return [series, agregar, capacidad];
}

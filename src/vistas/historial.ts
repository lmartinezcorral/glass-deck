import { useEffect, useRef, useState } from 'react';
import { api, enTauri } from '../api';
import { puntosPorMinuto } from '../formato';
import { useConfig } from '../hooks';

const PUNTOS_UNA_HORA = 3600;

export function usePuntosUltimoMinuto(): number {
  const [config] = useConfig();
  return puntosPorMinuto(config?.intervalos.cpu_ram_red_ms ?? 2000);
}

export function useHistorial(fuente: string, activo: boolean): [Record<string, number[]>, (muestras: Record<string, number>) => void] {
  const [series, setSeries] = useState<Record<string, number[]>>({});
  const activoRef = useRef(activo);
  activoRef.current = activo;

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
        const lista = previas[nombre] ?? [];
        const nueva = lista.length >= PUNTOS_UNA_HORA ? lista.slice(lista.length - PUNTOS_UNA_HORA + 1) : lista.slice();
        nueva.push(valor);
        siguientes[nombre] = nueva;
      }
      return siguientes;
    });
  };

  return [series, agregar];
}

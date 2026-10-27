import { useId, useMemo } from 'react';

export type VarianteTrazo = 'acento' | 'secundario' | 'coral' | 'ok' | 'aviso' | 'peligro';

export interface SparklineProps {
  datos: readonly number[];
  maxPuntos?: number;
  max?: number | 'auto';
  min?: number;
  alto?: number;
  variante?: VarianteTrazo;
  area?: boolean;
  etiqueta?: string;
  className?: string;
}

const ANCHO_VISTA = 200;

export function useIdSvg(prefijo: string): string {
  return `${prefijo}-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`;
}

export function maximoAuto(series: ReadonlyArray<readonly number[]>, min: number): number {
  let mayor = min;
  for (const datos of series) for (const v of datos) if (v > mayor) mayor = v;
  return mayor > min ? min + (mayor - min) * 1.15 : min + 1;
}

export function Sparkline({
  datos,
  maxPuntos = 60,
  max = 'auto',
  min = 0,
  alto = 38,
  variante = 'acento',
  area = true,
  etiqueta,
  className,
}: SparklineProps) {
  const idArea = useIdSvg('gd-spark');

  const { linea, relleno } = useMemo(() => {
    const ranuras = Math.max(2, Math.floor(maxPuntos));
    const vista = datos.length > ranuras ? datos.slice(datos.length - ranuras) : datos;
    const n = vista.length;
    if (n === 0) return { linea: '', relleno: '' };
    const tope = max === 'auto' ? maximoAuto([vista], min) : max;
    const rango = tope - min || 1;
    const dx = ANCHO_VISTA / (ranuras - 1);
    const desfase = ranuras - n;
    let d = '';
    for (let i = 0; i < n; i++) {
      const v = Math.min(tope, Math.max(min, vista[i] ?? min));
      const x = ((i + desfase) * dx).toFixed(1);
      const y = (alto - 2 - ((v - min) / rango) * (alto - 6)).toFixed(1);
      d += `${i ? 'L' : 'M'}${x},${y}`;
    }
    const xIni = (desfase * dx).toFixed(1);
    return { linea: d, relleno: `${d}L${ANCHO_VISTA},${alto}L${xIni},${alto}Z` };
  }, [datos, maxPuntos, max, min, alto]);

  const accesible = etiqueta
    ? { role: 'img' as const, 'aria-label': etiqueta }
    : { 'aria-hidden': true as const };

  return (
    <svg
      className={['gd-sparkline', `gd-trazo--${variante}`, className].filter(Boolean).join(' ')}
      viewBox={`0 0 ${ANCHO_VISTA} ${alto}`}
      preserveAspectRatio="none"
      style={{ height: alto }}
      focusable="false"
      {...accesible}
    >
      {area && (
        <defs>
          <linearGradient id={idArea} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="currentColor" stopOpacity={0.42} />
            <stop offset="1" stopColor="currentColor" stopOpacity={0} />
          </linearGradient>
        </defs>
      )}
      {area && relleno && <path className="gd-sparkline__area" d={relleno} fill={`url(#${idArea})`} />}
      {linea && <path className="gd-trazo__linea" d={linea} />}
    </svg>
  );
}

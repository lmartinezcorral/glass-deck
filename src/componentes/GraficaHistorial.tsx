import { useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type RefObject } from 'react';
import { maximoAuto, useIdSvg, type VarianteTrazo } from './Sparkline';

export interface SerieHistorial {
  nombre: string;
  datos: readonly number[];
  variante?: VarianteTrazo;
}

export interface GraficaHistorialProps {
  series: readonly SerieHistorial[];
  etiqueta: string;
  max?: number | 'auto';
  min?: number;
  alto?: number;
  etiquetasEje?: readonly string[] | null;
  area?: boolean;
  leyenda?: boolean;
  className?: string;
}

const ALTO_EJE = 18;
const MAX_SERIES = 3;
const VARIANTES: readonly VarianteTrazo[] = ['acento', 'secundario', 'coral'];
export const EJE_UNA_HORA: readonly string[] = ['−60 min', '−45', '−30', '−15', 'ahora'];

export function trazoReducido(
  datos: readonly number[],
  ancho: number,
  alto: number,
  min: number,
  max: number,
): string {
  const n = datos.length;
  if (n === 0 || ancho <= 0 || alto <= 0) return '';
  const rango = max - min || 1;
  const valor = (i: number): number => datos[i] ?? min;
  const y = (v: number): string =>
    (alto - 2 - ((Math.min(max, Math.max(min, v)) - min) / rango) * (alto - 6)).toFixed(1);

  if (n === 1) return `M0,${y(valor(0))}L${ancho},${y(valor(0))}`;

  const cubetas = Math.max(2, Math.floor(ancho));
  let d = '';

  if (n <= cubetas * 2) {
    const dx = ancho / (n - 1);
    for (let i = 0; i < n; i++) d += `${i ? 'L' : 'M'}${(i * dx).toFixed(1)},${y(valor(i))}`;
    return d;
  }

  for (let b = 0; b < cubetas; b++) {
    const ini = Math.floor((b * n) / cubetas);
    const fin = Math.max(ini + 1, Math.floor(((b + 1) * n) / cubetas));
    let iMin = ini;
    let iMax = ini;
    for (let i = ini + 1; i < fin; i++) {
      const v = valor(i);
      if (v < valor(iMin)) iMin = i;
      if (v > valor(iMax)) iMax = i;
    }
    const x = ((b / (cubetas - 1)) * ancho).toFixed(1);
    const primero = Math.min(iMin, iMax);
    const segundo = Math.max(iMin, iMax);
    d += `${b ? 'L' : 'M'}${x},${y(valor(primero))}`;
    if (segundo !== primero) d += `L${x},${y(valor(segundo))}`;
  }
  return d;
}

function useAncho<T extends HTMLElement>(): [RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [ancho, setAncho] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setAncho(Math.round(el.clientWidth));
    const observador = new ResizeObserver((entradas) => {
      const entrada = entradas[0];
      if (!entrada) return;
      const w = Math.round(entrada.contentRect.width);
      setAncho((previo) => (previo === w ? previo : w));
    });
    observador.observe(el);
    return () => observador.disconnect();
  }, []);
  return [ref, ancho];
}

export function GraficaHistorial({
  series,
  etiqueta,
  max = 'auto',
  min = 0,
  alto = 200,
  etiquetasEje = EJE_UNA_HORA,
  area = true,
  leyenda,
  className,
}: GraficaHistorialProps) {
  const [ref, ancho] = useAncho<HTMLDivElement>();
  const prefijo = useIdSvg('gd-grafica');
  const conEje = Boolean(etiquetasEje && etiquetasEje.length);
  const altoTrazo = Math.max(1, alto - (conEje ? ALTO_EJE : 0));
  const mostrarLeyenda = leyenda ?? Math.min(series.length, MAX_SERIES) > 1;

  const trazos = useMemo(() => {
    const visibles = series.slice(0, MAX_SERIES);
    const tope = max === 'auto' ? maximoAuto(visibles.map((s) => s.datos), min) : max;
    return visibles.map((s, i) => {
      const linea = trazoReducido(s.datos, ancho, altoTrazo, min, tope);
      return {
        nombre: s.nombre,
        variante: s.variante ?? VARIANTES[i] ?? 'acento',
        linea,
        relleno: linea ? `${linea}L${ancho},${altoTrazo}L0,${altoTrazo}Z` : '',
      };
    });
  }, [series, ancho, altoTrazo, min, max]);

  return (
    <div className={['gd-grafica-envoltura', className].filter(Boolean).join(' ')}>
      <div ref={ref} className="gd-grafica" style={{ height: alto }} role="img" aria-label={etiqueta}>
        {ancho > 0 && (
          <svg
            className="gd-grafica__lienzo"
            width={ancho}
            height={altoTrazo}
            viewBox={`0 0 ${ancho} ${altoTrazo}`}
            aria-hidden="true"
            focusable="false"
          >
            {area && (
              <defs>
                {trazos.map((t, i) => (
                  <linearGradient key={t.nombre} id={`${prefijo}-${i}`} x1="0" y1="0" x2="0" y2="1" className={`gd-trazo--${t.variante}`}>
                    <stop offset="0" stopColor="currentColor" stopOpacity={i === 0 ? 0.38 : 0.18} />
                    <stop offset="1" stopColor="currentColor" stopOpacity={0} />
                  </linearGradient>
                ))}
              </defs>
            )}
            {trazos.map((t, i) => (
              <g key={t.nombre} className={`gd-trazo--${t.variante}`}>
                {area && t.relleno && <path d={t.relleno} fill={`url(#${prefijo}-${i})`} />}
                {t.linea && <path className="gd-trazo__linea" d={t.linea} />}
              </g>
            ))}
          </svg>
        )}
        {conEje && etiquetasEje && (
          <div className="gd-grafica__eje" aria-hidden="true">
            {etiquetasEje.map((e, i) => (
              <span key={`${e}-${i}`}>{e}</span>
            ))}
          </div>
        )}
      </div>
      {mostrarLeyenda && (
        <ul className="gd-leyenda" aria-label="Series">
          {trazos.map((t) => (
            <li key={t.nombre}>
              <i className={`gd-trazo--${t.variante}`} aria-hidden="true" />
              {t.nombre}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export interface GraficaMultiplesProps {
  series: readonly SerieHistorial[];
  etiqueta: string;
  max?: number | 'auto';
  min?: number;
  altoCelda?: number;
  columnas?: number;
  formatoValor?: (v: number) => string;
  className?: string;
}

export function GraficaMultiples({
  series,
  etiqueta,
  max = 100,
  min = 0,
  altoCelda = 56,
  columnas = 4,
  formatoValor = (v) => `${Math.round(v)} %`,
  className,
}: GraficaMultiplesProps) {
  return (
    <ul
      className={['gd-multiples', className].filter(Boolean).join(' ')}
      style={{ '--gd-multiples-columnas': String(columnas) } as CSSProperties}
      aria-label={etiqueta}
    >
      {series.map((s) => {
        const ultimo = s.datos[s.datos.length - 1];
        return (
          <li key={s.nombre} className="gd-multiples__celda">
            <div className="gd-multiples__cabecera">
              <span>{s.nombre}</span>
              <b className="num">{ultimo === undefined ? '—' : formatoValor(ultimo)}</b>
            </div>
            <GraficaHistorial
              series={[s]}
              etiqueta={`${etiqueta}: ${s.nombre}`}
              max={max}
              min={min}
              alto={altoCelda}
              etiquetasEje={null}
              leyenda={false}
            />
          </li>
        );
      })}
    </ul>
  );
}

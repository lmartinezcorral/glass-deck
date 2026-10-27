import { useEffect, useState, type CSSProperties } from 'react';
import { useIdSvg } from './Sparkline';

export interface UmbralesMedidor {
  aviso: number;
  peligro: number;
}

export type EstadoUmbral = 'normal' | 'aviso' | 'peligro';

export const UMBRALES_PREDETERMINADOS: UmbralesMedidor = { aviso: 70, peligro: 90 };

export function estadoPorUmbral(valor: number, umbrales: UmbralesMedidor | null): EstadoUmbral {
  if (!umbrales) return 'normal';
  if (valor > umbrales.peligro) return 'peligro';
  if (valor > umbrales.aviso) return 'aviso';
  return 'normal';
}

export const limitarPorcentaje = (v: number): number =>
  Number.isFinite(v) ? Math.min(100, Math.max(0, v)) : 0;

export function useMontado(): boolean {
  const [montado, setMontado] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setMontado(true));
    return () => cancelAnimationFrame(id);
  }, []);
  return montado;
}

export interface MedidorProps {
  valor: number;
  etiqueta: string;
  texto?: string;
  subetiqueta?: string;
  textoAccesible?: string;
  tamano?: number;
  grosor?: number;
  umbrales?: UmbralesMedidor | null;
  className?: string;
}

export function Medidor({
  valor,
  etiqueta,
  texto,
  subetiqueta,
  textoAccesible,
  tamano = 92,
  grosor = 9,
  umbrales = UMBRALES_PREDETERMINADOS,
  className,
}: MedidorProps) {
  const idGradiente = useIdSvg('gd-medidor');
  const montado = useMontado();
  const pct = limitarPorcentaje(valor);
  const estado = estadoPorUmbral(pct, umbrales);
  const radio = (tamano - grosor) / 2;
  const circunferencia = 2 * Math.PI * radio;
  const centro = tamano / 2;
  const desplazamiento = circunferencia * (1 - (montado ? pct / 100 : 0));
  const textoCentro = texto ?? String(Math.round(pct));

  return (
    <div
      className={['gd-medidor', `gd-medidor--${estado}`, className].filter(Boolean).join(' ')}
      style={{ width: tamano, height: tamano, '--gd-medidor-tamano': `${tamano}px` } as CSSProperties}
      role="meter"
      aria-label={etiqueta}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pct)}
      aria-valuetext={textoAccesible ?? `${textoCentro}${subetiqueta ? ` ${subetiqueta}` : ''}`}
    >
      <svg width={tamano} height={tamano} viewBox={`0 0 ${tamano} ${tamano}`} aria-hidden="true" focusable="false">
        <defs>
          <linearGradient id={idGradiente} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" className="gd-medidor__parada-1" />
            <stop offset="1" className="gd-medidor__parada-2" />
          </linearGradient>
        </defs>
        <circle className="gd-medidor__pista" cx={centro} cy={centro} r={radio} fill="none" strokeWidth={grosor} />
        <circle
          className="gd-medidor__arco"
          cx={centro}
          cy={centro}
          r={radio}
          fill="none"
          stroke={`url(#${idGradiente})`}
          strokeWidth={grosor}
          strokeLinecap="round"
          strokeDasharray={circunferencia}
          strokeDashoffset={desplazamiento}
          transform={`rotate(-90 ${centro} ${centro})`}
        />
      </svg>
      <div className="gd-medidor__centro" aria-hidden="true">
        <b className="gd-medidor__valor num">{textoCentro}</b>
        {subetiqueta && <small className="gd-medidor__sub">{subetiqueta}</small>}
      </div>
    </div>
  );
}

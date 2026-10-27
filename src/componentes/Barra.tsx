import type { CSSProperties } from 'react';
import {
  estadoPorUmbral,
  limitarPorcentaje,
  UMBRALES_PREDETERMINADOS,
  useMontado,
  type UmbralesMedidor,
} from './Medidor';

export interface BarraProps {
  usado: number;
  total: number;
  etiqueta: string;
  detalle?: string;
  textoValor?: string;
  alto?: number;
  umbrales?: UmbralesMedidor | null;
  ocultarCabecera?: boolean;
  className?: string;
}

export function Barra({
  usado,
  total,
  etiqueta,
  detalle,
  textoValor,
  alto = 7,
  umbrales = UMBRALES_PREDETERMINADOS,
  ocultarCabecera = false,
  className,
}: BarraProps) {
  const montado = useMontado();
  const pct = total > 0 ? limitarPorcentaje((usado / total) * 100) : 0;
  const estado = estadoPorUmbral(pct, umbrales);
  const valor = textoValor ?? `${Math.round(pct)} %`;

  return (
    <div
      className={['gd-uso', `gd-uso--${estado}`, className].filter(Boolean).join(' ')}
      style={{ '--gd-uso-alto': `${alto}px` } as CSSProperties}
    >
      {!ocultarCabecera && (
        <div className="gd-uso__cabecera" aria-hidden="true">
          <b className="gd-uso__etiqueta">{etiqueta}</b>
          {detalle && <span className="gd-uso__detalle">{detalle}</span>}
          <span className="gd-uso__valor num">{valor}</span>
        </div>
      )}
      <div
        className="gd-uso__pista"
        role="meter"
        aria-label={detalle ? `${etiqueta} · ${detalle}` : etiqueta}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(pct)}
        aria-valuetext={valor}
      >
        <i className="gd-uso__relleno" style={{ width: `${montado ? pct : 0}%` }} />
      </div>
    </div>
  );
}

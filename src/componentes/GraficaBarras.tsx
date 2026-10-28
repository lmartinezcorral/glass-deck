export interface GraficaBarrasProps {
  valores: readonly number[];
  etiqueta: string;
  etiquetasEje?: readonly string[];
  alto?: number;
  /** Índice resaltado (por defecto el último, «hoy»). */
  resaltado?: number;
  className?: string;
}

export function GraficaBarras({ valores, etiqueta, etiquetasEje, alto = 160, resaltado, className }: GraficaBarrasProps) {
  const n = valores.length;
  const tope = Math.max(0, ...valores.filter(Number.isFinite));
  const marcado = resaltado ?? n - 1;
  return (
    <div className={['gd-barras', className].filter(Boolean).join(' ')}>
      <svg
        className="gd-barras__lienzo"
        viewBox={`0 0 ${Math.max(1, n) * 10} 100`}
        preserveAspectRatio="none"
        style={{ height: alto }}
        role="img"
        aria-label={etiqueta}
      >
        {valores.map((v, i) => {
          const h = tope > 0 && Number.isFinite(v) ? Math.max(v > 0 ? 1.5 : 0, (v / tope) * 96) : 0;
          return (
            <rect
              key={i}
              className={i === marcado ? 'gd-barras__barra gd-barras__barra--hoy' : 'gd-barras__barra'}
              x={i * 10 + 1.5}
              y={100 - h}
              width={7}
              height={h}
              rx={1.5}
            />
          );
        })}
      </svg>
      {etiquetasEje && etiquetasEje.length > 0 && (
        <div className="gd-grafica__eje gd-barras__eje" aria-hidden="true">
          {etiquetasEje.map((e, i) => (
            <span key={`${e}-${i}`}>{e}</span>
          ))}
        </div>
      )}
    </div>
  );
}

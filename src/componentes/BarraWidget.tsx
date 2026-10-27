import type { ReactNode } from 'react';

export interface BarraWidgetProps {
  titulo: string;
  icono: ReactNode;
  fijado: boolean;
  onPin: () => void;
  onMinimizar: () => void;
  onMaximizar: () => void;
  onCerrar: () => void;
  maximizado?: boolean;
}

const trazo = {
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.8,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
} as const;

function IconoBarra({ children }: { children: ReactNode }) {
  return (
    <svg className="gd-barra__svg" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {...trazo}>
      {children}
    </svg>
  );
}

const ICONO_PIN = (
  <IconoBarra>
    <path d="M9 3h6l-1 6 3.5 3.5h-11L10 9z" />
    <path d="M12 12.5V21" />
  </IconoBarra>
);
const ICONO_MIN = (
  <IconoBarra>
    <rect x="5" y="10.5" width="14" height="3.5" rx="1.75" />
  </IconoBarra>
);
const ICONO_MAX = (
  <IconoBarra>
    <path d="M14 4h6v6M10 20H4v-6M20 4l-7 7M4 20l7-7" />
  </IconoBarra>
);
const ICONO_RESTAURAR = (
  <IconoBarra>
    <path d="M4 14h6v6M20 10h-6V4M10 14l-6 6M14 10l6-6" />
  </IconoBarra>
);
const ICONO_CERRAR = (
  <IconoBarra>
    <path d="M6 6l12 12M18 6 6 18" />
  </IconoBarra>
);

export function BarraWidget({
  titulo,
  icono,
  fijado,
  onPin,
  onMinimizar,
  onMaximizar,
  onCerrar,
  maximizado = false,
}: BarraWidgetProps) {
  return (
    <header className="gd-barra" data-tauri-drag-region="">
      <span className="gd-barra__icono" data-tauri-drag-region="" aria-hidden="true">
        {icono}
      </span>
      <h1 className="gd-barra__titulo" data-tauri-drag-region="">
        {titulo}
      </h1>
      <div className="gd-barra__botones" role="group" aria-label={`Controles de ${titulo}`}>
        <button
          type="button"
          className="gd-barra__boton"
          aria-pressed={fijado}
          aria-label={`Siempre encima: ${titulo}`}
          title={fijado ? 'Quitar siempre encima' : 'Siempre encima'}
          onClick={onPin}
        >
          {ICONO_PIN}
        </button>
        <button
          type="button"
          className="gd-barra__boton"
          aria-label={`Minimizar ${titulo} a píldora`}
          title="Minimizar"
          onClick={onMinimizar}
        >
          {ICONO_MIN}
        </button>
        <button
          type="button"
          className="gd-barra__boton"
          aria-label={maximizado ? `Restaurar ${titulo} a vista normal` : `Vista detallada de ${titulo}`}
          title={maximizado ? 'Restaurar' : 'Maximizar'}
          onClick={onMaximizar}
        >
          {maximizado ? ICONO_RESTAURAR : ICONO_MAX}
        </button>
        <button
          type="button"
          className="gd-barra__boton"
          aria-label={`Cerrar ${titulo} (se reactiva desde el dock)`}
          title="Cerrar"
          onClick={onCerrar}
        >
          {ICONO_CERRAR}
        </button>
      </div>
    </header>
  );
}

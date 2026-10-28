import type { ReactNode, SVGProps } from 'react';
import type { IconoClima as IconoClimaClave } from './api';

function Base({ children, ...props }: SVGProps<SVGSVGElement> & { children: ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      {children}
    </svg>
  );
}

export const IconoCpu = () => (
  <Base>
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
    <path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3" />
  </Base>
);

export const IconoRam = () => (
  <Base>
    <rect x="2.5" y="7" width="19" height="10" rx="1.5" />
    <path d="M6 10v4M10 10v4M14 10v4M18 10v4M5 17v2M19 17v2" />
  </Base>
);

export const IconoDisco = () => (
  <Base>
    <ellipse cx="12" cy="6" rx="8" ry="3" />
    <path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6" />
    <path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" />
  </Base>
);

export const IconoRed = () => (
  <Base>
    <path d="M7 4v14M3.5 14.5 7 18l3.5-3.5" />
    <path d="M17 20V6M13.5 9.5 17 6l3.5 3.5" />
  </Base>
);

export const IconoProcesos = () => (
  <Base>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M7 9h10M7 13h6M7 17h8" />
  </Base>
);

export const IconoEscoba = () => (
  <Base>
    <path d="M14 3 21 10" />
    <path d="M12.5 7.5 16.5 11.5" />
    <path d="M10 10c-3 1-6 4-7 10 6-1 9-4 10-7Z" />
  </Base>
);

export const IconoEngrane = () => (
  <Base>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.6 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
  </Base>
);

export const IconoZonas = () => (
  <Base>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M12 4v16M3 12h9" />
  </Base>
);

export const IconoOjo = () => (
  <Base>
    <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12Z" />
    <circle cx="12" cy="12" r="3" />
  </Base>
);

export const IconoOjoTachado = () => (
  <Base>
    <path d="M10.6 5.1A10 10 0 0 1 12 5c6.4 0 10 7 10 7a17 17 0 0 1-3 3.9M6.6 6.6A17 17 0 0 0 2 12s3.6 7 10 7a10 10 0 0 0 5.4-1.6" />
    <path d="M3 3l18 18M9.9 9.9a3 3 0 0 0 4.2 4.2" />
  </Base>
);

export const IconoCandado = () => (
  <Base>
    <rect x="5" y="11" width="14" height="10" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  </Base>
);

export const IconoEscudo = () => (
  <Base>
    <path d="M12 3 4.5 6v6c0 4.4 3.2 8 7.5 9 4.3-1 7.5-4.6 7.5-9V6Z" />
  </Base>
);

export const IconoOcultar = () => (
  <Base>
    <path d="M6 9l6 6 6-6" />
  </Base>
);

export const IconoCuentas = () => (
  <Base>
    <circle cx="12" cy="8" r="4" />
    <path d="M4 21c1.5-4 4.5-6 8-6s6.5 2 8 6" />
  </Base>
);

export const IconoCartera = () => (
  <Base>
    <path d="M4 7.5A2.5 2.5 0 0 1 6.5 5H18v3" />
    <rect x="3.5" y="8" width="17" height="11" rx="2.5" />
    <path d="M16 13.5h2" />
  </Base>
);

export const IconoSolNube = () => (
  <Base>
    <path d="M8 3v1.5M3.5 8H5M4.8 4.8l1 1M11.2 4.8l-1 1" />
    <path d="M5.6 10.2A3 3 0 1 1 10.7 7" />
    <path d="M9 19h8.5a3.5 3.5 0 0 0 .3-7 5 5 0 0 0-9.6 1.3A2.9 2.9 0 0 0 9 19Z" />
  </Base>
);

export const IconoCursor = () => (
  <Base>
    <path d="M12 2.8 20 7.4v9.2L12 21.2 4 16.6V7.4Z" />
    <path d="M4 7.4 12 12l8-4.6M12 12v9.2" />
  </Base>
);

export const IconoClaude = () => (
  <Base>
    <path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" />
  </Base>
);

export const IconoLlave = () => (
  <Base>
    <circle cx="8" cy="15" r="4" />
    <path d="m11 12 8.5-8.5M16 7l2.5 2.5M14 9l2 2" />
  </Base>
);

const NUBE = 'M7.5 19h9.5a4 4 0 0 0 .4-8 5.5 5.5 0 0 0-10.6 1.4A3.3 3.3 0 0 0 7.5 19Z';
const NUBE_BAJA = 'M7.5 16h9.5a4 4 0 0 0 .4-8 5.5 5.5 0 0 0-10.6 1.4A3.3 3.3 0 0 0 7.5 16Z';

function Astro({ dia, chico }: { dia: boolean; chico?: boolean }) {
  if (chico) {
    return dia ? (
      <g className="gd-clima-icono__sol">
        <circle cx="8" cy="8" r="3" />
        <path d="M8 2.5v1M2.5 8h1M4.1 4.1l.7.7M11.9 4.1l-.7.7" />
      </g>
    ) : (
      <path className="gd-clima-icono__luna" d="M11 9.5A4 4 0 1 1 6.5 4a3.2 3.2 0 0 0 4.5 5.5Z" />
    );
  }
  return dia ? (
    <g className="gd-clima-icono__sol">
      <circle cx="12" cy="12" r="4.2" />
      <path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4" />
    </g>
  ) : (
    <path className="gd-clima-icono__luna" d="M19.5 14.5A8 8 0 1 1 9.5 4.5a6.3 6.3 0 0 0 10 10Z" />
  );
}

export function IconoClima({ icono, dia = true, tamano = 24 }: { icono: IconoClimaClave; dia?: boolean; tamano?: number }) {
  let cuerpo: ReactNode;
  switch (icono) {
    case 'despejado':
      cuerpo = <Astro dia={dia} />;
      break;
    case 'parcial':
      cuerpo = (
        <>
          <Astro dia={dia} chico />
          <path className="gd-clima-icono__nube" d={NUBE} />
        </>
      );
      break;
    case 'nublado':
      cuerpo = <path className="gd-clima-icono__nube" d={NUBE} />;
      break;
    case 'niebla':
      cuerpo = <path className="gd-clima-icono__nube" d="M4 9h16M3 13h18M5 17h14" />;
      break;
    case 'llovizna':
    case 'lluvia':
    case 'chubascos':
      cuerpo = (
        <>
          {icono === 'chubascos' && <Astro dia={dia} chico />}
          <path className="gd-clima-icono__nube" d={NUBE_BAJA} />
          <path
            className="gd-clima-icono__agua"
            d={icono === 'llovizna' ? 'M9 19v.5M13 19v.5M17 19v.5' : 'M9 18.5 8 21.5M13 18.5l-1 3M17 18.5l-1 3'}
          />
        </>
      );
      break;
    case 'nieve':
      cuerpo = (
        <>
          <path className="gd-clima-icono__nube" d={NUBE_BAJA} />
          <path className="gd-clima-icono__agua" d="M9 19.5v.1M12.5 21v.1M16 19.5v.1" strokeWidth={2.6} />
        </>
      );
      break;
    case 'tormenta':
      cuerpo = (
        <>
          <path className="gd-clima-icono__nube" d={NUBE_BAJA} />
          <path className="gd-clima-icono__rayo" d="m13 16-2.5 3.5h3L11 23" />
        </>
      );
      break;
    default:
      cuerpo = (
        <>
          <path className="gd-clima-icono__nube" d={NUBE} />
          <path d="M10.5 12.5a1.6 1.6 0 1 1 2.2 1.5c-.5.2-.7.6-.7 1.1M12 17v.2" />
        </>
      );
  }
  return (
    <Base className="gd-clima-icono" width={tamano} height={tamano}>
      {cuerpo}
    </Base>
  );
}

export const IconoCerrar = () => (
  <Base>
    <path d="M6 6l12 12M18 6 6 18" />
  </Base>
);

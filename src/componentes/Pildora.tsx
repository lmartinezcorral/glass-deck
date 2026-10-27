import type { ReactNode } from 'react';
import { Vidrio } from './Vidrio';

export interface PildoraProps {
  icono: ReactNode;
  texto: string;
  onRestaurar: () => void;
  etiqueta?: string;
  arrastrable?: boolean;
  className?: string;
}

export function Pildora({ icono, texto, onRestaurar, etiqueta, arrastrable = true, className }: PildoraProps) {
  return (
    <Vidrio className={['gd-pildora', className].filter(Boolean).join(' ')}>
      {arrastrable && <span className="gd-pildora__asa" data-tauri-drag-region="" aria-hidden="true" />}
      <button
        type="button"
        className="gd-pildora__boton"
        aria-label={etiqueta ?? `Restaurar widget: ${texto}`}
        title="Restaurar"
        onClick={onRestaurar}
      >
        <span className="gd-pildora__icono" aria-hidden="true">
          {icono}
        </span>
        <span className="gd-pildora__texto num">{texto}</span>
      </button>
    </Vidrio>
  );
}

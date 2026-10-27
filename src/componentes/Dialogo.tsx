import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Vidrio } from './Vidrio';

export interface ElementoDialogo {
  id: string;
  titulo: string;
  icono?: ReactNode;
  insignia?: ReactNode;
  detalle?: string;
}

export interface DialogoProps {
  abierto: boolean;
  titulo: string;
  descripcion?: string;
  icono?: ReactNode;
  elementos?: readonly ElementoDialogo[];
  aviso?: ReactNode;
  confirmacionEscrita?: string;
  textoConfirmar?: string;
  textoCancelar?: string;
  peligro?: boolean;
  onConfirmar: () => void;
  onCancelar: () => void;
  children?: ReactNode;
}

const SELECTOR_ENFOCABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

const trazo = {
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.8,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
} as const;

const ICONO_AVISO = (
  <svg className="gd-icono" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {...trazo}>
    <path d="M12 3.5 22 20.5H2z" />
    <path d="M12 10v4.5M12 17.5v.3" />
  </svg>
);
const ICONO_INFO = (
  <svg className="gd-icono" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {...trazo}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 7.5v.3" />
  </svg>
);
const ICONO_CERRAR = (
  <svg className="gd-icono" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {...trazo}>
    <path d="M6 6l12 12M18 6 6 18" />
  </svg>
);

export function Dialogo({
  abierto,
  titulo,
  descripcion,
  icono,
  elementos,
  aviso,
  confirmacionEscrita,
  textoConfirmar = 'Confirmar',
  textoCancelar = 'Cancelar',
  peligro = false,
  onConfirmar,
  onCancelar,
  children,
}: DialogoProps) {
  const panel = useRef<HTMLElement | null>(null);
  const cancelar = useRef(onCancelar);
  const [escrito, setEscrito] = useState('');
  const base = useId();
  const idTitulo = `${base}-titulo`;
  const idDescripcion = `${base}-descripcion`;

  cancelar.current = onCancelar;

  const habilitado =
    !confirmacionEscrita || escrito.trim().toUpperCase() === confirmacionEscrita.trim().toUpperCase();

  useEffect(() => {
    if (!abierto) return;
    const previo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setEscrito('');

    const enfocables = (): HTMLElement[] =>
      Array.from(panel.current?.querySelectorAll<HTMLElement>(SELECTOR_ENFOCABLE) ?? []).filter(
        (el) => el.getClientRects().length > 0,
      );

    const cuadro = requestAnimationFrame(() => {
      const inicial = panel.current?.querySelector<HTMLElement>('[data-foco-inicial]');
      (inicial ?? panel.current)?.focus();
    });

    const manejarTeclas = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        cancelar.current();
        return;
      }
      if (e.key !== 'Tab') return;
      const lista = enfocables();
      const primero = lista[0];
      const ultimo = lista[lista.length - 1];
      if (!primero || !ultimo) {
        e.preventDefault();
        return;
      }
      const activo = document.activeElement;
      const dentro = activo instanceof Node && panel.current?.contains(activo);
      if (!dentro) {
        e.preventDefault();
        primero.focus();
      } else if (e.shiftKey && (activo === primero || activo === panel.current)) {
        e.preventDefault();
        ultimo.focus();
      } else if (!e.shiftKey && activo === ultimo) {
        e.preventDefault();
        primero.focus();
      }
    };

    document.addEventListener('keydown', manejarTeclas, true);
    return () => {
      cancelAnimationFrame(cuadro);
      document.removeEventListener('keydown', manejarTeclas, true);
      previo?.focus();
    };
  }, [abierto]);

  if (!abierto) return null;

  const focoEnCancelar = !confirmacionEscrita && peligro;

  return (
    <div className="gd-dialogo-velo">
      <Vidrio
        ref={panel}
        elemento="section"
        solido
        reflejo={false}
        className={['gd-dialogo', peligro && 'gd-dialogo--peligro'].filter(Boolean).join(' ')}
        role={peligro ? 'alertdialog' : 'dialog'}
        aria-modal="true"
        aria-labelledby={idTitulo}
        aria-describedby={descripcion ? idDescripcion : undefined}
        tabIndex={-1}
      >
        <header className="gd-dialogo__cabecera">
          <span className="gd-dialogo__icono" aria-hidden="true">
            {icono ?? (peligro ? ICONO_AVISO : ICONO_INFO)}
          </span>
          <div className="gd-dialogo__titulos">
            <h2 id={idTitulo}>{titulo}</h2>
            {descripcion && <p id={idDescripcion}>{descripcion}</p>}
          </div>
          <button type="button" className="gd-boton-icono gd-dialogo__cerrar" aria-label="Cancelar y cerrar diálogo" onClick={onCancelar}>
            {ICONO_CERRAR}
          </button>
        </header>

        {elementos && elementos.length > 0 && (
          <ul className="gd-dialogo__lista" aria-label="Elementos afectados">
            {elementos.map((el) => (
              <li key={el.id} className="gd-dialogo__elemento">
                <span className="gd-dialogo__elemento-icono" aria-hidden="true">
                  {el.icono}
                </span>
                <b>{el.titulo}</b>
                <span>{el.insignia}</span>
                <span className="gd-etiqueta num">{el.detalle}</span>
              </li>
            ))}
          </ul>
        )}

        {children}

        {aviso && (
          <div className="gd-aviso" role="note">
            {ICONO_AVISO}
            <div>{aviso}</div>
          </div>
        )}

        {confirmacionEscrita && (
          <input
            className="gd-campo gd-dialogo__escrito"
            value={escrito}
            onChange={(e) => setEscrito(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && habilitado) onConfirmar();
            }}
            placeholder={confirmacionEscrita}
            aria-label={`Escribe ${confirmacionEscrita} para confirmar`}
            autoComplete="off"
            spellCheck={false}
            data-foco-inicial=""
          />
        )}

        <footer className="gd-dialogo__acciones">
          <button type="button" className="gd-boton" onClick={onCancelar} data-foco-inicial={focoEnCancelar ? '' : undefined}>
            {textoCancelar}
          </button>
          <button
            type="button"
            className={['gd-boton', peligro ? 'gd-boton--peligro' : 'gd-boton--primario'].join(' ')}
            disabled={!habilitado}
            onClick={onConfirmar}
            data-foco-inicial={!confirmacionEscrita && !peligro ? '' : undefined}
          >
            {textoConfirmar}
          </button>
        </footer>
      </Vidrio>
    </div>
  );
}

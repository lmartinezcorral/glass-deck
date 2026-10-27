import { useRef, type KeyboardEvent } from 'react';

export interface OpcionSelector<T extends string> {
  valor: T;
  etiqueta: string;
  deshabilitada?: boolean;
}

export interface SelectorProps<T extends string> {
  opciones: readonly OpcionSelector<T>[];
  valor: T;
  onCambiar: (valor: T) => void;
  etiqueta: string;
  className?: string;
}

export function Selector<T extends string>({ opciones, valor, onCambiar, etiqueta, className }: SelectorProps<T>) {
  const grupo = useRef<HTMLDivElement>(null);

  const mover = (e: KeyboardEvent<HTMLDivElement>): void => {
    const paso = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!paso) return;
    const botones = Array.from(grupo.current?.querySelectorAll<HTMLButtonElement>('button:not([disabled])') ?? []);
    const actual = botones.indexOf(document.activeElement as HTMLButtonElement);
    if (actual < 0 || botones.length === 0) return;
    e.preventDefault();
    botones[(actual + paso + botones.length) % botones.length]?.focus();
  };

  return (
    <div
      ref={grupo}
      className={['gd-selector', className].filter(Boolean).join(' ')}
      role="group"
      aria-label={etiqueta}
      onKeyDown={mover}
    >
      {opciones.map((op) => (
        <button
          key={op.valor}
          type="button"
          className="gd-selector__opcion"
          aria-pressed={op.valor === valor}
          disabled={op.deshabilitada}
          onClick={() => onCambiar(op.valor)}
        >
          {op.etiqueta}
        </button>
      ))}
    </div>
  );
}

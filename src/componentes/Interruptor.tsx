import { useId } from 'react';

export interface InterruptorProps {
  activo: boolean;
  onCambiar: (activo: boolean) => void;
  etiqueta: string;
  descripcion?: string;
  deshabilitado?: boolean;
  ocultarEtiqueta?: boolean;
  className?: string;
}

export function Interruptor({
  activo,
  onCambiar,
  etiqueta,
  descripcion,
  deshabilitado = false,
  ocultarEtiqueta = false,
  className,
}: InterruptorProps) {
  const base = useId();
  const idEtiqueta = `${base}-etiqueta`;
  const idDescripcion = `${base}-descripcion`;

  const control = (
    <button
      type="button"
      role="switch"
      className="gd-interruptor__control"
      aria-checked={activo}
      aria-label={ocultarEtiqueta ? etiqueta : undefined}
      aria-labelledby={ocultarEtiqueta ? undefined : idEtiqueta}
      aria-describedby={!ocultarEtiqueta && descripcion ? idDescripcion : undefined}
      disabled={deshabilitado}
      onClick={() => onCambiar(!activo)}
    />
  );

  if (ocultarEtiqueta) return control;

  return (
    <div className={['gd-interruptor', deshabilitado && 'gd-interruptor--deshabilitado', className].filter(Boolean).join(' ')}>
      <div className="gd-interruptor__texto">
        <span id={idEtiqueta}>{etiqueta}</span>
        {descripcion && <small id={idDescripcion}>{descripcion}</small>}
      </div>
      {control}
    </div>
  );
}

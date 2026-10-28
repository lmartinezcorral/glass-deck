import type { EstadoFuente } from '../api';
import { hace, horaCorta } from '../formato';

/** Hora de la última lectura o el motivo del fallo con el último dato válido (GD-CUR-06, GD-WX-03); nunca inventa datos. */
export function EstadoLectura({ estado, ahora }: { estado: EstadoFuente<unknown> | null; ahora: number }) {
  if (!estado) {
    return (
      <p className="gd-lectura gd-etiqueta" role="status">
        Esperando la primera lectura…
      </p>
    );
  }
  const { hora_ms, error, texto_error, proximo_intento_ms } = estado;
  if (error) {
    return (
      <p className="gd-lectura gd-lectura--error gd-etiqueta" role="status">
        <span className="gd-chip gd-chip--aviso">{texto_error ?? 'Error'}</span>
        <span className="num">
          {hora_ms ? `Último dato válido ${hace(hora_ms, ahora)} (${horaCorta(hora_ms)})` : 'Sin datos válidos todavía'}
          {proximo_intento_ms ? ` · reintento a las ${horaCorta(proximo_intento_ms)}` : ''}
        </span>
      </p>
    );
  }
  return (
    <p className="gd-lectura gd-etiqueta num" role="status">
      {hora_ms ? `Leído ${hace(hora_ms, ahora)} · ${horaCorta(hora_ms)}` : 'Esperando la primera lectura…'}
    </p>
  );
}

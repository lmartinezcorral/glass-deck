import { useState } from 'react';
import { EJE_UNA_HORA, GraficaHistorial, Sparkline } from '../componentes';
import { agregarAcotado, velocidad } from '../formato';
import { useLectura } from '../hooks';
import { IconoRed } from '../iconos';
import { useHistorial } from './historial';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

const PUNTOS_SPARK = 60;

export function VistaRed() {
  const maximizado = useMaximizado();
  const [bajadas, setBajadas] = useState<number[]>([]);
  const [subidas, setSubidas] = useState<number[]>([]);
  const [series, agregar] = useHistorial('red', maximizado);
  const r = useLectura('red', (d) => {
    setBajadas((l) => agregarAcotado(l, d.bajada_bps, PUNTOS_SPARK));
    setSubidas((l) => agregarAcotado(l, d.subida_bps, PUNTOS_SPARK));
    agregar({ bajada: d.bajada_bps / 1024, subida: d.subida_bps / 1024 });
  });

  return (
    <MarcoWidget
      id="red"
      titulo="Red"
      icono={<IconoRed />}
      textoPildora={r ? `↓ ${velocidad(r.bajada_bps)}` : '—'}
      maximizado={maximizado}
    >
      <div className={`gd-vista ${maximizado ? 'gd-vista--detalle' : 'gd-vista--pila'}`}>
        <dl className="gd-kv gd-vista__pares">
          <div>
            <dt>Bajada</dt>
            <dd>{r ? velocidad(r.bajada_bps) : '—'}</dd>
          </div>
          <div>
            <dt>Subida</dt>
            <dd>{r ? velocidad(r.subida_bps) : '—'}</dd>
          </div>
        </dl>
        {maximizado ? (
          <GraficaHistorial
            series={[
              { nombre: 'Bajada KB/s', datos: series.bajada ?? [], variante: 'acento' },
              { nombre: 'Subida KB/s', datos: series.subida ?? [], variante: 'coral' },
            ]}
            etiqueta="Tráfico de red, última hora"
            etiquetasEje={EJE_UNA_HORA}
          />
        ) : (
          <div className="gd-vista__doble">
            <Sparkline datos={bajadas} maxPuntos={PUNTOS_SPARK} etiqueta="Bajada, último minuto" />
            <Sparkline datos={subidas} maxPuntos={PUNTOS_SPARK} variante="coral" etiqueta="Subida, último minuto" />
          </div>
        )}
        <span className="gd-etiqueta gd-privado">
          {r ? [r.adaptador, r.tipo, r.ip].filter(Boolean).join(' · ') : 'Buscando adaptador…'}
        </span>
      </div>
    </MarcoWidget>
  );
}

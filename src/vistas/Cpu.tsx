import { useState } from 'react';
import { EJE_UNA_HORA, GraficaHistorial, GraficaMultiples, Medidor, Sparkline } from '../componentes';
import { agregarAcotado, porcentaje } from '../formato';
import { useLectura } from '../hooks';
import { IconoCpu } from '../iconos';
import { useHistorial } from './historial';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

const PUNTOS_SPARK = 60;

export function VistaCpu() {
  const maximizado = useMaximizado();
  const [reciente, setReciente] = useState<number[]>([]);
  const [series, agregar] = useHistorial('cpu', maximizado);
  const muestra = useLectura('cpu', (m) => {
    setReciente((r) => agregarAcotado(r, m.total, PUNTOS_SPARK));
    agregar({ total: m.total, ...Object.fromEntries(m.nucleos.map((v, i) => [`n${i}`, v])) });
  });
  const total = muestra?.total ?? 0;

  const nucleos = Object.keys(series)
    .filter((k) => /^n\d+$/.test(k))
    .sort((a, b) => Number(a.slice(1)) - Number(b.slice(1)))
    .map((k) => ({ nombre: `Núcleo ${Number(k.slice(1)) + 1}`, datos: series[k] ?? [] }));

  return (
    <MarcoWidget
      id="cpu"
      titulo="CPU"
      subtitulo={maximizado ? (muestra?.nombre ?? 'Procesador') : 'Uso total del procesador'}
      icono={<IconoCpu />}
      textoPildora={porcentaje(total)}
      maximizado={maximizado}
    >
      {maximizado ? (
        <div className="gd-vista gd-vista--detalle">
          <GraficaHistorial
            series={[{ nombre: 'Total', datos: series.total ?? [] }]}
            etiqueta="Uso total de CPU, última hora"
            max={100}
            etiquetasEje={EJE_UNA_HORA}
          />
          <GraficaMultiples series={nucleos} etiqueta="Uso por núcleo" columnas={4} />
        </div>
      ) : (
        <div className="gd-vista gd-vista--fila">
          <Medidor valor={total} etiqueta="Uso de CPU" subetiqueta="%" />
          <div className="gd-vista__columna">
            <span className="gd-etiqueta" title={muestra?.nombre}>
              {muestra ? `${muestra.nucleos.length} núcleos lógicos` : 'Midiendo…'}
            </span>
            <Sparkline datos={reciente} maxPuntos={PUNTOS_SPARK} max={100} etiqueta="CPU, último minuto" />
            <span className="gd-etiqueta">Último minuto</span>
          </div>
        </div>
      )}
    </MarcoWidget>
  );
}

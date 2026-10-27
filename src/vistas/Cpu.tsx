import { useState } from 'react';
import { EJE_UNA_HORA, GraficaHistorial, GraficaMultiples, Medidor, Sparkline } from '../componentes';
import { agregarAcotado, porcentaje } from '../formato';
import { useLectura } from '../hooks';
import { IconoCpu } from '../iconos';
import { useHistorial, usePuntosUltimoMinuto } from './historial';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

export function VistaCpu() {
  const maximizado = useMaximizado();
  const puntos = usePuntosUltimoMinuto();
  const [reciente, setReciente] = useState<number[]>([]);
  const [series, agregar] = useHistorial('cpu', maximizado);
  const muestra = useLectura('cpu', (m) => {
    setReciente((r) => agregarAcotado(r, m.total, puntos));
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
        <div className="gd-vista gd-vista--pila">
          <div className="gd-vista__cabecera">
            <Medidor valor={total} etiqueta="Uso de CPU" subetiqueta="%" tamano={68} />
            <dl className="gd-kv gd-vista__columna">
              <div>
                <dt>Núcleos</dt>
                <dd>{muestra ? muestra.nucleos.length : '—'}</dd>
              </div>
              <div>
                <dt>Núcleo más alto</dt>
                <dd>{muestra && muestra.nucleos.length > 0 ? porcentaje(Math.max(...muestra.nucleos)) : '—'}</dd>
              </div>
            </dl>
          </div>
          <div className="gd-vista__unidad">
            <Sparkline datos={reciente} maxPuntos={puntos} max={100} alto={30} etiqueta="CPU, último minuto" />
            <span className="gd-etiqueta">Último minuto</span>
          </div>
        </div>
      )}
    </MarcoWidget>
  );
}

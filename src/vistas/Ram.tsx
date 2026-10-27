import { Barra, EJE_UNA_HORA, GraficaHistorial, Medidor } from '../componentes';
import { bytes, gb, porcentaje } from '../formato';
import { useLectura } from '../hooks';
import { IconoRam } from '../iconos';
import { useHistorial } from './historial';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

export function VistaRam() {
  const maximizado = useMaximizado();
  const [series, agregar] = useHistorial('ram', maximizado);
  const m = useLectura('ram', (d) => {
    if (d.total > 0) agregar({ usada: (d.usada * 100) / d.total });
  });
  const pct = m && m.total > 0 ? (m.usada * 100) / m.total : 0;

  const desglose = m && (
    <div className="gd-vista__columna">
      <Barra usado={m.usada} total={m.total} etiqueta="En uso" textoValor={bytes(m.usada)} />
      <Barra usado={m.en_cache} total={m.total} etiqueta="En caché" textoValor={bytes(m.en_cache)} umbrales={null} />
      <Barra usado={m.libre} total={m.total} etiqueta="Libre" textoValor={bytes(m.libre)} umbrales={null} />
      {maximizado && m.limite_comprometida > 0 && (
        <Barra
          usado={m.comprometida}
          total={m.limite_comprometida}
          etiqueta="Comprometida"
          detalle={`de ${bytes(m.limite_comprometida)}`}
          textoValor={bytes(m.comprometida)}
        />
      )}
    </div>
  );

  return (
    <MarcoWidget
      id="ram"
      titulo="Memoria"
      subtitulo={maximizado ? 'Memoria RAM en la última hora' : 'Memoria RAM en uso (GB)'}
      icono={<IconoRam />}
      textoPildora={porcentaje(pct)}
      maximizado={maximizado}
    >
      {maximizado ? (
        <div className="gd-vista gd-vista--detalle">
          <GraficaHistorial
            series={[{ nombre: 'En uso', datos: series.usada ?? [] }]}
            etiqueta="Memoria en uso, última hora"
            max={100}
            etiquetasEje={EJE_UNA_HORA}
          />
          {desglose}
        </div>
      ) : (
        <div className="gd-vista gd-vista--fila">
          <Medidor
            valor={pct}
            etiqueta="Memoria en uso"
            texto={m ? gb(m.usada) : '—'}
            subetiqueta={m ? `de ${gb(m.total)} GB` : 'GB'}
            textoAccesible={m ? `${bytes(m.usada)} de ${bytes(m.total)}, ${porcentaje(pct)}` : 'Midiendo'}
          />
          {desglose ?? <span className="gd-etiqueta">Midiendo…</span>}
        </div>
      )}
    </MarcoWidget>
  );
}

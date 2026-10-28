import { Barra, EJE_UNA_HORA, GraficaHistorial } from '../componentes';
import { bytes, porcentaje, velocidad } from '../formato';
import { useLectura } from '../hooks';
import { IconoDisco } from '../iconos';
import { useHistorial } from './historial';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

const MB = 1_048_576;

export function VistaDisco() {
  const maximizado = useMaximizado('disco');
  const [series, agregar, capacidad] = useHistorial('disco', maximizado);
  const unidades = useLectura('disco', (lista) => {
    agregar(
      Object.fromEntries(
        lista.flatMap((u) => [
          [`${u.unidad} lectura`, u.lectura_bps / MB],
          [`${u.unidad} escritura`, u.escritura_bps / MB],
        ]),
      ),
    );
  });
  const principal = unidades?.[0];
  const usoPrincipal = principal && principal.total > 0 ? ((principal.total - principal.libre) * 100) / principal.total : 0;

  return (
    <MarcoWidget
      id="disco"
      titulo="Disco"
      subtitulo="Espacio usado y actividad por unidad"
      icono={<IconoDisco />}
      textoPildora={principal ? `${principal.unidad} ${porcentaje(usoPrincipal)}` : '—'}
      maximizado={maximizado}
    >
      {!unidades ? (
        <span className="gd-etiqueta">Midiendo…</span>
      ) : (
        <div className={`gd-vista ${maximizado ? 'gd-vista--detalle' : 'gd-vista--pila'}`}>
          {unidades.map((u) => (
            <div key={u.unidad} className="gd-vista__unidad">
              <Barra
                usado={u.total - u.libre}
                total={u.total}
                etiqueta={u.unidad}
                detalle={`${u.etiqueta || 'Unidad local'}${u.extraible ? ' · USB' : ''} · ${bytes(u.libre)} libres`}
              />
              <span className="gd-etiqueta num">
                Lectura {velocidad(u.lectura_bps)} · Escritura {velocidad(u.escritura_bps)}
              </span>
              {maximizado && (
                <GraficaHistorial
                  capacidad={capacidad}
                  series={[
                    { nombre: 'Lectura MB/s', datos: series[`${u.unidad} lectura`] ?? [], variante: 'acento' },
                    { nombre: 'Escritura MB/s', datos: series[`${u.unidad} escritura`] ?? [], variante: 'coral' },
                  ]}
                  etiqueta={`Actividad de ${u.unidad}, última hora`}
                  alto={120}
                  etiquetasEje={EJE_UNA_HORA}
                />
              )}
            </div>
          ))}
        </div>
      )}
    </MarcoWidget>
  );
}

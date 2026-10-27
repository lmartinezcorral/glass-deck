import { useMemo, useState } from 'react';
import { api, mensajeError, type FilaProceso, type ResultadoProceso } from '../api';
import { Dialogo, Selector } from '../componentes';
import { bytes, porcentaje } from '../formato';
import { useLectura } from '../hooks';
import { IconoCandado, IconoProcesos } from '../iconos';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

type Orden = 'cpu' | 'ram' | 'nombre';
const CONFIRMACION = 'CERRAR';
const TOP = 5;

const TEXTO_RESULTADO: Record<ResultadoProceso['resultado'], string> = {
  cerrado: 'Cerrado',
  rechazado: 'No permitido',
  sin_permiso: 'Sin permiso',
  ya_no_existia: 'Ya no existía',
};

const TEXTO_NIVEL: Record<FilaProceso['nivel'], string> = {
  protegido: 'Protegido: Glassdeck nunca lo cierra',
  reforzado: 'Importante: pide escribir CERRAR',
  normal: '',
};

function ordenar(filas: readonly FilaProceso[], orden: Orden, asc: boolean): FilaProceso[] {
  const copia = filas.slice();
  copia.sort((a, b) => {
    const d = orden === 'nombre' ? a.nombre.localeCompare(b.nombre) : orden === 'cpu' ? a.cpu - b.cpu : a.ram - b.ram;
    return asc ? d : -d;
  });
  return copia;
}

export function VistaProcesos() {
  const maximizado = useMaximizado();
  const filas = useLectura('procesos') ?? [];
  const [orden, setOrden] = useState<Orden>('cpu');
  const [asc, setAsc] = useState(false);
  const [busqueda, setBusqueda] = useState('');
  const [marcados, setMarcados] = useState<Map<number, FilaProceso>>(new Map());
  const [dialogo, setDialogo] = useState(false);
  const [resultados, setResultados] = useState<ResultadoProceso[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const visibles = useMemo(() => {
    const q = busqueda.trim().toLowerCase();
    const filtradas = q ? filas.filter((f) => f.nombre.toLowerCase().includes(q) || String(f.pid) === q) : filas;
    return ordenar(filtradas, maximizado ? orden : orden === 'nombre' ? 'cpu' : orden, maximizado ? asc : false);
  }, [filas, busqueda, orden, asc, maximizado]);

  const top = visibles.slice(0, TOP);
  const pesado = top[0];
  const seleccion = [...marcados.values()];
  const requiereEscribir = seleccion.some((f) => f.nivel === 'reforzado');
  const sugerirExplorer = seleccion.some((f) => f.reiniciar_explorer);

  const alternar = (f: FilaProceso): void => {
    if (f.nivel === 'protegido') return;
    setMarcados((m) => {
      const n = new Map(m);
      if (n.has(f.pid)) n.delete(f.pid);
      else n.set(f.pid, f);
      return n;
    });
  };

  const cambiarOrden = (o: Orden): void => {
    if (o === orden) setAsc(!asc);
    else {
      setOrden(o);
      setAsc(o === 'nombre');
    }
  };

  const confirmar = (): void => {
    setDialogo(false);
    setError(null);
    api
      .procesosCerrar(
        seleccion.map((f) => f.pid),
        requiereEscribir ? CONFIRMACION : null,
      )
      .then((r) => {
        setResultados(r);
        setMarcados(new Map());
      })
      .catch((e) => setError(mensajeError(e)));
  };

  const encabezado = (o: Orden, texto: string) => (
    <th scope="col" aria-sort={orden === o ? (asc ? 'ascending' : 'descending') : 'none'}>
      <button type="button" className="gd-tabla__orden" onClick={() => cambiarOrden(o)}>
        {texto}
        {orden === o ? (asc ? ' ▲' : ' ▼') : ''}
      </button>
    </th>
  );

  return (
    <MarcoWidget
      id="procesos"
      titulo="Procesos"
      subtitulo={maximizado ? 'Todos los procesos: marca los que quieras cerrar' : `Los ${TOP} que más consumen`}
      icono={<IconoProcesos />}
      textoPildora={pesado ? `${pesado.nombre.replace(/\.exe$/i, '')} ${porcentaje(pesado.cpu)}` : '—'}
      maximizado={maximizado}
    >
      {!maximizado ? (
        <div className="gd-vista gd-vista--pila">
          <Selector
            etiqueta="Ordenar por"
            opciones={[
              { valor: 'cpu', etiqueta: 'CPU' },
              { valor: 'ram', etiqueta: 'RAM' },
            ]}
            valor={orden === 'ram' ? 'ram' : 'cpu'}
            onCambiar={(v) => setOrden(v)}
          />
          <ol className="gd-lista-procesos" aria-label="Procesos que más consumen">
            {top.map((f) => (
              <li key={f.pid}>
                <span className="gd-privado" title={f.nombre}>
                  {f.nombre}
                </span>
                <b className="num">{orden === 'ram' ? bytes(f.ram) : porcentaje(f.cpu)}</b>
              </li>
            ))}
            {top.length === 0 && <li className="gd-etiqueta">Midiendo…</li>}
          </ol>
        </div>
      ) : (
        <div className="gd-vista gd-vista--detalle">
          <div className="gd-vista__herramientas">
            <input
              className="gd-campo"
              type="search"
              placeholder="Buscar por nombre o PID"
              aria-label="Buscar proceso"
              value={busqueda}
              onChange={(e) => setBusqueda(e.target.value)}
            />
            <button type="button" className="gd-boton gd-boton--sm" onClick={() => api.abrirAdministradorTareas().catch(() => undefined)}>
              Administrador de tareas
            </button>
            <button
              type="button"
              className="gd-boton gd-boton--peligro gd-boton--sm"
              disabled={seleccion.length === 0}
              onClick={() => {
                setResultados(null);
                setDialogo(true);
              }}
            >
              Cerrar {seleccion.length > 0 ? `(${seleccion.length})` : ''}
            </button>
          </div>
          <div className="gd-tabla-contenedor">
            <table className="gd-tabla">
              <thead>
                <tr>
                  <th scope="col">
                    <span className="gd-solo-lector">Seleccionar</span>
                  </th>
                  {encabezado('nombre', 'Nombre')}
                  <th scope="col">PID</th>
                  {encabezado('cpu', 'CPU')}
                  {encabezado('ram', 'RAM')}
                </tr>
              </thead>
              <tbody>
                {visibles.map((f) => (
                  <tr key={f.pid} className={marcados.has(f.pid) ? 'gd-tabla__fila--marcada' : undefined}>
                    <td>
                      {f.nivel === 'protegido' ? (
                        <span className="gd-tabla__candado" title={TEXTO_NIVEL.protegido} aria-label={TEXTO_NIVEL.protegido}>
                          <IconoCandado />
                        </span>
                      ) : (
                        <input
                          type="checkbox"
                          checked={marcados.has(f.pid)}
                          onChange={() => alternar(f)}
                          aria-label={`Seleccionar ${f.nombre}`}
                        />
                      )}
                    </td>
                    <td className="gd-privado" title={f.nombre}>
                      {f.nombre}
                      {f.nivel === 'reforzado' && (
                        <span className="gd-chip gd-chip--aviso" title={TEXTO_NIVEL.reforzado}>
                          importante
                        </span>
                      )}
                    </td>
                    <td className="num">{f.pid}</td>
                    <td className="num">{porcentaje(f.cpu)}</td>
                    <td className="num">{bytes(f.ram)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {error && (
            <p className="gd-widget__error" role="alert">
              {error}
            </p>
          )}
          {resultados && (
            <ul className="gd-resultados" aria-label="Resultado del cierre">
              {resultados.map((r) => (
                <li key={r.pid}>
                  <span className="gd-privado">{r.nombre}</span>
                  <span className={`gd-chip ${r.resultado === 'cerrado' ? 'gd-chip--ok' : 'gd-chip--aviso'}`}>
                    {TEXTO_RESULTADO[r.resultado]}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      <Dialogo
        abierto={dialogo}
        peligro
        titulo={`¿Cerrar ${seleccion.length} ${seleccion.length === 1 ? 'proceso' : 'procesos'}?`}
        descripcion="Primero se les pide cerrar normalmente; si no responden en 5 segundos se fuerzan."
        elementos={seleccion.map((f) => ({
          id: String(f.pid),
          titulo: f.nombre,
          detalle: `PID ${f.pid} · ${bytes(f.ram)}`,
          insignia: f.nivel === 'reforzado' ? <span className="gd-chip gd-chip--aviso">importante</span> : undefined,
        }))}
        aviso={
          <>
            Puedes perder trabajo sin guardar.
            {requiereEscribir && ' Incluye programas importantes: escribe CERRAR para continuar.'}
            {sugerirExplorer && (
              <>
                {' '}
                Para el Explorador de Windows es mejor{' '}
                <button
                  type="button"
                  className="gd-enlace"
                  onClick={() => {
                    setDialogo(false);
                    api.reiniciarExplorer().catch((e) => setError(mensajeError(e)));
                  }}
                >
                  reiniciarlo
                </button>
                .
              </>
            )}
          </>
        }
        confirmacionEscrita={requiereEscribir ? CONFIRMACION : undefined}
        textoConfirmar="Cerrar"
        onConfirmar={confirmar}
        onCancelar={() => setDialogo(false)}
      />
    </MarcoWidget>
  );
}

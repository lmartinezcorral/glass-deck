import { useEffect, useState } from 'react';
import { api, enTauri, mensajeError, type OpcionesLimpieza, type OrigenLimpieza, type ResumenLimpieza } from '../api';
import { Dialogo, Vidrio } from '../componentes';
import { bytes } from '../formato';
import { IconoEscoba, IconoEscudo } from '../iconos';

type Fase = 'cargando' | 'lista' | 'limpiando' | 'hecho';

export function VistaLimpieza() {
  const [origenes, setOrigenes] = useState<OrigenLimpieza[]>([]);
  const [marcados, setMarcados] = useState<OpcionesLimpieza>({ temp_usuario: true, temp_windows: false, papelera: false });
  const [fase, setFase] = useState<Fase>('cargando');
  const [confirmar, setConfirmar] = useState(false);
  const [resumen, setResumen] = useState<ResumenLimpieza | null>(null);
  const [error, setError] = useState<string | null>(null);

  const cargar = (): void => {
    if (!enTauri()) return;
    setFase('cargando');
    api
      .limpiezaVistaPrevia()
      .then((lista) => {
        setOrigenes(lista);
        setMarcados({
          temp_usuario: lista.find((o) => o.id === 'temp_usuario')?.marcado_defecto ?? true,
          temp_windows: lista.find((o) => o.id === 'temp_windows')?.marcado_defecto ?? false,
          papelera: lista.find((o) => o.id === 'papelera')?.marcado_defecto ?? false,
        });
        setFase('lista');
      })
      .catch((e) => {
        setError(mensajeError(e));
        setFase('lista');
      });
  };

  useEffect(cargar, []);

  const seleccion = origenes.filter((o) => marcados[o.id]);
  const total = seleccion.reduce((s, o) => s + o.bytes, 0);
  const pideAdmin = seleccion.some((o) => o.requiere_admin);
  const cerrar = (): void => {
    api.panelCerrar().catch(() => undefined);
  };

  const ejecutar = (): void => {
    setConfirmar(false);
    setFase('limpiando');
    setError(null);
    api
      .limpiezaEjecutar(marcados)
      .then((r) => {
        setResumen(r);
        setFase('hecho');
      })
      .catch((e) => {
        setError(mensajeError(e));
        setFase('lista');
      });
  };

  return (
    <Vidrio elemento="main" lleno solido className="gd-panel" aria-label="Limpiar temporales">
      <BarraPanel
        titulo="Limpiar temporales"
        subtitulo="Vista previa: solo archivos con más de 24 h; los que estén en uso se omiten"
        onCerrar={cerrar}
      />
      <div className="gd-panel__cuerpo">
        {fase === 'hecho' && resumen ? (
          <div className="gd-vista gd-vista--pila">
            <p className="gd-panel__destacado num">{bytes(resumen.liberado)} liberados</p>
            <p className="gd-etiqueta">
              {resumen.archivos_borrados} archivos borrados · {resumen.omitidos} en uso o recientes se dejaron intactos
            </p>
            {resumen.windows_temp && <p className="gd-etiqueta">Temporales de Windows: {resumen.windows_temp}</p>}
            {resumen.papelera && <p className="gd-etiqueta">Papelera: {resumen.papelera}</p>}
            <div className="gd-panel__acciones">
              <button type="button" className="gd-boton" onClick={cargar}>
                Volver a revisar
              </button>
              <button type="button" className="gd-boton gd-boton--primario" onClick={cerrar}>
                Listo
              </button>
            </div>
          </div>
        ) : (
          <div className="gd-vista gd-vista--pila">
            <p className="gd-etiqueta">Solo se borran archivos con más de 24 horas. Lo que esté en uso se omite.</p>
            <ul className="gd-opciones" aria-busy={fase === 'cargando'}>
              {fase === 'cargando' && <li className="gd-etiqueta">Calculando tamaño…</li>}
              {origenes.map((o) => (
                <li key={o.id}>
                  <label className="gd-opciones__fila">
                    <input
                      type="checkbox"
                      checked={marcados[o.id]}
                      disabled={fase !== 'lista'}
                      onChange={(e) => setMarcados({ ...marcados, [o.id]: e.target.checked })}
                    />
                    <span className="gd-opciones__nombre">
                      {o.nombre}
                      {o.requiere_admin && (
                        <span className="gd-opciones__escudo" title="Windows pedirá permiso de administrador">
                          <IconoEscudo />
                          <span className="gd-solo-lector">Requiere permiso de administrador</span>
                        </span>
                      )}
                    </span>
                    <span className="num">
                      {o.estimado ? '≈ ' : ''}
                      {bytes(o.bytes)}
                    </span>
                  </label>
                </li>
              ))}
            </ul>
            {error && (
              <p className="gd-widget__error" role="alert">
                {error}
              </p>
            )}
            <div className="gd-panel__acciones">
              <span className="gd-etiqueta num">Total: {bytes(total)}</span>
              <button
                type="button"
                className="gd-boton gd-boton--primario"
                disabled={fase !== 'lista' || seleccion.length === 0}
                onClick={() => setConfirmar(true)}
              >
                {fase === 'limpiando' ? 'Limpiando…' : 'Limpiar'}
              </button>
            </div>
          </div>
        )}
      </div>
      <Dialogo
        abierto={confirmar}
        titulo="¿Limpiar ahora?"
        descripcion={`Se liberarán hasta ${bytes(total)}. Esta acción no se puede deshacer.`}
        icono={<IconoEscoba />}
        elementos={seleccion.map((o) => ({ id: o.id, titulo: o.nombre, detalle: bytes(o.bytes) }))}
        aviso={pideAdmin ? 'Windows te pedirá permiso de administrador para los temporales del sistema.' : undefined}
        textoConfirmar="Limpiar"
        onConfirmar={ejecutar}
        onCancelar={() => setConfirmar(false)}
      />
    </Vidrio>
  );
}

export function BarraPanel({ titulo, subtitulo, onCerrar }: { titulo: string; subtitulo: string; onCerrar: () => void }) {
  return (
    <>
      <header className="gd-barra gd-panel__barra" data-tauri-drag-region="">
        <h1 className="gd-barra__titulo" data-tauri-drag-region="">
          {titulo}
        </h1>
        <div className="gd-barra__botones">
          <button type="button" className="gd-barra__boton" aria-label={`Cerrar ${titulo}`} title="Cerrar" onClick={onCerrar}>
            <svg className="gd-barra__svg" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
              <path d="M6 6l12 12M18 6 6 18" />
            </svg>
          </button>
        </div>
      </header>
      <p className="gd-panel__subtitulo" data-tauri-drag-region="">
        {subtitulo}
      </p>
    </>
  );
}
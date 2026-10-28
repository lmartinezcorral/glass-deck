import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { api, enTauri, mensajeError, type AccionAtajo, type Capa, type Config, type EstadoAtajo, type TemaConfig } from '../api';
import { Interruptor, Selector, Vidrio } from '../componentes';
import { useConfig } from '../hooks';
import { IconoCandado } from '../iconos';
import { ACENTOS, aplicarTema } from '../tema/tema';
import { BarraPanel } from './Limpieza';

type Pestana = 'widgets' | 'apariencia' | 'intervalos' | 'procesos' | 'atajos' | 'general';

const PESTANAS: ReadonlyArray<{ id: Pestana; nombre: string }> = [
  { id: 'widgets', nombre: 'Widgets' },
  { id: 'apariencia', nombre: 'Apariencia' },
  { id: 'intervalos', nombre: 'Intervalos' },
  { id: 'procesos', nombre: 'Procesos' },
  { id: 'atajos', nombre: 'Atajos' },
  { id: 'general', nombre: 'General' },
];

const NOMBRES_WIDGET: Record<string, string> = {
  cpu: 'CPU',
  ram: 'Memoria',
  disco: 'Disco',
  red: 'Red',
  procesos: 'Procesos',
};

const ESPERA_GUARDADO_MS = 400;

const MODIFICADORES = new Set(['Control', 'Alt', 'Shift', 'Meta', 'OS']);

export function combinacionDeTecla(e: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'key' | 'code'>): string | null {
  if (MODIFICADORES.has(e.key)) return null;
  const partes: string[] = [];
  if (e.ctrlKey) partes.push('Ctrl');
  if (e.altKey) partes.push('Alt');
  if (e.shiftKey) partes.push('Shift');
  if (e.metaKey) partes.push('Super');
  if (partes.length === 0) return null;
  let tecla = e.code;
  if (/^Key[A-Z]$/.test(tecla)) tecla = tecla.slice(3);
  else if (/^Digit\d$/.test(tecla)) tecla = tecla.slice(5);
  else if (!/^F\d{1,2}$/.test(tecla) && !['Space', 'Home', 'End', 'PageUp', 'PageDown', 'Insert'].includes(tecla)) return null;
  partes.push(tecla);
  return partes.join('+');
}

export function VistaCustomize() {
  const [config, setConfig] = useConfig();
  const [pestana, setPestana] = useState<Pestana>('widgets');
  const [error, setError] = useState<string | null>(null);
  const [guardado, setGuardado] = useState(false);
  const temporizador = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(temporizador.current), []);

  const guardar = (siguiente: Config): void => {
    setConfig(siguiente);
    aplicarTema({ ...siguiente.apariencia, animaciones: true });
    window.clearTimeout(temporizador.current);
    temporizador.current = window.setTimeout(() => {
      api
        .configGuardar(siguiente)
        .then(() => {
          setError(null);
          setGuardado(true);
          window.setTimeout(() => setGuardado(false), 1500);
        })
        .catch((e) => setError(mensajeError(e)));
    }, ESPERA_GUARDADO_MS);
  };

  const cerrar = (): void => {
    api.panelCerrar().catch(() => undefined);
  };

  return (
    <Vidrio elemento="main" lleno solido className="gd-panel gd-customize" aria-label="Customize">
      <BarraPanel
        titulo="Customize"
        subtitulo="Los cambios se aplican al instante y se guardan en %AppData%\Glassdeck\settings.json"
        onCerrar={cerrar}
      />
      <div className="gd-customize__contenido">
        <nav className="gd-customize__pestanas" role="tablist" aria-label="Secciones">
          {PESTANAS.map((p) => (
            <button
              key={p.id}
              type="button"
              role="tab"
              id={`pestana-${p.id}`}
              aria-selected={pestana === p.id}
              aria-controls={`seccion-${p.id}`}
              className="gd-customize__pestana"
              onClick={() => setPestana(p.id)}
            >
              {p.nombre}
            </button>
          ))}
        </nav>
        <section className="gd-customize__seccion" role="tabpanel" id={`seccion-${pestana}`} aria-labelledby={`pestana-${pestana}`}>
          {!config ? (
            <p className="gd-etiqueta">Cargando configuración…</p>
          ) : (
            <>
              {pestana === 'widgets' && <SeccionWidgets config={config} alCambiar={setConfig} alError={setError} />}
              {pestana === 'apariencia' && <SeccionApariencia config={config} guardar={guardar} />}
              {pestana === 'intervalos' && <SeccionIntervalos config={config} guardar={guardar} />}
              {pestana === 'procesos' && <SeccionProcesos config={config} guardar={guardar} />}
              {pestana === 'atajos' && <SeccionAtajos />}
              {pestana === 'general' && <SeccionGeneral config={config} guardar={guardar} alError={setError} />}
            </>
          )}
          <p className="gd-customize__estado" role="status" aria-live="polite">
            {error ? <span className="gd-widget__error">{error}</span> : guardado ? 'Guardado' : ''}
          </p>
        </section>
      </div>
    </Vidrio>
  );
}

interface PropsSeccion {
  config: Config;
  guardar: (c: Config) => void;
}

function SeccionWidgets({ config, alCambiar, alError }: { config: Config; alCambiar: (c: Config) => void; alError: (e: string | null) => void }) {
  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Widgets visibles</h2>
      {Object.entries(NOMBRES_WIDGET).map(([id, nombre]) => {
        const w = config.widgets[id];
        return (
          <Interruptor
            key={id}
            etiqueta={nombre}
            activo={w?.activo ?? false}
            onCambiar={(activo) => {
              if (w) alCambiar({ ...config, widgets: { ...config.widgets, [id]: { ...w, activo } } });
              api
                .widgetActivar(id, activo)
                .then(() => alError(null))
                .catch((e) => alError(mensajeError(e)));
            }}
          />
        );
      })}
      <p className="gd-etiqueta">Zonas, notas y clima llegan en fases posteriores.</p>
    </div>
  );
}

function SeccionApariencia({ config, guardar }: PropsSeccion) {
  const a = config.apariencia;
  const cambiar = (parcial: Partial<Config['apariencia']>): void => guardar({ ...config, apariencia: { ...a, ...parcial } });
  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Tema</h2>
      <Selector<TemaConfig>
        etiqueta="Tema"
        opciones={[
          { valor: 'oscuro', etiqueta: 'Oscuro' },
          { valor: 'claro', etiqueta: 'Claro' },
          { valor: 'sistema', etiqueta: 'Como Windows' },
        ]}
        valor={a.tema}
        onCambiar={(tema) => cambiar({ tema })}
      />
      <h2 className="gd-customize__titulo">Color de acento</h2>
      <div className="gd-acentos" role="radiogroup" aria-label="Color de acento">
        {ACENTOS.map((c) => (
          <button
            key={c.valor}
            type="button"
            role="radio"
            aria-checked={a.acento.toLowerCase() === c.valor}
            aria-label={c.nombre}
            title={c.nombre}
            className="gd-acentos__muestra"
            style={{ background: c.valor }}
            onClick={() => cambiar({ acento: c.valor })}
          />
        ))}
      </div>
      <Deslizador etiqueta="Opacidad del vidrio" min={0.2} max={0.9} paso={0.02} valor={a.opacidad} texto={`${Math.round(a.opacidad * 100)} %`} alCambiar={(opacidad) => cambiar({ opacidad })} />
      <Deslizador etiqueta="Desenfoque" min={0} max={48} paso={1} valor={a.desenfoque} texto={`${a.desenfoque} px`} alCambiar={(desenfoque) => cambiar({ desenfoque })} />
    </div>
  );
}

function Deslizador({ etiqueta, min, max, paso, valor, texto, alCambiar }: { etiqueta: string; min: number; max: number; paso: number; valor: number; texto: string; alCambiar: (v: number) => void }) {
  const pct = ((valor - min) / (max - min)) * 100;
  return (
    <label className="gd-deslizador">
      <span className="gd-deslizador__cabecera">
        <span>{etiqueta}</span>
        <b className="num">{texto}</b>
      </span>
      <input
        className="gd-rango"
        type="range"
        min={min}
        max={max}
        step={paso}
        value={valor}
        style={{ '--v': `${pct}%` } as CSSProperties}
        onChange={(e) => alCambiar(Number(e.target.value))}
      />
    </label>
  );
}

/** Solo valores en o sobre el mínimo del backend (`Intervalos::validar`), así no hay entrada inválida. */
export const OPCIONES_INTERVALO = {
  cpu_ram_red_ms: [1000, 2000, 3000, 5000],
  procesos_ms: [2000, 3000, 5000, 10000],
  disco_ms: [1000, 2000, 5000, 10000],
} as const satisfies Record<keyof Config['intervalos'], readonly number[]>;

function SeccionIntervalos({ config, guardar }: PropsSeccion) {
  const i = config.intervalos;
  const cambiar = (parcial: Partial<Config['intervalos']>): void => guardar({ ...config, intervalos: { ...i, ...parcial } });
  const opciones = (valores: readonly number[]) => valores.map((v) => ({ valor: String(v), etiqueta: `${v / 1000} s` }));
  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Cada cuánto se actualiza</h2>
      <p className="gd-etiqueta">Intervalos más largos consumen menos. Con batería se usan al menos 3 s.</p>
      <span className="gd-deslizador__cabecera">CPU, memoria y red</span>
      <Selector etiqueta="CPU, memoria y red" opciones={opciones(OPCIONES_INTERVALO.cpu_ram_red_ms)} valor={String(i.cpu_ram_red_ms)} onCambiar={(v) => cambiar({ cpu_ram_red_ms: Number(v) })} />
      <span className="gd-deslizador__cabecera">Procesos</span>
      <Selector etiqueta="Procesos" opciones={opciones(OPCIONES_INTERVALO.procesos_ms)} valor={String(i.procesos_ms)} onCambiar={(v) => cambiar({ procesos_ms: Number(v) })} />
      <span className="gd-deslizador__cabecera">Disco</span>
      <Selector etiqueta="Disco" opciones={opciones(OPCIONES_INTERVALO.disco_ms)} valor={String(i.disco_ms)} onCambiar={(v) => cambiar({ disco_ms: Number(v) })} />
    </div>
  );
}

function SeccionProcesos({ config, guardar }: PropsSeccion) {
  const [nuevo, setNuevo] = useState('');
  const [aviso, setAviso] = useState<string | null>(null);
  const [fijos, setFijos] = useState<string[]>([]);
  const lista = config.lista_negra_usuario;

  useEffect(() => {
    if (!enTauri()) return;
    api
      .procesosFijos()
      .then(setFijos)
      .catch(() => undefined);
  }, []);

  const agregar = async (): Promise<void> => {
    const nombre = nuevo.trim();
    if (!nombre) return;
    if (lista.some((n) => n.toLowerCase() === nombre.toLowerCase())) {
      setAviso('Ya está en tu lista.');
      return;
    }
    if (enTauri() && (await api.procesoEsFijo(nombre).catch(() => false))) {
      setAviso('Ese proceso ya está protegido siempre por Glassdeck.');
      setNuevo('');
      return;
    }
    setAviso(null);
    setNuevo('');
    guardar({ ...config, lista_negra_usuario: [...lista, nombre] });
  };

  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Procesos que nunca se cierran</h2>
      <p className="gd-aviso-linea">
        <IconoCandado /> Los procesos críticos de Windows y los de otros usuarios están protegidos siempre; esa lista no se puede editar.
      </p>
      {fijos.length > 0 && (
        <details className="gd-fijos">
          <summary>Ver los {fijos.length} protegidos siempre</summary>
          <ul className="gd-lista-editable" aria-label="Protegidos siempre">
            {fijos.map((n) => (
              <li key={n}>
                <span>{n}</span>
                <span className="gd-tabla__candado" title="Protegido siempre" aria-label="Protegido siempre">
                  <IconoCandado />
                </span>
              </li>
            ))}
          </ul>
        </details>
      )}
      <p className="gd-etiqueta">Agrega aquí los tuyos (por ejemplo, el nombre del programa que no quieres cerrar por accidente).</p>
      <form
        className="gd-vista__herramientas"
        onSubmit={(e) => {
          e.preventDefault();
          void agregar();
        }}
      >
        <input className="gd-campo" placeholder="programa.exe" aria-label="Nombre del proceso" value={nuevo} onChange={(e) => setNuevo(e.target.value)} />
        <button type="submit" className="gd-boton gd-boton--sm">
          Agregar
        </button>
      </form>
      {aviso && <p className="gd-etiqueta" role="status">{aviso}</p>}
      <ul className="gd-lista-editable">
        {lista.map((n) => (
          <li key={n}>
            <span>{n}</span>
            <button
              type="button"
              className="gd-boton gd-boton--sm"
              aria-label={`Quitar ${n}`}
              onClick={() => guardar({ ...config, lista_negra_usuario: lista.filter((x) => x !== n) })}
            >
              Quitar
            </button>
          </li>
        ))}
        {lista.length === 0 && <li className="gd-etiqueta">Tu lista está vacía.</li>}
      </ul>
    </div>
  );
}

const NOMBRES_ATAJO: Record<AccionAtajo, string> = {
  mostrar_ocultar: 'Mostrar u ocultar todo',
  privacidad: 'Modo privacidad',
};

function SeccionAtajos() {
  const [estados, setEstados] = useState<EstadoAtajo[]>([]);
  const [grabando, setGrabando] = useState<AccionAtajo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (enTauri()) api.atajosEstado().then(setEstados).catch((e) => setError(mensajeError(e)));
  }, []);

  const alTeclear = (accion: AccionAtajo, e: KeyboardEvent<HTMLButtonElement>): void => {
    if (grabando !== accion) return;
    e.preventDefault();
    if (e.key === 'Escape') {
      setGrabando(null);
      return;
    }
    const combinacion = combinacionDeTecla(e);
    if (!combinacion) return;
    setGrabando(null);
    api
      .atajoCambiar(accion, combinacion)
      .then((r) => {
        setEstados(r);
        setError(null);
      })
      .catch((err) => setError(mensajeError(err)));
  };

  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Atajos de teclado globales</h2>
      <p className="gd-etiqueta">Pulsa el botón y luego la combinación (con Ctrl, Alt, Shift o Windows). Esc cancela.</p>
      {estados.map((s) => (
        <div key={s.accion} className="gd-atajo">
          <span>{NOMBRES_ATAJO[s.accion]}</span>
          <button
            type="button"
            className={`gd-boton gd-boton--sm gd-atajo__tecla${grabando === s.accion ? ' gd-atajo__tecla--grabando' : ''}`}
            aria-label={`Cambiar atajo de ${NOMBRES_ATAJO[s.accion]}, actual ${s.combinacion}`}
            onClick={() => setGrabando(s.accion)}
            onKeyDown={(e) => alTeclear(s.accion, e)}
            onBlur={() => setGrabando((g) => (g === s.accion ? null : g))}
          >
            {grabando === s.accion ? 'Pulsa la combinación…' : s.combinacion.replace('Super', 'Win')}
          </button>
          {!s.registrado && <span className="gd-chip gd-chip--aviso">{s.error ?? 'Este atajo lo usa otro programa'}</span>}
        </div>
      ))}
      {error && (
        <p className="gd-widget__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

function SeccionGeneral({ config, guardar, alError }: PropsSeccion & { alError: (e: string | null) => void }) {
  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">General</h2>
      <Interruptor etiqueta="Iniciar con Windows" activo={config.arranque_windows} onCambiar={(v) => guardar({ ...config, arranque_windows: v })} />
      <span className="gd-deslizador__cabecera">Dónde viven los widgets</span>
      <Selector<Capa>
        etiqueta="Capa de los widgets"
        opciones={[
          { valor: 'escritorio', etiqueta: 'En el escritorio' },
          { valor: 'siempre_encima', etiqueta: 'Siempre encima' },
        ]}
        valor={config.capa_defecto}
        onCambiar={(capa_defecto) => guardar({ ...config, capa_defecto })}
      />
      <Interruptor
        etiqueta="Ahorrar batería"
        descripcion="Con batería, actualiza cada 3 s como mínimo y sin animaciones"
        activo={config.pausar_con_bateria}
        onCambiar={(v) => guardar({ ...config, pausar_con_bateria: v })}
      />
      <Interruptor
        etiqueta="Imán entre widgets"
        descripcion="Al soltar un widget cerca de otro, se alinea con un espacio de 8 px"
        activo={config.iman}
        onCambiar={(v) => guardar({ ...config, iman: v })}
      />
      <div className="gd-panel__acciones">
        <button
          type="button"
          className="gd-boton"
          onClick={() =>
            api
              .restablecerPosiciones()
              .then(() => alError(null))
              .catch((e) => alError(mensajeError(e)))
          }
        >
          Restablecer posiciones
        </button>
      </div>
    </div>
  );
}

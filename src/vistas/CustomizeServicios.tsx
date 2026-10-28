import { useEffect, useId, useState, type ReactNode } from 'react';
import {
  api,
  enTauri,
  escuchar,
  EVENTOS,
  mensajeError,
  type AvisoEnviado,
  type ClaveCredencial,
  type Config,
  type FilaClaude,
  type ModoUbicacion,
  type PrecioModelo,
  type ServicioWeb,
  type Tokens,
  type Ubicacion,
  type Umbrales,
} from '../api';
import { Interruptor, Selector } from '../componentes';
import { usd } from '../formato';
import { useServicio } from '../hooks';
import {
  credencialPresente,
  estadoServicio,
  NOMBRE_SERVICIO,
  PuntoCuenta,
  resumenCuentas,
  textoEstadoCuenta,
  useCredenciales,
  useCuentasWeb,
  type ResumenCuenta,
} from './Cuentas';
import { CuentasGoogle } from './CuentasGoogle';

interface PropsSeccion {
  config: Config;
  guardar: (c: Config) => void;
}

export function precioPara(modelo: string, precios: readonly PrecioModelo[]): PrecioModelo | null {
  const m = modelo.toLowerCase();
  let mejor: PrecioModelo | null = null;
  for (const p of precios) {
    if (p.patron && m.includes(p.patron) && (!mejor || p.patron.length >= mejor.patron.length)) mejor = p;
  }
  return mejor;
}

export function costoTokens(t: Tokens, p: PrecioModelo): number {
  return (t.entrada * p.entrada + t.salida * p.salida + t.cache_escritura * p.cache_escritura + t.cache_lectura * p.cache_lectura) / 1_000_000;
}

/** Misma regla que `claude_code::costo` en el núcleo, para ver el efecto de un precio antes de que llegue la lectura nueva. */
export function estimarCosto(filas: readonly FilaClaude[], precios: readonly PrecioModelo[], dia?: string): number {
  let total = 0;
  for (const f of filas) {
    if (dia && f.dia !== dia) continue;
    const p = precioPara(f.modelo, precios);
    if (p) total += costoTokens(f.tokens, p);
  }
  return total;
}

function leerNumero(texto: string): number | null {
  const limpio = texto.trim().replace(',', '.');
  if (!limpio) return null;
  const n = Number(limpio);
  return Number.isFinite(n) ? n : null;
}

function CampoNumero({
  valor,
  etiqueta,
  alCambiar,
  min = 0,
  max = 100_000,
  permitirVacio = false,
  placeholder,
}: {
  valor: number | null;
  etiqueta: string;
  alCambiar: (v: number | null) => void;
  min?: number;
  max?: number;
  permitirVacio?: boolean;
  placeholder?: string;
}) {
  const [texto, setTexto] = useState(valor === null ? '' : String(valor));
  const [enfocado, setEnfocado] = useState(false);
  useEffect(() => {
    if (!enfocado) setTexto(valor === null ? '' : String(valor));
  }, [valor, enfocado]);
  const n = leerNumero(texto);
  const valido = n === null ? permitirVacio : n >= min && n <= max;
  return (
    <input
      className="gd-campo gd-campo--numero num"
      inputMode="decimal"
      aria-label={etiqueta}
      aria-invalid={!valido}
      placeholder={placeholder}
      value={texto}
      onFocus={() => setEnfocado(true)}
      onBlur={() => setEnfocado(false)}
      onChange={(e) => {
        setTexto(e.target.value);
        const v = leerNumero(e.target.value);
        if (v === null ? permitirVacio : v >= min && v <= max) alCambiar(v);
      }}
    />
  );
}

const CAMPOS_PRECIO: ReadonlyArray<{ campo: keyof Omit<PrecioModelo, 'patron'>; nombre: string }> = [
  { campo: 'entrada', nombre: 'Entrada' },
  { campo: 'salida', nombre: 'Salida' },
  { campo: 'cache_escritura', nombre: 'Caché escritura' },
  { campo: 'cache_lectura', nombre: 'Caché lectura' },
];

const MAX_UMBRALES = 5;

export function normalizarUmbrales(valores: readonly number[]): number[] {
  return [...new Set(valores.map(Math.round).filter((v) => v >= 1 && v <= 100))].sort((a, b) => a - b).slice(0, MAX_UMBRALES);
}

function EditorUmbrales({ etiqueta, valores, alCambiar }: { etiqueta: string; valores: number[]; alCambiar: (v: number[]) => void }) {
  const [nuevo, setNuevo] = useState('');
  const id = useId();
  const n = leerNumero(nuevo);
  const valido = n !== null && n >= 1 && n <= 100 && !valores.includes(Math.round(n));
  const lleno = valores.length >= MAX_UMBRALES;
  return (
    <div className="gd-umbrales" role="group" aria-labelledby={id}>
      <span id={id} className="gd-deslizador__cabecera">
        {etiqueta}
      </span>
      <div className="gd-umbrales__fila">
        {valores.map((v) => (
          <button
            key={v}
            type="button"
            className="gd-chip gd-chip--acento gd-umbrales__chip num"
            aria-label={`Quitar umbral ${v} %`}
            onClick={() => alCambiar(valores.filter((x) => x !== v))}
          >
            {v} % <span aria-hidden="true">×</span>
          </button>
        ))}
        {valores.length === 0 && <span className="gd-etiqueta">Sin avisos para este medidor</span>}
        <form
          className="gd-umbrales__nuevo"
          onSubmit={(e) => {
            e.preventDefault();
            if (!valido || lleno || n === null) return;
            alCambiar(normalizarUmbrales([...valores, n]));
            setNuevo('');
          }}
        >
          <input
            className="gd-campo gd-campo--numero num"
            inputMode="numeric"
            placeholder="%"
            aria-label={`Nuevo umbral para ${etiqueta}`}
            value={nuevo}
            disabled={lleno}
            onChange={(e) => setNuevo(e.target.value)}
          />
          <button type="submit" className="gd-boton gd-boton--sm" disabled={!valido || lleno}>
            Agregar
          </button>
        </form>
      </div>
    </div>
  );
}

const NOMBRES_MEDIDOR: Record<string, string> = {
  cursor: 'Cursor',
  claude_5h: 'Claude · 5 h',
  claude_semanal: 'Claude · semanal',
  chatgpt_5h: 'ChatGPT · 5 h',
  chatgpt_semanal: 'ChatGPT · semanal',
};

export function describirAviso(clave: string): string {
  const primero = clave.indexOf(':');
  const ultimo = clave.lastIndexOf(':');
  if (primero < 0 || ultimo <= primero) return clave;
  const medidor = clave.slice(0, primero);
  const ciclo = clave.slice(primero + 1, ultimo);
  const umbral = clave.slice(ultimo + 1);
  return `${NOMBRES_MEDIDOR[medidor] ?? medidor} · ${umbral} % · ciclo ${ciclo}`;
}

export function SeccionGasto({ config, guardar }: PropsSeccion) {
  const claude = useServicio('claude_code');
  const [nuevoPatron, setNuevoPatron] = useState('');
  const [historial, setHistorial] = useState<AvisoEnviado[]>([]);
  const [prueba, setPrueba] = useState<string | null>(null);
  const precios = config.precios_modelos;
  const u = config.umbrales;
  const datos = claude?.datos ?? null;
  const sinPrecio = (datos?.sin_precio ?? []).filter((m) => !precioPara(m, precios));

  useEffect(() => {
    if (!enTauri()) return;
    const cargar = (): void => {
      api
        .avisosHistorial()
        .then(setHistorial)
        .catch(() => undefined);
    };
    cargar();
    const quitar = escuchar(EVENTOS.aviso, cargar);
    return () => {
      quitar.then((f) => f());
    };
  }, []);

  const cambiarPrecios = (lista: PrecioModelo[]): void => guardar({ ...config, precios_modelos: lista });
  const cambiarUmbrales = (parcial: Partial<Umbrales>): void => guardar({ ...config, umbrales: { ...u, ...parcial } });
  const agregarPatron = (texto: string): void => {
    const patron = texto.trim().toLowerCase();
    if (!patron || precios.some((p) => p.patron === patron)) return;
    const base = precioPara(patron, precios);
    cambiarPrecios([...precios, { patron, entrada: base?.entrada ?? 0, salida: base?.salida ?? 0, cache_escritura: base?.cache_escritura ?? 0, cache_lectura: base?.cache_lectura ?? 0 }]);
  };

  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Precios por modelo</h2>
      <p className="gd-etiqueta">
        USD por millón de tokens. Cada fila aplica a los modelos cuyo nombre contiene el patrón; si varios coinciden, gana el más largo.
      </p>
      <div className="gd-tabla-contenedor gd-precios">
        <table className="gd-tabla gd-tabla--compacta" aria-label="Precios por modelo">
          <thead>
            <tr>
              <th>Patrón</th>
              {CAMPOS_PRECIO.map((c) => (
                <th key={c.campo}>{c.nombre}</th>
              ))}
              <th>
                <span className="gd-solo-lector">Acciones</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {precios.map((p, i) => (
              <tr key={p.patron}>
                <td>{p.patron}</td>
                {CAMPOS_PRECIO.map((c) => (
                  <td key={c.campo}>
                    <CampoNumero
                      etiqueta={`${c.nombre} de ${p.patron}, USD por millón`}
                      valor={p[c.campo]}
                      alCambiar={(v) => v !== null && cambiarPrecios(precios.map((x, j) => (j === i ? { ...x, [c.campo]: v } : x)))}
                    />
                  </td>
                ))}
                <td>
                  <button
                    type="button"
                    className="gd-boton gd-boton--sm"
                    aria-label={`Quitar ${p.patron}`}
                    onClick={() => cambiarPrecios(precios.filter((_, j) => j !== i))}
                  >
                    Quitar
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <form
        className="gd-vista__herramientas"
        onSubmit={(e) => {
          e.preventDefault();
          agregarPatron(nuevoPatron);
          setNuevoPatron('');
        }}
      >
        <input
          className="gd-campo"
          placeholder="patrón del modelo, p. ej. opus-5"
          aria-label="Patrón del modelo nuevo"
          value={nuevoPatron}
          maxLength={60}
          onChange={(e) => setNuevoPatron(e.target.value)}
        />
        <button type="submit" className="gd-boton gd-boton--sm" disabled={!nuevoPatron.trim()}>
          Agregar modelo
        </button>
      </form>
      {sinPrecio.length > 0 && (
        <div className="gd-aviso-linea gd-umbrales__fila" role="note">
          <span>Modelos sin precio (hoy cuentan $0):</span>
          {sinPrecio.map((m) => (
            <button key={m} type="button" className="gd-chip gd-chip--aviso" onClick={() => agregarPatron(m)} aria-label={`Agregar precio para ${m}`}>
              + {m}
            </button>
          ))}
        </div>
      )}
      <p className="gd-customize__estimacion num" role="status" aria-live="polite">
        {datos
          ? `Claude Code con estos precios: hoy ≈ ${usd(estimarCosto(datos.filas, precios, datos.hoy))} · últimos 30 días ≈ ${usd(estimarCosto(datos.filas, precios))}`
          : 'Activa el widget Spending para ver el costo con estos precios.'}
      </p>

      <h2 className="gd-customize__titulo">Avisos de gasto</h2>
      <Interruptor
        etiqueta="Notificaciones de Windows"
        descripcion="Un aviso por umbral en cada ciclo de facturación o ventana de límite"
        activo={u.notificaciones}
        onCambiar={(notificaciones) => cambiarUmbrales({ notificaciones })}
      />
      <EditorUmbrales etiqueta="Cursor (% del presupuesto o del límite del plan)" valores={u.cursor} alCambiar={(cursor) => cambiarUmbrales({ cursor })} />
      <label className="gd-campo-etiquetado">
        <span className="gd-deslizador__cabecera">Presupuesto mensual de Cursor (USD)</span>
        <CampoNumero
          etiqueta="Presupuesto mensual de Cursor en USD"
          valor={u.presupuesto_cursor_usd}
          permitirVacio
          min={1}
          placeholder="Vacío: usa el límite del plan"
          alCambiar={(presupuesto_cursor_usd) => cambiarUmbrales({ presupuesto_cursor_usd })}
        />
      </label>
      <EditorUmbrales etiqueta="Claude · límite de 5 h" valores={u.claude_5h} alCambiar={(claude_5h) => cambiarUmbrales({ claude_5h })} />
      <EditorUmbrales etiqueta="Claude · límite semanal" valores={u.claude_semanal} alCambiar={(claude_semanal) => cambiarUmbrales({ claude_semanal })} />
      <EditorUmbrales etiqueta="ChatGPT · límite de 5 h" valores={u.chatgpt_5h} alCambiar={(chatgpt_5h) => cambiarUmbrales({ chatgpt_5h })} />
      <EditorUmbrales etiqueta="ChatGPT · límite semanal" valores={u.chatgpt_semanal} alCambiar={(chatgpt_semanal) => cambiarUmbrales({ chatgpt_semanal })} />
      <div className="gd-panel__acciones">
        <span className="gd-etiqueta" role="status">
          {prueba}
        </span>
        <button
          type="button"
          className="gd-boton"
          onClick={() =>
            api
              .avisosProbar()
              .then(() => setPrueba('Aviso enviado. Haz clic en él mientras está en pantalla para abrir Spending.'))
              .catch((e) => setPrueba(mensajeError(e)))
          }
        >
          Probar aviso
        </button>
      </div>
      <h3 className="gd-customize__subtitulo">Avisos enviados</h3>
      <ul className="gd-lista-editable" aria-label="Avisos enviados">
        {historial.map((a) => (
          <li key={a.clave}>
            <span>{describirAviso(a.clave)}</span>
            <span className="gd-etiqueta num">{new Date(a.hora_ms).toLocaleString('es-MX', { dateStyle: 'short', timeStyle: 'short' })}</span>
          </li>
        ))}
        {historial.length === 0 && <li className="gd-etiqueta">Todavía no se ha enviado ningún aviso.</li>}
      </ul>
    </div>
  );
}

const ESPERA_BUSQUEDA_MS = 400;

export function BuscadorCiudad({ alElegir }: { alElegir: (u: Ubicacion) => void }) {
  const [texto, setTexto] = useState('');
  const [resultados, setResultados] = useState<Ubicacion[] | null>(null);
  const [buscando, setBuscando] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const t = texto.trim();
    setError(null);
    if (t.length < 2) {
      setResultados(null);
      setBuscando(false);
      return;
    }
    let vigente = true;
    const id = window.setTimeout(() => {
      setBuscando(true);
      api
        .climaBuscarCiudad(t)
        .then((r) => vigente && setResultados(r))
        .catch((e) => {
          if (!vigente) return;
          setResultados(null);
          setError(mensajeError(e));
        })
        .finally(() => vigente && setBuscando(false));
    }, ESPERA_BUSQUEDA_MS);
    return () => {
      vigente = false;
      window.clearTimeout(id);
    };
  }, [texto]);

  return (
    <div className="gd-vista gd-vista--pila gd-buscador">
      <input
        className="gd-campo"
        type="search"
        placeholder="Escribe una ciudad, p. ej. Chihuahua"
        aria-label="Buscar ciudad"
        maxLength={80}
        value={texto}
        onChange={(e) => setTexto(e.target.value)}
      />
      <p className="gd-etiqueta" role="status" aria-live="polite">
        {error ?? (buscando ? 'Buscando…' : resultados && resultados.length === 0 ? 'Sin resultados.' : texto.trim().length === 1 ? 'Escribe al menos 2 letras.' : '')}
      </p>
      {resultados && resultados.length > 0 && (
        <ul className="gd-lista-editable" aria-label="Resultados de la búsqueda">
          {resultados.map((u) => (
            <li key={`${u.latitud},${u.longitud}`}>
              <button type="button" className="gd-buscador__opcion" onClick={() => alElegir(u)}>
                <b>{u.nombre}</b>
                <span className="gd-etiqueta">{[u.region, u.pais].filter(Boolean).join(', ')}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function SeccionClima({ config, alFijar }: { config: Config; alFijar: (c: Config) => void }) {
  const [modo, setModo] = useState<ModoUbicacion>(config.clima.modo);
  const [error, setError] = useState<string | null>(null);
  const fijada = config.clima.modo === 'manual' ? config.clima.ubicacion : null;

  useEffect(() => setModo(config.clima.modo), [config.clima.modo]);

  const fijar = (u: Ubicacion | null): void => {
    api
      .climaFijarUbicacion(u)
      .then((c) => {
        setError(null);
        alFijar(c);
      })
      .catch((e) => setError(mensajeError(e)));
  };

  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Ubicación del clima</h2>
      <Selector<ModoUbicacion>
        etiqueta="Modo de ubicación"
        opciones={[
          { valor: 'automatica', etiqueta: 'Automática por IP' },
          { valor: 'manual', etiqueta: 'Ciudad manual' },
        ]}
        valor={modo}
        onCambiar={(m) => {
          setModo(m);
          if (m === 'automatica' && config.clima.modo !== 'automatica') fijar(null);
          if (m === 'manual' && config.clima.ubicacion && config.clima.modo !== 'manual') fijar(config.clima.ubicacion);
        }}
      />
      {modo === 'automatica' ? (
        <p className="gd-etiqueta">Glassdeck consulta ipapi.co para conocer tu ciudad aproximada, como máximo cada 6 horas. En modo manual no se consulta.</p>
      ) : (
        <>
          <p className="gd-aviso-linea">
            {fijada ? (
              <>
                Ciudad fijada: <b>{[fijada.nombre, fijada.region, fijada.pais].filter(Boolean).join(', ')}</b>
              </>
            ) : (
              'Busca tu ciudad y elígela de la lista.'
            )}
          </p>
          <BuscadorCiudad alElegir={(u) => fijar(u)} />
        </>
      )}
      {error && (
        <p className="gd-widget__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

function Tarjeta({ titulo, detalle, chip, children }: { titulo: string; detalle: string; chip?: ReactNode; children: ReactNode }) {
  return (
    <section className="gd-tarjeta gd-cuenta" aria-label={titulo}>
      <h3>
        {titulo}
        {chip}
      </h3>
      <p className="gd-etiqueta">{detalle}</p>
      {children}
    </section>
  );
}

function ClaveAdmin({
  clave,
  presente,
  etiqueta,
  nombre,
  ejecutar,
}: {
  clave: Extract<ClaveCredencial, 'anthropic_admin' | 'openai_admin'>;
  presente: boolean;
  etiqueta: string;
  nombre: string;
  ejecutar: (p: Promise<unknown>, exito?: string) => void;
}) {
  const [valor, setValor] = useState('');
  if (presente) {
    return (
      <div className="gd-vista__herramientas">
        <span className="gd-chip gd-chip--ok">Clave guardada</span>
        <button type="button" className="gd-boton gd-boton--sm" aria-label={`Borrar clave de ${nombre}`} onClick={() => ejecutar(api.bovedaBorrar(clave), `Clave de ${nombre} borrada.`)}>
          Borrar clave
        </button>
      </div>
    );
  }
  return (
    <form
      className="gd-vista__herramientas"
      onSubmit={(e) => {
        e.preventDefault();
        const limpio = valor.trim();
        if (!limpio) return;
        setValor('');
        ejecutar(api.bovedaGuardar(clave, limpio), `Clave de ${nombre} guardada.`);
      }}
    >
      <input
        className="gd-campo"
        type="password"
        autoComplete="off"
        spellCheck={false}
        placeholder="Clave de administrador (opcional)"
        aria-label={etiqueta}
        value={valor}
        onChange={(e) => setValor(e.target.value)}
      />
      <button type="submit" className="gd-boton gd-boton--sm" disabled={!valor.trim()}>
        Guardar
      </button>
    </form>
  );
}

function BotonesSesion({
  servicio,
  estado,
  conectando,
  ejecutar,
}: {
  servicio: ServicioWeb;
  estado: ResumenCuenta['estado'];
  conectando: boolean;
  ejecutar: (p: Promise<unknown>, exito?: string) => void;
}) {
  const nombre = NOMBRE_SERVICIO[servicio];
  const conectar = (
    <button
      type="button"
      className="gd-boton gd-boton--sm gd-boton--primario"
      aria-label={`${estado === 'reconectar' ? 'Reconectar' : 'Conectar'} ${nombre}`}
      disabled={conectando}
      onClick={() => ejecutar(api.cuentaConectar(servicio), `Inicia sesión en la ventana «Glassdeck · Conectar ${nombre}»; se cierra sola al terminar.`)}
    >
      {conectando ? 'Esperando inicio de sesión…' : estado === 'reconectar' ? 'Reconectar' : 'Conectar'}
    </button>
  );
  return (
    <div className="gd-panel__acciones">
      {estado !== 'conectada' && conectar}
      {estado !== 'sin_conectar' && (
        <button
          type="button"
          className="gd-boton gd-boton--sm"
          aria-label={`Desconectar ${nombre}`}
          onClick={() => ejecutar(api.cuentaDesconectar(servicio), `${nombre} desconectado: se borró la sesión y su perfil de login.`)}
        >
          Desconectar
        </button>
      )}
    </div>
  );
}

export function SeccionCuentas() {
  const [credenciales, recargar] = useCredenciales();
  const [web, recargarWeb] = useCuentasWeb();
  const claude = useServicio('claude_code');
  const [mensaje, setMensaje] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const cuentas = resumenCuentas(credenciales, web);
  const presente = (c: ClaveCredencial): boolean => credencialPresente(credenciales, c);
  const sesion = (servicio: ServicioWeb) => ({
    servicio,
    estado: estadoServicio(servicio, credenciales, web),
    conectando: web?.web.find((c) => c.servicio === servicio)?.conectando ?? false,
  });

  const ejecutar = (p: Promise<unknown>, exito?: string): void => {
    p.then(() => {
      setError(null);
      if (exito) setMensaje(exito);
      recargar();
      recargarWeb();
    }).catch((e) => {
      setMensaje(null);
      setError(mensajeError(e));
    });
  };

  const estadoDe = (id: ResumenCuenta['id']) => cuentas.find((c) => c.id === id)!;
  const chipEstado = (id: ResumenCuenta['id']) => {
    const c = estadoDe(id);
    return (
      <span className="gd-cuenta__estado gd-etiqueta">
        <PuntoCuenta estado={c.estado} />
        {textoEstadoCuenta(c.estado)}
      </span>
    );
  };

  return (
    <div className="gd-vista gd-vista--pila">
      <h2 className="gd-customize__titulo">Cuentas e inicios de sesión</h2>
      <p className="gd-etiqueta">Las credenciales se guardan en el Administrador de credenciales de Windows; Glassdeck nunca las muestra.</p>

      <CuentasGoogle credenciales={credenciales} recargar={recargar} />

      <Tarjeta
        titulo="Cursor"
        detalle="Gasto del ciclo leído del panel de cursor.com"
        chip={
          <>
            <span className="gd-chip gd-chip--aviso">no oficial</span>
            {chipEstado('cursor')}
          </>
        }
      >
        <p className="gd-etiqueta">Cursor no ofrece una API pública de gasto; si cambia su web, este dato puede dejar de actualizarse.</p>
        {estadoDe('cursor').estado === 'reconectar' && <p className="gd-aviso-linea">{estadoDe('cursor').detalle}</p>}
        <BotonesSesion {...sesion('cursor')} ejecutar={ejecutar} />
      </Tarjeta>

      <Tarjeta titulo="Claude" detalle="Claude Code, Claude.ai y la consola de la API" chip={chipEstado('claude')}>
        <div className="gd-cuenta__fila">
          <b>Claude Code (local)</b>
          <span className={`gd-chip ${claude?.datos ? 'gd-chip--ok' : ''}`}>{claude?.datos ? 'Detectado' : 'Sin datos'}</span>
          <span className="gd-etiqueta">Lee los registros locales de uso. No requiere inicio de sesión.</span>
        </div>
        <div className="gd-cuenta__fila">
          <b>Claude.ai</b>
          <span className="gd-chip gd-chip--aviso">no oficial</span>
          <span className="gd-etiqueta">
            {estadoDe('claude').estado === 'reconectar' ? estadoDe('claude').detalle : '% usado del límite de 5 h y del semanal.'}
          </span>
          <BotonesSesion {...sesion('claude_ai')} ejecutar={ejecutar} />
        </div>
        <div className="gd-cuenta__fila gd-cuenta__fila--consola">
          <b>Consola de la API</b>
          <span className="gd-chip">Opcional</span>
          <span className="gd-etiqueta">Costo real del mes de tu organización con una clave de administrador.</span>
          <ClaveAdmin
            clave="anthropic_admin"
            presente={presente('anthropic_admin')}
            etiqueta="Clave de administrador de la consola de Anthropic"
            nombre="la consola de Anthropic"
            ejecutar={ejecutar}
          />
        </div>
      </Tarjeta>

      <Tarjeta
        titulo="ChatGPT"
        detalle="Límites de Codex en chatgpt.com y costo de la API de OpenAI"
        chip={
          <>
            <span className="gd-chip gd-chip--aviso">no oficial</span>
            {chipEstado('chatgpt')}
          </>
        }
      >
        <div className="gd-cuenta__fila">
          <b>ChatGPT</b>
          <span className="gd-chip gd-chip--aviso">no oficial</span>
          <span className="gd-etiqueta">
            {estadoDe('chatgpt').estado === 'reconectar' ? estadoDe('chatgpt').detalle : '% usado de la ventana de 5 h y de la semanal de Codex.'}
          </span>
          <BotonesSesion {...sesion('chatgpt')} ejecutar={ejecutar} />
        </div>
        <div className="gd-cuenta__fila gd-cuenta__fila--consola">
          <b>API de OpenAI</b>
          <span className="gd-chip">Opcional</span>
          <span className="gd-etiqueta">Costo real del mes de tu organización con una clave de administrador.</span>
          <ClaveAdmin
            clave="openai_admin"
            presente={presente('openai_admin')}
            etiqueta="Clave de administrador de la API de OpenAI"
            nombre="la API de OpenAI"
            ejecutar={ejecutar}
          />
        </div>
      </Tarjeta>

      <p className="gd-etiqueta" role="status" aria-live="polite">
        {mensaje}
      </p>
      {error && (
        <p className="gd-widget__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

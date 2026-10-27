import { useEffect, useMemo, useState, type CSSProperties, type ReactNode } from 'react';
import {
  Barra,
  BarraWidget,
  Dialogo,
  GraficaHistorial,
  GraficaMultiples,
  Interruptor,
  Medidor,
  Pildora,
  Selector,
  Sparkline,
  Vidrio,
  type ElementoDialogo,
  type SerieHistorial,
} from '../componentes';
import { ACENTOS, aplicarTema, seguirTemaSistema, type Tema } from '../tema/tema';

function generador(semilla: number): () => number {
  let a = semilla;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function caminata(n: number, inicio: number, paso: number, min: number, max: number, azar: () => number): number[] {
  const salida: number[] = [];
  let v = inicio;
  for (let i = 0; i < n; i++) {
    v = Math.min(max, Math.max(min, v + (azar() - 0.5) * paso));
    salida.push(v);
  }
  return salida;
}

const azar = generador(20261025);

const HISTORIAL_HORA: readonly SerieHistorial[] = [
  { nombre: 'Bajada', datos: caminata(3600, 4, 3, 0.1, 14, azar), variante: 'acento' },
  { nombre: 'Subida', datos: caminata(3600, 0.6, 0.8, 0.05, 3, azar), variante: 'secundario' },
];

const NUCLEOS: readonly SerieHistorial[] = Array.from({ length: 8 }, (_, i) => ({
  nombre: `N${i}`,
  datos: caminata(120, 20 + azar() * 40, 22, 2, 100, azar),
}));

const PROCESOS: readonly ElementoDialogo[] = [
  { id: 'cursor', titulo: 'Cursor.exe', icono: <LetraApp letra="C" fondo="#111" />, detalle: '1.8 GB', insignia: <span className="gd-chip gd-chip--aviso">confirmación reforzada</span> },
  { id: 'edge', titulo: 'msedge.exe', icono: <LetraApp letra="e" fondo="#1d63ed" />, detalle: '1.2 GB' },
  { id: 'node', titulo: 'node.exe', icono: <LetraApp letra="n" fondo="#3c873a" />, detalle: '540 MB' },
];

const OPCIONES_TEMA = [
  { valor: 'oscuro', etiqueta: 'Oscuro' },
  { valor: 'claro', etiqueta: 'Claro' },
  { valor: 'sistema', etiqueta: 'Sistema' },
] as const satisfies ReadonlyArray<{ valor: Tema; etiqueta: string }>;

const OPCIONES_ORDEN = [
  { valor: 'cpu', etiqueta: 'CPU' },
  { valor: 'ram', etiqueta: 'RAM' },
] as const;

function LetraApp({ letra, fondo }: { letra: string; fondo: string }) {
  return (
    <span
      style={{
        display: 'grid',
        placeItems: 'center',
        width: 20,
        height: 20,
        borderRadius: 6,
        background: fondo,
        color: '#fff',
        fontSize: 10,
        fontWeight: 700,
      }}
    >
      {letra}
    </span>
  );
}

function Icono({ children }: { children: ReactNode }) {
  return (
    <svg className="gd-icono" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}

const ICONO_CPU = (
  <Icono>
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
    <path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3" />
  </Icono>
);
const ICONO_DISCO = (
  <Icono>
    <rect x="3" y="5" width="18" height="14" rx="3" />
    <path d="M3 13h18" />
  </Icono>
);
const ICONO_RED = (
  <Icono>
    <path d="M8 4v16M8 20l-4-4M8 20l4-4M16 20V4M16 4l-4 4M16 4l4 4" />
  </Icono>
);
const ICONO_LISTA = (
  <Icono>
    <path d="M9 6h12M9 12h12M9 18h12" />
    <circle cx="4.5" cy="6" r="1.2" />
    <circle cx="4.5" cy="12" r="1.2" />
    <circle cx="4.5" cy="18" r="1.2" />
  </Icono>
);

export function Galeria() {
  const [tema, setTema] = useState<Tema>('oscuro');
  const [acento, setAcento] = useState<string>(ACENTOS[0]?.valor ?? '#22d3ee');
  const [opacidad, setOpacidad] = useState(0.44);
  const [animaciones, setAnimaciones] = useState(true);

  const [cpu, setCpu] = useState(34);
  const [historialCpu, setHistorialCpu] = useState<number[]>(() => caminata(61, 32, 14, 8, 85, azar));
  const [bajada, setBajada] = useState<number[]>(() => caminata(61, 4, 4, 0.1, 14, azar));

  const [fijado, setFijado] = useState(false);
  const [minimizado, setMinimizado] = useState(false);
  const [maximizado, setMaximizado] = useState(false);
  const [cerrado, setCerrado] = useState(false);
  const [arrastrando, setArrastrando] = useState(false);
  const [dialogo, setDialogo] = useState(false);
  const [orden, setOrden] = useState<'cpu' | 'ram'>('cpu');
  const [arranque, setArranque] = useState(true);
  const [mensaje, setMensaje] = useState('');

  useEffect(() => {
    aplicarTema({ tema, acento, opacidad, desenfoque: 24, animaciones });
  }, [tema, acento, opacidad, animaciones]);

  useEffect(() => seguirTemaSistema(), []);

  useEffect(() => {
    const id = window.setInterval(() => {
      setCpu((v) => Math.min(98, Math.max(4, v + (azar() - 0.5) * 22)));
      setHistorialCpu((h) => [...h.slice(1), Math.min(98, Math.max(4, (h[h.length - 1] ?? 30) + (azar() - 0.5) * 16))]);
      setBajada((h) => [...h.slice(1), Math.min(14, Math.max(0.1, (h[h.length - 1] ?? 4) + (azar() - 0.5) * 4))]);
    }, 1000);
    return () => window.clearInterval(id);
  }, []);

  const ultimaBajada = bajada[bajada.length - 1] ?? 0;
  const estiloRango = useMemo(() => ({ '--v': `${opacidad * 100}%` }) as CSSProperties, [opacidad]);

  return (
    <main className="gd-galeria">
      <Vidrio elemento="header" solido className="gd-galeria__cabecera">
        <h1>Glassdeck · Galería de componentes</h1>
        <Selector etiqueta="Tema" opciones={OPCIONES_TEMA} valor={tema} onCambiar={setTema} />
        <div className="gd-galeria__acentos" role="group" aria-label="Color de acento">
          {ACENTOS.map((a) => (
            <button
              key={a.valor}
              type="button"
              className="gd-galeria__acento"
              style={{ color: a.valor }}
              aria-pressed={a.valor === acento}
              aria-label={`Acento ${a.nombre}`}
              title={a.nombre}
              onClick={() => setAcento(a.valor)}
            />
          ))}
        </div>
        <label className="gd-galeria__opacidad">
          Opacidad
          <input
            className="gd-rango"
            type="range"
            min={0}
            max={100}
            value={Math.round(opacidad * 100)}
            style={estiloRango}
            onChange={(e) => setOpacidad(Number(e.target.value) / 100)}
          />
          <b className="num">{Math.round(opacidad * 100)} %</b>
        </label>
        <Interruptor etiqueta="Animaciones" activo={animaciones} onCambiar={setAnimaciones} />
      </Vidrio>

      <div className="gd-galeria__rejilla">
        {!cerrado && !minimizado && (
          <Vidrio elemento="section" className="gd-widget" fijado={fijado} arrastrando={arrastrando} aria-label="Widget CPU">
            <BarraWidget
              titulo="CPU"
              icono={ICONO_CPU}
              fijado={fijado}
              maximizado={maximizado}
              onPin={() => setFijado((v) => !v)}
              onMinimizar={() => setMinimizado(true)}
              onMaximizar={() => setMaximizado((v) => !v)}
              onCerrar={() => setCerrado(true)}
            />
            <div className="gd-widget__cuerpo">
              <div className="gd-galeria__fila">
                <Medidor valor={cpu} etiqueta="Uso de CPU" subetiqueta="% uso" />
                <dl className="gd-kv">
                  <div>
                    <dt>Frecuencia</dt>
                    <dd>{(3.2 + (cpu / 100) * 1.4).toFixed(2)} GHz</dd>
                  </div>
                  <div>
                    <dt>Procesos</dt>
                    <dd>287</dd>
                  </div>
                  <div>
                    <dt>Núcleos</dt>
                    <dd>8 · 16 hilos</dd>
                  </div>
                </dl>
              </div>
              <div style={{ marginTop: 10 }}>
                <Sparkline datos={historialCpu} max={100} etiqueta="Uso de CPU en los últimos 60 segundos" />
              </div>
            </div>
          </Vidrio>
        )}

        {minimizado && !cerrado && (
          <div>
            <Pildora icono={ICONO_CPU} texto={`CPU ${Math.round(cpu)} %`} onRestaurar={() => setMinimizado(false)} />
          </div>
        )}

        {cerrado && (
          <Vidrio className="gd-widget" style={{ paddingTop: 14 }}>
            <p className="gd-etiqueta">Widget CPU cerrado.</p>
            <button type="button" className="gd-boton gd-boton--sm" style={{ marginTop: 10 }} onClick={() => setCerrado(false)}>
              Reactivar
            </button>
          </Vidrio>
        )}

        <Vidrio elemento="section" className="gd-widget" aria-label="Widget Disco">
          <BarraWidget titulo="Disco" icono={ICONO_DISCO} fijado={false} onPin={() => undefined} onMinimizar={() => undefined} onMaximizar={() => undefined} onCerrar={() => undefined} />
          <div className="gd-widget__cuerpo">
            <Barra etiqueta="C:" detalle="Sistema · NVMe" usado={612} total={953} textoValor="612 / 953 GB" />
            <Barra etiqueta="D:" detalle="Datos · HDD" usado={1.12} total={1.81} textoValor="1.12 / 1.81 TB" />
            <Barra etiqueta="E:" detalle="Respaldo" usado={1.75} total={1.81} textoValor="1.75 / 1.81 TB" />
          </div>
        </Vidrio>

        <Vidrio elemento="section" className="gd-widget" aria-label="Widget Red">
          <BarraWidget titulo="Red" icono={ICONO_RED} fijado onPin={() => undefined} onMinimizar={() => undefined} onMaximizar={() => undefined} onCerrar={() => undefined} />
          <div className="gd-widget__cuerpo gd-galeria__pila">
            <div>
              <span className="gd-etiqueta">Bajada</span>
              <b className="num" style={{ display: 'block', fontSize: 17 }}>
                {ultimaBajada.toFixed(1)} MB/s
              </b>
            </div>
            <Sparkline datos={bajada} alto={30} max={16} />
            <Sparkline datos={HISTORIAL_HORA[1]?.datos.slice(-61) ?? []} alto={30} variante="secundario" />
          </div>
        </Vidrio>

        <Vidrio elemento="section" className="gd-widget" aria-label="Medidores por umbral">
          <BarraWidget titulo="Umbrales" icono={ICONO_CPU} fijado={false} onPin={() => undefined} onMinimizar={() => undefined} onMaximizar={() => undefined} onCerrar={() => undefined} />
          <div className="gd-widget__cuerpo gd-galeria__fila" style={{ justifyContent: 'space-between' }}>
            <Medidor valor={45} etiqueta="Normal" subetiqueta="normal" tamano={70} grosor={7} />
            <Medidor valor={78} etiqueta="Aviso" subetiqueta="aviso" tamano={70} grosor={7} />
            <Medidor valor={95} etiqueta="Peligro" subetiqueta="peligro" tamano={70} grosor={7} />
          </div>
        </Vidrio>

        <Vidrio elemento="section" className="gd-widget gd-galeria__doble" aria-label="Controles">
          <BarraWidget titulo="Controles" icono={ICONO_LISTA} fijado={false} onPin={() => undefined} onMinimizar={() => undefined} onMaximizar={() => undefined} onCerrar={() => undefined} />
          <div className="gd-widget__cuerpo gd-galeria__pila">
            <div className="gd-galeria__fila">
              <span className="gd-etiqueta">Top 5 por consumo</span>
              <div style={{ marginLeft: 'auto' }}>
                <Selector etiqueta="Ordenar procesos" opciones={OPCIONES_ORDEN} valor={orden} onCambiar={setOrden} />
              </div>
            </div>
            <Interruptor
              etiqueta="Arrancar con Windows"
              descripcion="Se inicia minimizado en la bandeja"
              activo={arranque}
              onCambiar={setArranque}
            />
            <Interruptor etiqueta="Excluir widgets del editor de zonas" activo={false} onCambiar={() => undefined} deshabilitado />
            <div className="gd-galeria__fila">
              <button type="button" className="gd-boton">Restablecer</button>
              <button type="button" className="gd-boton gd-boton--primario">Listo</button>
              <button type="button" className="gd-boton gd-boton--peligro" onClick={() => setDialogo(true)}>
                Cerrar procesos…
              </button>
              <button
                type="button"
                className="gd-boton gd-boton--sm"
                onPointerDown={() => setArrastrando(true)}
                onPointerUp={() => setArrastrando(false)}
                onPointerLeave={() => setArrastrando(false)}
              >
                Mantén para simular arrastre
              </button>
            </div>
            <div className="gd-galeria__fila">
              <span className="gd-chip">Opcional</span>
              <span className="gd-chip gd-chip--acento">Gmail · personal</span>
              <span className="gd-chip gd-chip--aviso">no oficial</span>
              <span className="gd-chip gd-chip--ok">Conectada</span>
              <span className="gd-chip gd-chip--peligro">Error</span>
            </div>
            {mensaje && (
              <p className="gd-etiqueta" role="status">
                {mensaje}
              </p>
            )}
          </div>
        </Vidrio>

        <Vidrio elemento="section" className="gd-widget gd-galeria__ancho" aria-label="Historial">
          <BarraWidget titulo="Red · historial 1 h (3600 puntos)" icono={ICONO_RED} fijado={false} onPin={() => undefined} onMinimizar={() => undefined} onMaximizar={() => undefined} onCerrar={() => undefined} />
          <div className="gd-widget__cuerpo">
            <GraficaHistorial series={HISTORIAL_HORA} etiqueta="Historial de red de la última hora" alto={220} max={16} />
          </div>
        </Vidrio>

        <Vidrio elemento="section" className="gd-widget gd-galeria__ancho" aria-label="Uso por núcleo">
          <BarraWidget titulo="CPU · uso por núcleo" icono={ICONO_CPU} fijado={false} onPin={() => undefined} onMinimizar={() => undefined} onMaximizar={() => undefined} onCerrar={() => undefined} />
          <div className="gd-widget__cuerpo">
            <GraficaMultiples series={NUCLEOS} etiqueta="Uso por núcleo" columnas={4} />
          </div>
        </Vidrio>

        <div className="gd-galeria__ventana gd-galeria__ancho">
          <Vidrio className="gd-widget" lleno style={{ minHeight: 300, paddingTop: 14 }}>
            <p className="gd-etiqueta">El diálogo se dibuja dentro de la ventana del widget.</p>
            <button type="button" className="gd-boton gd-boton--peligro" style={{ marginTop: 10 }} onClick={() => setDialogo(true)}>
              Abrir diálogo de confirmación
            </button>
          </Vidrio>
          <Dialogo
            abierto={dialogo}
            peligro
            titulo="¿Cerrar 3 procesos?"
            descripcion="Esta acción puede hacer que pierdas trabajo sin guardar."
            elementos={PROCESOS}
            aviso={
              <>
                <b>Incluye Cursor.exe.</b> Tiene confirmación reforzada: podrías perder cambios sin guardar. Escribe <b>CERRAR</b> para continuar.
              </>
            }
            confirmacionEscrita="CERRAR"
            textoConfirmar="Cerrar 3 procesos"
            onCancelar={() => setDialogo(false)}
            onConfirmar={() => {
              setDialogo(false);
              setMensaje('3 procesos cerrados (simulado)');
            }}
          >
            <div className="gd-galeria__fila" style={{ justifyContent: 'space-between', marginBottom: 12 }}>
              <span className="gd-etiqueta">Memoria a liberar</span>
              <b className="num" style={{ fontSize: 16 }}>
                ~3.5 GB
              </b>
            </div>
          </Dialogo>
        </div>
      </div>
    </main>
  );
}

export default Galeria;

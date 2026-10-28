import { useEffect, useState, type ReactNode } from 'react';
import {
  api,
  mensajeError,
  type AgendaCuenta,
  type AlertaRotativa,
  type BuzonCuenta,
  type CorreoNoLeido,
  type EstadoFuente,
  type EventoAgenda,
} from '../api';
import { horaCorta } from '../formato';
import { useAhora, useConfig, usePrivacidad, useServicio } from '../hooks';
import { IconoAnterior, IconoCalendario, IconoCorreo, IconoOjo, IconoOjoTachado, IconoSiguiente } from '../iconos';
import { EstadoLectura } from './EstadoLectura';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

const MAX_PUNTOS = 8;
const OCULTO = '••••••••';

export type ElementoAlerta =
  | { tipo: 'correo'; clave: string; cuenta: string; correoCuenta: string; correo: CorreoNoLeido }
  | { tipo: 'evento'; clave: string; cuenta: string; correoCuenta: string; evento: EventoAgenda };

/** GD-ROT-03: abre el hilo en la cuenta correcta aunque haya varias sesiones de Google en el navegador. */
export function urlCorreo(correoCuenta: string, hilo: string): string {
  return `https://mail.google.com/mail/u/${encodeURIComponent(correoCuenta).replace('%40', '@')}/#inbox/${encodeURIComponent(hilo)}`;
}

export function urlEvento(enlace: string, correoCuenta: string): string | null {
  try {
    const u = new URL(enlace);
    u.searchParams.set('authuser', correoCuenta);
    return u.toString();
  } catch {
    return null;
  }
}

export function urlDe(e: ElementoAlerta): string | null {
  if (e.tipo === 'correo') return urlCorreo(e.correoCuenta, e.correo.hilo);
  return e.evento.enlace ? urlEvento(e.evento.enlace, e.correoCuenta) : null;
}

/** Mismas claves que `elementos_rotables` del núcleo; los eventos ya terminados no se muestran. */
export function elementosAlerta(buzones: readonly BuzonCuenta[], agendas: readonly AgendaCuenta[], ahora: number): ElementoAlerta[] {
  const correos: ElementoAlerta[] = buzones.flatMap((b) =>
    b.mensajes.map((m) => ({ tipo: 'correo' as const, clave: `correo:${b.cuenta}:${m.id}`, cuenta: b.cuenta, correoCuenta: b.correo, correo: m })),
  );
  const eventos: ElementoAlerta[] = agendas
    .flatMap((a) =>
      a.eventos
        .filter((e) => e.fin_ms > ahora)
        .map((e) => ({ tipo: 'evento' as const, clave: `evento:${a.cuenta}:${e.id}`, cuenta: a.cuenta, correoCuenta: a.correo, evento: e })),
    )
    .sort((x, y) => (x.tipo === 'evento' && y.tipo === 'evento' ? x.evento.inicio_ms - y.evento.inicio_ms : 0));
  return [...eventos, ...correos];
}

function cuandoEvento(e: EventoAgenda, ahora: number): string {
  if (e.todo_el_dia) return 'Todo el día';
  if (e.inicio_ms <= ahora) return `Ahora · hasta ${horaCorta(e.fin_ms)}`;
  const min = Math.round((e.inicio_ms - ahora) / 60_000);
  if (min < 60) return `En ${min} min · ${horaCorta(e.inicio_ms)}`;
  return `${horaCorta(e.inicio_ms)} – ${horaCorta(e.fin_ms)}`;
}

function horaCorreo(ms: number, ahora: number): string {
  if (!ms) return '';
  const fecha = new Date(ms);
  if (fecha.toDateString() === new Date(ahora).toDateString()) return horaCorta(ms);
  return fecha.toLocaleDateString('es-MX', { day: 'numeric', month: 'short' }).replace('.', '');
}

/** Con privacidad el texto ni siquiera llega al DOM; `.gd-privado` además lo desenfoca si cambia a mitad de un render. */
function Privado({ texto, privado, como = 'span' }: { texto: string; privado: boolean; como?: 'span' | 'b' }) {
  const Etiqueta = como;
  return (
    <Etiqueta className="gd-privado" aria-hidden={privado || undefined}>
      {privado ? OCULTO : texto}
    </Etiqueta>
  );
}

function abrir(e: ElementoAlerta, alError: (m: string) => void): void {
  const url = urlDe(e);
  if (!url) {
    alError('Este evento no trae enlace de Google Calendar');
    return;
  }
  api.googleAbrir(url).catch((err) => alError(mensajeError(err)));
}

function etiquetaElemento(e: ElementoAlerta, privado: boolean, ahora: number): string {
  const destino = e.tipo === 'correo' ? 'Abrir en Gmail' : 'Abrir en Google Calendar';
  if (privado) return `${e.tipo === 'correo' ? 'Correo no leído' : 'Evento'} oculto por privacidad. ${destino}`;
  if (e.tipo === 'correo') return `Correo de ${e.correo.remitente}: ${e.correo.asunto}. ${destino}`;
  return `${e.evento.titulo}, ${cuandoEvento(e.evento, ahora)}. ${destino}`;
}

export function TarjetaAlerta({ e, privado, ahora, alError }: { e: ElementoAlerta; privado: boolean; ahora: number; alError: (m: string) => void }) {
  const esCorreo = e.tipo === 'correo';
  return (
    <button type="button" className="gd-correo__tarjeta" aria-label={etiquetaElemento(e, privado, ahora)} onClick={() => abrir(e, alError)}>
      <span className={`gd-correo__icono${esCorreo ? '' : ' gd-correo__icono--evento'}`} aria-hidden="true">
        {esCorreo ? <IconoCorreo /> : <IconoCalendario />}
      </span>
      <span className="gd-correo__texto" aria-hidden="true">
        <span className="gd-correo__de">
          <Privado como="b" texto={esCorreo ? e.correo.remitente : e.evento.titulo} privado={privado} />
          <time className="num">{esCorreo ? horaCorreo(e.correo.fecha_ms, ahora) : cuandoEvento(e.evento, ahora)}</time>
        </span>
        <span className="gd-correo__asunto">
          {esCorreo ? (
            <Privado texto={e.correo.asunto} privado={privado} />
          ) : e.evento.ubicacion ? (
            <Privado texto={e.evento.ubicacion} privado={privado} />
          ) : (
            'Google Calendar'
          )}
        </span>
      </span>
    </button>
  );
}

function Anillo({ fraccion }: { fraccion: number }) {
  const r = 7;
  const c = 2 * Math.PI * r;
  return (
    <svg className="gd-correo__anillo" viewBox="0 0 18 18" width="18" height="18" aria-hidden="true">
      <circle cx="9" cy="9" r={r} />
      <circle cx="9" cy="9" r={r} strokeDasharray={c} strokeDashoffset={c * (1 - Math.min(1, Math.max(0, fraccion)))} />
    </svg>
  );
}

function cuentaRegresiva(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
}

function Rotacion({ alerta, periodoMs, ahora, detalle }: { alerta: AlertaRotativa | null; periodoMs: number; ahora: number; detalle?: boolean }) {
  const minutos = Math.round(periodoMs / 60_000);
  const restante = alerta ? alerta.proxima_ms - ahora : periodoMs;
  return (
    <span className="gd-correo__rota" title={`La alerta rota cada ${minutos} min`}>
      <Anillo fraccion={1 - restante / periodoMs} />
      <span>
        rota cada {minutos} min · {detalle && 'siguiente en '}
        <b className="num">{cuentaRegresiva(restante)}</b>
      </span>
    </span>
  );
}

function resumenConteo(buzones: readonly BuzonCuenta[], eventos: number): string {
  const noLeidos = buzones.reduce((s, b) => s + b.no_leidos, 0);
  return `${noLeidos} no leído${noLeidos === 1 ? '' : 's'} · ${eventos} evento${eventos === 1 ? '' : 's'}`;
}

function CuentasConProblema({ buzones }: { buzones: readonly BuzonCuenta[] }) {
  const malas = buzones.filter((b) => b.estado === 'reconectar');
  if (malas.length === 0) return null;
  return (
    <p className="gd-aviso-linea gd-correo__reconectar" role="status">
      <span className="gd-chip gd-chip--aviso">Reconectar</span>
      <span>
        {malas.length === 1 ? 'Una cuenta perdió el acceso' : `${malas.length} cuentas perdieron el acceso`}; las demás siguen al día.
      </span>
      <button type="button" className="gd-boton gd-boton--sm" onClick={() => api.panelAbrir('cuentas').catch(() => undefined)}>
        Revisar
      </button>
    </p>
  );
}

export function VistaCorreo() {
  const maximizado = useMaximizado('correo');
  const [config] = useConfig();
  const privado = usePrivacidad(config);
  const correo = useServicio('correo');
  const agenda = useServicio('calendario');
  const alerta = useServicio('alerta')?.datos ?? null;
  const ahora = useAhora(1_000);
  const [indice, setIndice] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => setIndice(null), [alerta?.clave]);

  const buzones = correo?.datos?.cuentas ?? [];
  const agendas = agenda?.datos?.cuentas ?? [];
  const elementos = elementosAlerta(buzones, agendas, ahora);
  const delNucleo = elementos.findIndex((e) => e.clave === alerta?.clave);
  const actual = indice ?? (delNucleo >= 0 ? delNucleo : 0);
  const elemento = elementos[actual % Math.max(1, elementos.length)] ?? null;
  const eventos = elementos.filter((e) => e.tipo === 'evento').length;
  const periodo = config?.intervalos.rotacion_ms ?? 600_000;
  const atajo = (config?.atajos.privacidad ?? 'Super+Alt+P').replace('Super', 'Win');
  const sinCuentas = correo?.datos ? buzones.length === 0 : false;
  const noLeidos = buzones.reduce((s, b) => s + b.no_leidos, 0);
  const mover = (paso: number): void => {
    if (elementos.length === 0) return;
    setIndice((actual + paso + elementos.length) % elementos.length);
  };

  const botonPrivacidad = (
    <button
      type="button"
      className="gd-boton-icono"
      aria-pressed={privado}
      aria-label="Privacidad: ocultar remitente y asunto"
      title={`Privacidad (${atajo})`}
      onClick={() => api.privacidadAlternar().catch((e) => setError(mensajeError(e)))}
    >
      {privado ? <IconoOjoTachado /> : <IconoOjo />}
    </button>
  );

  const vacio = sinCuentas ? (
    <div className="gd-correo__vacio">
      <p className="gd-etiqueta">Conecta una o más cuentas de Google para ver tus no leídos y tu agenda.</p>
      <button type="button" className="gd-boton gd-boton--sm gd-boton--primario" onClick={() => api.panelAbrir('cuentas').catch(() => undefined)}>
        Conectar Google
      </button>
    </div>
  ) : (
    <p className="gd-etiqueta gd-correo__vacio">{correo?.datos ? 'Sin no leídos ni eventos próximos.' : 'Leyendo tus cuentas…'}</p>
  );

  const chipCuenta = elemento ? (
    <span className="gd-chip gd-chip--acento gd-correo__cuenta" title={privado ? undefined : elemento.correoCuenta}>
      Gmail · <Privado texto={elemento.correoCuenta.split('@')[0] ?? ''} privado={privado} />
    </span>
  ) : (
    <span className="gd-chip">Gmail</span>
  );

  const errorLinea = error && (
    <p className="gd-widget__error" role="alert">
      {error}
    </p>
  );

  return (
    <MarcoWidget
      id="correo"
      titulo="Correo y agenda"
      subtitulo="No leídos y eventos próximos · solo encabezados"
      icono={<IconoCorreo />}
      textoPildora={`${noLeidos} · ${eventos}`}
      maximizado={maximizado}
    >
      {maximizado ? (
        <Detalle
          buzones={buzones}
          elementos={elementos}
          elemento={elemento}
          privado={privado}
          ahora={ahora}
          alError={setError}
          cabecera={
            <>
              <Rotacion alerta={alerta} periodoMs={periodo} ahora={ahora} detalle />
              {botonPrivacidad}
            </>
          }
          vacio={vacio}
          pie={
            <>
              {errorLinea}
              <EstadoLectura estado={correo} ahora={ahora} />
            </>
          }
          agenda={agenda}
        />
      ) : (
        <div className="gd-vista gd-vista--pila gd-correo">
          <div className="gd-correo__arriba">
            {chipCuenta}
            <Rotacion alerta={alerta} periodoMs={periodo} ahora={ahora} />
            {botonPrivacidad}
          </div>
          {elemento ? <TarjetaAlerta e={elemento} privado={privado} ahora={ahora} alError={setError} /> : vacio}
          <div className="gd-correo__nav">
            <button type="button" className="gd-boton-icono" aria-label="Alerta anterior" disabled={elementos.length < 2} onClick={() => mover(-1)}>
              <IconoAnterior />
            </button>
            <span className="gd-correo__puntos" aria-hidden="true">
              {elementos.slice(0, MAX_PUNTOS).map((e, i) => (
                <i key={e.clave} className={i === actual ? 'gd-correo__punto--activo' : undefined} />
              ))}
            </span>
            <span className="gd-etiqueta num">{resumenConteo(buzones, eventos)}</span>
            <button type="button" className="gd-boton-icono" aria-label="Alerta siguiente" disabled={elementos.length < 2} onClick={() => mover(1)}>
              <IconoSiguiente />
            </button>
          </div>
          <CuentasConProblema buzones={buzones} />
          {errorLinea}
          <EstadoLectura estado={correo} ahora={ahora} />
        </div>
      )}
    </MarcoWidget>
  );
}

interface PropsDetalle {
  buzones: BuzonCuenta[];
  elementos: ElementoAlerta[];
  elemento: ElementoAlerta | null;
  privado: boolean;
  ahora: number;
  alError: (m: string) => void;
  cabecera: ReactNode;
  vacio: ReactNode;
  pie: ReactNode;
  agenda: EstadoFuente<{ cuentas: AgendaCuenta[] }> | null;
}

/** GD-MAIL-04: no leídos por cuenta y la agenda de todas las cuentas en orden. */
function Detalle({ buzones, elementos, elemento, privado, ahora, alError, cabecera, vacio, pie, agenda }: PropsDetalle) {
  const eventos = elementos.filter((e) => e.tipo === 'evento');
  return (
    <div className="gd-vista gd-vista--detalle gd-correo">
      <section className="gd-tarjeta">
        <h3 className="gd-correo__titulo">
          Alerta actual
          {cabecera}
        </h3>
        {elemento ? <TarjetaAlerta e={elemento} privado={privado} ahora={ahora} alError={alError} /> : vacio}
      </section>
      <CuentasConProblema buzones={buzones} />
      <div className="gd-rejilla-2">
        <section className="gd-tarjeta">
          <h3 className="gd-correo__titulo">
            No leídos
            <span className="gd-chip">
              {buzones.length} cuenta{buzones.length === 1 ? '' : 's'}
            </span>
          </h3>
          {buzones.map((b) => (
            <div key={b.cuenta} className="gd-correo__bloque" role="group" aria-label={privado ? 'Cuenta de Google' : `Cuenta ${b.correo}`}>
              <p className="gd-correo__cabecera">
                <Privado como="b" texto={b.correo} privado={privado} />
                <span className={`gd-chip num${b.estado === 'reconectar' ? ' gd-chip--aviso' : ''}`}>
                  {b.estado === 'reconectar' ? 'Reconectar' : `${b.no_leidos} no leídos`}
                </span>
              </p>
              <ul className="gd-correo__lista">
                {elementos
                  .filter((e) => e.tipo === 'correo' && e.cuenta === b.cuenta)
                  .map((e) => (
                    <li key={e.clave}>
                      <TarjetaAlerta e={e} privado={privado} ahora={ahora} alError={alError} />
                    </li>
                  ))}
                {b.mensajes.length === 0 && b.estado === 'conectada' && <li className="gd-etiqueta">Sin no leídos.</li>}
              </ul>
            </div>
          ))}
          {buzones.length === 0 && vacio}
        </section>
        <section className="gd-tarjeta">
          <h3 className="gd-correo__titulo">Agenda · hoy y próximas 24 h</h3>
          <ol className="gd-correo__lista">
            {eventos.map((e) => (
              <li key={e.clave}>
                <TarjetaAlerta e={e} privado={privado} ahora={ahora} alError={alError} />
              </li>
            ))}
            {eventos.length === 0 && <li className="gd-etiqueta">Sin eventos próximos.</li>}
          </ol>
          <EstadoLectura estado={agenda} ahora={ahora} />
        </section>
      </div>
      <p className="gd-aviso-linea">Glassdeck solo lee remitente, asunto y hora. Nunca descarga ni muestra el cuerpo de los correos.</p>
      {pie}
    </div>
  );
}

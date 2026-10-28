import type { ReactNode } from 'react';
import type { CostoMes, DatosChatgpt, DatosClaudeAi, DatosCursor, EstadoCuentas, EstadoFuente, ServicioWeb, VentanaUso } from '../api';
import { Barra } from '../componentes';
import { horaCorta, porcentaje, usd } from '../formato';
import { useServicio } from '../hooks';
import { estadoServicio, NOMBRE_SERVICIO, PuntoCuenta, textoEstadoCuenta, useCuentasWeb, type EstadoCuenta } from './Cuentas';
import { EstadoLectura } from './EstadoLectura';

/** Cursor, claude.ai y ChatGPT (no oficiales) y las APIs de costos con clave de administrador (bloques 13 y 17). */
export interface DatosWeb {
  cursor: EstadoFuente<DatosCursor> | null;
  claude_ai: EstadoFuente<DatosClaudeAi> | null;
  chatgpt: EstadoFuente<DatosChatgpt> | null;
  anthropic_consola: EstadoFuente<CostoMes> | null;
  openai_api: EstadoFuente<CostoMes> | null;
  cuentas: EstadoCuentas | null;
}

export const WEB_SIN_DATOS: DatosWeb = { cursor: null, claude_ai: null, chatgpt: null, anthropic_consola: null, openai_api: null, cuentas: null };

export function useDatosWeb(): DatosWeb {
  const [cuentas] = useCuentasWeb();
  return {
    cursor: useServicio('cursor'),
    claude_ai: useServicio('claude_ai'),
    chatgpt: useServicio('chatgpt'),
    anthropic_consola: useServicio('anthropic_consola'),
    openai_api: useServicio('openai_api'),
    cuentas,
  };
}

const LECTURA_CORTADA = new Set(['sesion_expirada', 'cloudflare']);

/** Sin el estado del núcleo, el propio resultado de la lectura dice si hay sesión y si sigue vigente. */
function estadoFila(servicio: ServicioWeb, fuente: EstadoFuente<unknown> | null, cuentas: EstadoCuentas | null): EstadoCuenta {
  if (cuentas) return estadoServicio(servicio, null, cuentas);
  if (fuente?.error && LECTURA_CORTADA.has(fuente.error)) return 'reconectar';
  return fuente?.datos ? 'conectada' : 'sin_conectar';
}

/** Hora de reinicio de una ventana de límite: hoy solo la hora; otro día, también el día. */
export function textoReinicio(reinicio: string | null, ahora: number): string | null {
  if (!reinicio) return null;
  const ms = Date.parse(reinicio);
  if (!Number.isFinite(ms)) return null;
  const mismoDia = new Date(ms).toDateString() === new Date(ahora).toDateString();
  if (mismoDia) return `hoy ${horaCorta(ms)}`;
  const dia = new Date(ms).toLocaleDateString('es-MX', { weekday: 'short', day: 'numeric' }).replace('.', '');
  return `${dia} ${horaCorta(ms)}`;
}

function fechaCorte(fecha: string | null): string | null {
  if (!fecha) return null;
  const ms = Date.parse(fecha);
  return Number.isFinite(ms) ? new Date(ms).toLocaleDateString('es-MX', { day: 'numeric', month: 'short' }).replace('.', '') : null;
}

export function resumenCursor(d: DatosCursor): string {
  if (d.gasto_usd !== null) return d.limite_usd ? `${usd(d.gasto_usd)} de ${usd(d.limite_usd)}` : usd(d.gasto_usd);
  if (d.solicitudes !== null) return d.solicitudes_limite ? `${d.solicitudes} de ${d.solicitudes_limite} solicitudes` : `${d.solicitudes} solicitudes`;
  return '—';
}

function resumenVentanas(d: { cinco_horas: VentanaUso | null; semanal: VentanaUso | null }): string {
  const partes = [d.cinco_horas && `5 h ${porcentaje(d.cinco_horas.porcentaje)}`, d.semanal && `sem ${porcentaje(d.semanal.porcentaje)}`].filter(Boolean);
  return partes.length > 0 ? partes.join(' · ') : '—';
}

interface Fila {
  clave: string;
  nombre: string;
  oficial: boolean;
  estado: EstadoCuenta;
  fuente: EstadoFuente<unknown> | null;
  valor: string | null;
}

function textoValor(f: Fila): string {
  if (f.estado === 'sin_conectar') return 'Sin conectar';
  return f.valor ?? (f.fuente?.error ? 'Sin datos válidos' : 'Esperando la primera lectura…');
}

function FilaCompacta({ fila }: { fila: Fila }) {
  const error = fila.estado !== 'sin_conectar' ? fila.fuente?.texto_error : null;
  return (
    <li className="gd-spending__remota" data-fuente={fila.clave}>
      <PuntoCuenta estado={fila.estado} />
      <b>{fila.nombre}</b>
      <span className={`gd-chip${fila.oficial ? '' : ' gd-chip--aviso'}`}>{fila.oficial ? 'API' : 'no oficial'}</span>
      <span className="gd-etiqueta gd-spending__remota-detalle num gd-privado" title={error ?? textoEstadoCuenta(fila.estado)}>
        {textoValor(fila)}
      </span>
      {error && <span className="gd-chip gd-chip--aviso gd-spending__motivo">{error}</span>}
    </li>
  );
}

function Ventana({ etiqueta, ventana, ahora }: { etiqueta: string; ventana: VentanaUso | null; ahora: number }) {
  if (!ventana) return null;
  const reinicio = textoReinicio(ventana.reinicio, ahora);
  return (
    <Barra
      className="gd-privado"
      usado={ventana.porcentaje}
      total={100}
      etiqueta={etiqueta}
      detalle={reinicio ? `se reinicia ${reinicio}` : undefined}
      textoValor={porcentaje(ventana.porcentaje)}
    />
  );
}

function Par({ nombre, valor }: { nombre: string; valor: string | null }) {
  if (valor === null) return null;
  return (
    <div>
      <dt>{nombre}</dt>
      <dd className="num gd-privado">{valor}</dd>
    </div>
  );
}

function TarjetaFuente({ fila, ahora, children }: { fila: Fila; ahora: number; children: ReactNode }) {
  return (
    <section className="gd-tarjeta gd-spending__web" aria-label={fila.nombre} data-fuente={fila.clave}>
      <h3>
        {fila.nombre} <span className={`gd-chip${fila.oficial ? '' : ' gd-chip--aviso'}`}>{fila.oficial ? 'API oficial' : 'no oficial'}</span>
        <span className="gd-cuenta__estado gd-etiqueta">
          <PuntoCuenta estado={fila.estado} />
          {textoEstadoCuenta(fila.estado)}
        </span>
      </h3>
      {fila.estado === 'sin_conectar' ? (
        <p className="gd-etiqueta">Sin conectar. Conéctalo desde Customize › Cuentas.</p>
      ) : (
        <>
          {children}
          <EstadoLectura estado={fila.fuente} ahora={ahora} />
        </>
      )}
    </section>
  );
}

export function SeccionesWeb({ web, maximizado, ahora }: { web: DatosWeb; maximizado: boolean; ahora: number }) {
  const cursor = web.cursor?.datos ?? null;
  const claudeAi = web.claude_ai?.datos ?? null;
  const chatgpt = web.chatgpt?.datos ?? null;
  const filasWeb: Fila[] = [
    { clave: 'cursor', fuente: web.cursor, valor: cursor && resumenCursor(cursor) },
    { clave: 'claude_ai', fuente: web.claude_ai, valor: claudeAi && resumenVentanas(claudeAi) },
    { clave: 'chatgpt', fuente: web.chatgpt, valor: chatgpt && resumenVentanas(chatgpt) },
  ].map((f) => ({
    ...f,
    nombre: NOMBRE_SERVICIO[f.clave as ServicioWeb],
    oficial: false,
    estado: estadoFila(f.clave as ServicioWeb, f.fuente, web.cuentas),
  }));
  const costo = (clave: string, nombre: string, fuente: EstadoFuente<CostoMes> | null): Fila => ({
    clave,
    nombre,
    oficial: true,
    estado: fuente?.error && LECTURA_CORTADA.has(fuente.error) ? 'reconectar' : 'conectada',
    fuente,
    valor: fuente?.datos ? `${usd(fuente.datos.costo_usd)} este mes` : null,
  });
  const filasApi = [
    web.cuentas?.claves.anthropic_admin && costo('anthropic_consola', 'Consola de Anthropic', web.anthropic_consola),
    web.cuentas?.claves.openai_admin && costo('openai_api', 'API de OpenAI', web.openai_api),
  ].filter((f): f is Fila => Boolean(f));

  if (!maximizado) {
    return (
      <ul className="gd-spending__remotas" aria-label="Otras fuentes">
        {[...filasWeb, ...filasApi].map((f) => (
          <FilaCompacta key={f.clave} fila={f} />
        ))}
      </ul>
    );
  }

  const [fCursor, fClaude, fChatgpt] = filasWeb as [Fila, Fila, Fila];
  return (
    <>
      <div className="gd-rejilla-3 gd-spending__fuentes" aria-label="Cursor, Claude.ai y ChatGPT">
        <TarjetaFuente fila={fCursor} ahora={ahora}>
          {cursor && (
            <dl className="gd-kv gd-lista-pares">
              <Par nombre="Gasto del ciclo" valor={cursor.gasto_usd !== null ? usd(cursor.gasto_usd) : null} />
              <Par nombre="Límite" valor={cursor.limite_usd !== null ? usd(cursor.limite_usd) : null} />
              <Par
                nombre="Incluido en el plan"
                valor={cursor.incluido_usado_usd !== null ? `${usd(cursor.incluido_usado_usd)}${cursor.incluido_limite_usd ? ` de ${usd(cursor.incluido_limite_usd)}` : ''}` : null}
              />
              <Par
                nombre="Bajo demanda"
                valor={cursor.bajo_demanda_usd !== null ? `${usd(cursor.bajo_demanda_usd)}${cursor.bajo_demanda_limite_usd ? ` de ${usd(cursor.bajo_demanda_limite_usd)}` : ''}` : null}
              />
              <Par nombre="Solicitudes" valor={cursor.solicitudes !== null ? resumenCursor({ ...cursor, gasto_usd: null }) : null} />
              <Par nombre="Fecha de corte" valor={fechaCorte(cursor.ciclo_fin)} />
            </dl>
          )}
        </TarjetaFuente>
        <TarjetaFuente fila={fClaude} ahora={ahora}>
          {claudeAi && (
            <>
              <Ventana etiqueta="Límite de 5 h" ventana={claudeAi.cinco_horas} ahora={ahora} />
              <Ventana etiqueta="Límite semanal" ventana={claudeAi.semanal} ahora={ahora} />
              {claudeAi.extras.map((x) => (
                <Ventana key={x.nombre} etiqueta={`Semanal · ${x.nombre}`} ventana={x} ahora={ahora} />
              ))}
            </>
          )}
        </TarjetaFuente>
        <TarjetaFuente fila={fChatgpt} ahora={ahora}>
          {chatgpt && (
            <>
              {chatgpt.plan && <p className="gd-etiqueta">Plan {chatgpt.plan}</p>}
              <Ventana etiqueta="Límite de 5 h" ventana={chatgpt.cinco_horas} ahora={ahora} />
              <Ventana etiqueta="Límite semanal" ventana={chatgpt.semanal} ahora={ahora} />
            </>
          )}
        </TarjetaFuente>
      </div>
      <p className="gd-etiqueta">
        Cursor, Claude.ai y ChatGPT se leen de sus páginas web porque no tienen API pública de uso: si cambian, el dato puede dejar de actualizarse.
      </p>
      {filasApi.length > 0 && (
        <div className="gd-rejilla-2" aria-label="APIs con clave de administrador">
          {filasApi.map((f) => (
            <TarjetaFuente key={f.clave} fila={f} ahora={ahora}>
              <p className="gd-spending__monto num gd-privado">{f.valor ?? '—'}</p>
            </TarjetaFuente>
          ))}
        </div>
      )}
    </>
  );
}

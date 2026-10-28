import type { EstadoFuente, FilaClaude, ResumenClaudeCode, Tokens } from '../api';
import { GraficaBarras } from '../componentes';
import { diaCorto, tokens, usd } from '../formato';
import { useAhora, useConfig, usePrivacidad, useServicio } from '../hooks';
import { IconoCartera } from '../iconos';
import { EstadoLectura } from './EstadoLectura';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

export const totalTokens = (t: Tokens): number => t.entrada + t.salida + t.cache_escritura + t.cache_lectura;

export interface FilaDesglose {
  nombre: string;
  tokens: number;
  costo_usd: number;
}

export function agruparFilas(filas: readonly FilaClaude[], por: 'modelo' | 'proyecto'): FilaDesglose[] {
  const mapa = new Map<string, FilaDesglose>();
  for (const f of filas) {
    const nombre = f[por] || 'Sin nombre';
    const actual = mapa.get(nombre) ?? { nombre, tokens: 0, costo_usd: 0 };
    actual.tokens += totalTokens(f.tokens);
    actual.costo_usd += f.costo_usd;
    mapa.set(nombre, actual);
  }
  return [...mapa.values()].sort((a, b) => b.costo_usd - a.costo_usd || b.tokens - a.tokens);
}

export function VistaSpending() {
  const maximizado = useMaximizado();
  const [config] = useConfig();
  const privado = usePrivacidad(config);
  const claude = useServicio('claude_code');
  const ahora = useAhora();
  const hoy = claude?.datos?.total_hoy;
  return (
    <MarcoWidget
      id="spending"
      titulo="Spending"
      subtitulo="Gasto y uso de Cursor y Claude"
      icono={<IconoCartera />}
      textoPildora={privado ? 'oculto' : hoy ? `≈ ${usd(hoy.costo_usd)} hoy` : '—'}
      maximizado={maximizado}
    >
      <ContenidoSpending claude={claude} maximizado={maximizado} ahora={ahora} />
    </MarcoWidget>
  );
}

interface PropsContenido {
  claude: EstadoFuente<ResumenClaudeCode> | null;
  maximizado: boolean;
  ahora: number;
  /** GD-CLA-05: la sección de la consola solo existe con clave configurada; su lectura llega en el bloque 13. */
  consolaConfigurada?: boolean;
}

function FuenteRemota({ nombre, detalle }: { nombre: string; detalle: string }) {
  return (
    <li className="gd-spending__remota">
      <i className="gd-punto gd-punto--sin_conectar" aria-hidden="true" />
      <b>{nombre}</b>
      <span className="gd-chip gd-chip--aviso">no oficial</span>
      <span className="gd-etiqueta gd-spending__remota-detalle" title={detalle}>
        Sin conectar
      </span>
    </li>
  );
}

function Kpi({ etiqueta, valor, detalle, chip }: { etiqueta: string; valor: string; detalle?: string; chip?: string }) {
  return (
    <div className="gd-kpi">
      <small>
        {etiqueta} {chip && <span className="gd-chip gd-chip--aviso">{chip}</span>}
      </small>
      <b className="num gd-privado">{valor}</b>
      {detalle && <span className="gd-etiqueta num gd-privado">{detalle}</span>}
    </div>
  );
}

function TablaDesglose({ titulo, filas, nombreColumna }: { titulo: string; filas: FilaDesglose[]; nombreColumna: string }) {
  return (
    <section className="gd-tarjeta">
      <h3>{titulo}</h3>
      {filas.length === 0 ? (
        <p className="gd-etiqueta">Sin uso en este periodo.</p>
      ) : (
        <table className="gd-tabla gd-tabla--compacta" aria-label={titulo}>
          <thead>
            <tr>
              <th>{nombreColumna}</th>
              <th className="gd-tabla__num">Tokens</th>
              <th className="gd-tabla__num">Costo equiv.</th>
            </tr>
          </thead>
          <tbody className="gd-privado">
            {filas.map((f) => (
              <tr key={f.nombre}>
                <td className="gd-tabla__nombre" title={f.nombre}>
                  {f.nombre}
                </td>
                <td className="gd-tabla__num num">{tokens(f.tokens)}</td>
                <td className="gd-tabla__num num">≈ {usd(f.costo_usd)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

export function ContenidoSpending({ claude, maximizado, ahora, consolaConfigurada = false }: PropsContenido) {
  const d = claude?.datos ?? null;

  if (!maximizado) {
    return (
      <div className="gd-vista gd-vista--pila gd-spending">
        <section className="gd-spending__claude" aria-label="Claude Code">
          <header className="gd-spending__cabecera">
            <h3>Claude Code</h3>
            <span className="gd-chip">local</span>
          </header>
          {d ? (
            <dl className="gd-spending__cifras">
              <div>
                <dt>Hoy</dt>
                <dd className="gd-spending__monto num gd-privado">≈ {usd(d.total_hoy.costo_usd)}</dd>
                <dd className="gd-etiqueta num gd-privado">{tokens(totalTokens(d.total_hoy.tokens))} tokens</dd>
              </div>
              <div>
                <dt>Este mes</dt>
                <dd className="gd-spending__monto num gd-privado">≈ {usd(d.total_mes.costo_usd)}</dd>
                <dd className="gd-etiqueta num gd-privado">{tokens(totalTokens(d.total_mes.tokens))} tokens</dd>
              </div>
            </dl>
          ) : (
            <p className="gd-etiqueta">Sin datos de Claude Code todavía.</p>
          )}
        </section>
        <ul className="gd-spending__remotas" aria-label="Otras fuentes">
          <FuenteRemota nombre="Cursor" detalle="Gasto del ciclo actual" />
          <FuenteRemota nombre="Claude.ai" detalle="Límites de 5 h y semanal" />
        </ul>
        {consolaConfigurada && <p className="gd-etiqueta">Consola de la API: sin datos todavía.</p>}
        <EstadoLectura estado={claude} ahora={ahora} />
      </div>
    );
  }

  const dias = d?.dias ?? [];
  const eje = dias.length > 0 ? [diaCorto(dias[0]!.dia), diaCorto(dias[Math.floor(dias.length / 2)]!.dia), 'hoy'] : [];
  const hoy = d?.total_hoy.tokens;

  return (
    <div className="gd-vista gd-vista--detalle gd-spending">
      <div className="gd-kpis">
        <Kpi
          etiqueta="Claude Code hoy (equivalente)"
          valor={d ? `≈ ${usd(d.total_hoy.costo_usd)}` : '—'}
          detalle={d ? `${tokens(totalTokens(d.total_hoy.tokens))} tokens` : undefined}
        />
        <Kpi
          etiqueta="Claude Code este mes"
          valor={d ? `≈ ${usd(d.total_mes.costo_usd)}` : '—'}
          detalle={d ? `${tokens(totalTokens(d.total_mes.tokens))} tokens` : undefined}
        />
        <Kpi etiqueta="Cursor este mes" chip="no oficial" valor="Sin conectar" />
        <Kpi etiqueta="Claude.ai · 5 h / semanal" chip="no oficial" valor="Sin conectar" />
      </div>

      <div className="gd-rejilla-2">
        <section className="gd-tarjeta">
          <h3>Claude Code · costo equivalente por día</h3>
          {dias.length > 0 ? (
            <GraficaBarras
              className="gd-privado"
              valores={dias.map((x) => x.costo_usd)}
              etiqueta={`Costo equivalente diario de Claude Code, últimos ${dias.length} días`}
              etiquetasEje={eje}
            />
          ) : (
            <p className="gd-etiqueta">Sin datos todavía.</p>
          )}
        </section>
        <section className="gd-tarjeta">
          <h3>Claude Code hoy</h3>
          <dl className="gd-kv gd-lista-pares">
            <div>
              <dt>Entrada</dt>
              <dd className="gd-privado">{hoy ? `${tokens(hoy.entrada)} tokens` : '—'}</dd>
            </div>
            <div>
              <dt>Salida</dt>
              <dd className="gd-privado">{hoy ? `${tokens(hoy.salida)} tokens` : '—'}</dd>
            </div>
            <div>
              <dt>Caché (escritura)</dt>
              <dd className="gd-privado">{hoy ? `${tokens(hoy.cache_escritura)} tokens` : '—'}</dd>
            </div>
            <div>
              <dt>Caché (lectura)</dt>
              <dd className="gd-privado">{hoy ? `${tokens(hoy.cache_lectura)} tokens` : '—'}</dd>
            </div>
            <div>
              <dt>Costo equivalente</dt>
              <dd className="gd-privado">{d ? `≈ ${usd(d.total_hoy.costo_usd)}` : '—'}</dd>
            </div>
          </dl>
        </section>
      </div>

      <div className="gd-rejilla-2">
        <TablaDesglose titulo="Por modelo · últimos 30 días" nombreColumna="Modelo" filas={agruparFilas(d?.filas ?? [], 'modelo')} />
        <TablaDesglose titulo="Por proyecto · últimos 30 días" nombreColumna="Proyecto" filas={agruparFilas(d?.filas ?? [], 'proyecto')} />
      </div>

      <section className="gd-tarjeta gd-spending__fuentes" aria-label="Cursor y Claude.ai">
        <h3>Cursor y Claude.ai</h3>
        <ul className="gd-spending__remotas">
          <FuenteRemota nombre="Cursor" detalle="Gasto del ciclo actual" />
          <FuenteRemota nombre="Claude.ai" detalle="Límites de 5 h y semanal" />
        </ul>
        <p className="gd-etiqueta">Se conectan desde Customize › Cuentas. Se leen de sus páginas web porque no tienen API pública de uso.</p>
      </section>

      {consolaConfigurada && (
        <section className="gd-tarjeta" aria-label="Consola de la API">
          <h3>Consola de la API</h3>
          <p className="gd-etiqueta">Clave guardada; el costo real del mes aparecerá con la siguiente lectura.</p>
        </section>
      )}

      {d && d.sin_precio.length > 0 && (
        <p className="gd-aviso-linea" role="note">
          Sin precio en la tabla (cuentan $0): {d.sin_precio.join(', ')}. Agrégalos en Customize › Gasto y avisos.
        </p>
      )}
      <p className="gd-etiqueta">
        El costo de Claude Code es un equivalente calculado con tu tabla de precios, no un cargo real.
      </p>
      <EstadoLectura estado={claude} ahora={ahora} />
    </div>
  );
}

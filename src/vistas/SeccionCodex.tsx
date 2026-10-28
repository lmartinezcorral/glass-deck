import type { EstadoFuente, FilaCodex, ResumenCodex } from '../api';
import { GraficaBarras } from '../componentes';
import { diaCorto, tokens, usd } from '../formato';
import { EstadoLectura } from './EstadoLectura';

export const NOTA_SIN_DESGLOSE = 'sin desglose, no se calcula costo';

export interface ModeloCodex {
  modelo: string;
  sesiones: number;
  tokens: number;
  costo_usd: number | null;
}

export function agruparPorModelo(filas: readonly FilaCodex[]): ModeloCodex[] {
  const mapa = new Map<string, ModeloCodex>();
  for (const f of filas) {
    const actual = mapa.get(f.modelo) ?? { modelo: f.modelo, sesiones: 0, tokens: 0, costo_usd: null };
    actual.sesiones += f.sesiones;
    actual.tokens += f.tokens;
    if (f.costo_usd !== null) actual.costo_usd = (actual.costo_usd ?? 0) + f.costo_usd;
    mapa.set(f.modelo, actual);
  }
  return [...mapa.values()].sort((a, b) => b.tokens - a.tokens);
}

const conCosto = (costo: number | null): string => (costo === null ? '' : ` · ≈ ${usd(costo)}`);

interface Props {
  codex: EstadoFuente<ResumenCodex> | null;
  maximizado: boolean;
  ahora: number;
}

/** GD-OAI-01..03: tokens locales de Codex; el costo solo aparece si los registros traen desglose y hay precio. */
export function SeccionCodex({ codex, maximizado, ahora }: Props) {
  const d = codex?.datos ?? null;
  const nota = d && !d.con_desglose ? NOTA_SIN_DESGLOSE : null;

  if (!maximizado) {
    return (
      <section className="gd-spending__codex" aria-label="Codex">
        <header className="gd-spending__cabecera">
          <h3>Codex</h3>
          <span className="gd-chip">local</span>
          {nota && (
            <span className="gd-etiqueta gd-spending__nota" title={nota}>
              {nota}
            </span>
          )}
        </header>
        {d ? (
          <p className="gd-spending__linea num gd-privado">
            Hoy {tokens(d.total_hoy.tokens)}
            {conCosto(d.total_hoy.costo_usd)} · mes {tokens(d.total_mes.tokens)} tokens{conCosto(d.total_mes.costo_usd)}
          </p>
        ) : (
          <p className="gd-etiqueta">Sin datos de Codex todavía.</p>
        )}
        {codex?.error && <EstadoLectura estado={codex} ahora={ahora} />}
      </section>
    );
  }

  const dias = d?.dias ?? [];
  const eje = dias.length > 0 ? [diaCorto(dias[0]!.dia), diaCorto(dias[Math.floor(dias.length / 2)]!.dia), 'hoy'] : [];
  const modelos = agruparPorModelo(d?.filas ?? []);
  const hayCosto = modelos.some((m) => m.costo_usd !== null);

  return (
    <section className="gd-tarjeta gd-spending__codex" aria-label="Codex">
      <h3>Codex · tokens por día</h3>
      <div className="gd-kpis gd-kpis--2">
        <div className="gd-kpi">
          <small>Codex hoy</small>
          <b className="num gd-privado">{d ? `${tokens(d.total_hoy.tokens)} tokens` : '—'}</b>
          {d?.total_hoy.costo_usd != null && <span className="gd-etiqueta num gd-privado">≈ {usd(d.total_hoy.costo_usd)}</span>}
        </div>
        <div className="gd-kpi">
          <small>Codex este mes</small>
          <b className="num gd-privado">{d ? `${tokens(d.total_mes.tokens)} tokens` : '—'}</b>
          {d?.total_mes.costo_usd != null && <span className="gd-etiqueta num gd-privado">≈ {usd(d.total_mes.costo_usd)}</span>}
        </div>
      </div>
      {dias.length > 0 ? (
        <GraficaBarras
          className="gd-privado"
          valores={dias.map((x) => x.tokens)}
          etiqueta={`Tokens diarios de Codex, últimos ${dias.length} días`}
          etiquetasEje={eje}
        />
      ) : (
        <p className="gd-etiqueta">Sin datos todavía.</p>
      )}
      {modelos.length === 0 ? (
        <p className="gd-etiqueta">Sin uso de Codex en los últimos 30 días.</p>
      ) : (
        <table className="gd-tabla gd-tabla--compacta" aria-label="Codex por modelo · últimos 30 días">
          <thead>
            <tr>
              <th>Modelo</th>
              <th className="gd-tabla__num">Sesiones</th>
              <th className="gd-tabla__num">Tokens</th>
              {hayCosto && <th className="gd-tabla__num">Costo equiv.</th>}
            </tr>
          </thead>
          <tbody className="gd-privado">
            {modelos.map((m) => (
              <tr key={m.modelo}>
                <td className="gd-tabla__nombre" title={m.modelo}>
                  {m.modelo}
                </td>
                <td className="gd-tabla__num num">{m.sesiones}</td>
                <td className="gd-tabla__num num">{tokens(m.tokens)}</td>
                {hayCosto && <td className="gd-tabla__num num">{m.costo_usd === null ? '—' : `≈ ${usd(m.costo_usd)}`}</td>}
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {nota && (
        <p className="gd-etiqueta" role="note">
          Codex Desktop solo registra el total de tokens: {nota}.
        </p>
      )}
      {d && d.sin_precio.length > 0 && (
        <p className="gd-aviso-linea" role="note">
          Sin precio en la tabla (sin costo): {d.sin_precio.join(', ')}. Agrégalos en Customize › Gasto y avisos.
        </p>
      )}
      {codex?.error && <EstadoLectura estado={codex} ahora={ahora} />}
    </section>
  );
}

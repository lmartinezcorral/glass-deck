import type { CSSProperties } from 'react';
import { api, type Clima, type EstadoFuente } from '../api';
import { diaCorto, grados } from '../formato';
import { useAhora, useServicio } from '../hooks';
import { IconoClima, IconoSolNube } from '../iconos';
import { EstadoLectura } from './EstadoLectura';
import { MarcoWidget, useMaximizado } from './MarcoWidget';

const HORAS_NORMAL = 5;

export function VistaClima() {
  const maximizado = useMaximizado('clima');
  const clima = useServicio('clima');
  const ahora = useAhora();
  const actual = clima?.datos?.actual;
  return (
    <MarcoWidget
      id="clima"
      titulo="Clima"
      subtitulo="Clima y pronóstico de tu ubicación"
      icono={<IconoSolNube />}
      textoPildora={actual ? grados(actual.temperatura) : '—'}
      maximizado={maximizado}
    >
      <ContenidoClima clima={clima} maximizado={maximizado} ahora={ahora} />
    </MarcoWidget>
  );
}

const lugarCompleto = (l: Clima['lugar']): string => [l.nombre, l.region, l.pais].filter(Boolean).join(', ');
const horaDe = (iso: string): string => iso.slice(11, 16) || iso;
const porcentaje = (v: number | null): string => (v === null ? '—' : `${Math.round(v)} %`);

function Lugar({ lugar }: { lugar: Clima['lugar'] }) {
  return (
    <p className="gd-clima__lugar" title={lugarCompleto(lugar)}>
      <b>{lugar.nombre}</b>
      {lugar.automatica && <span className="gd-chip">automática</span>}
    </p>
  );
}

function Horas({ horas, todas }: { horas: Clima['horas']; todas: boolean }) {
  const lista = todas ? horas : horas.slice(0, HORAS_NORMAL);
  return (
    <ol
      className={`gd-clima__horas${todas ? ' gd-clima__horas--todas' : ''}`}
      aria-label={todas ? `Pronóstico por hora, próximas ${lista.length} horas` : 'Próximas horas'}
      tabIndex={todas ? 0 : undefined}
    >
      {lista.map((h) => (
        <li key={h.hora}>
          <span className="gd-etiqueta num">{horaDe(h.hora)}</span>
          <IconoClima icono={h.icono} dia={h.es_dia} tamano={todas ? 22 : 18} />
          <b className="num">{grados(h.temperatura)}</b>
          {todas && <span className="gd-etiqueta num gd-clima__lluvia">{porcentaje(h.prob_lluvia)}</span>}
        </li>
      ))}
    </ol>
  );
}

function Dias({ dias }: { dias: Clima['dias'] }) {
  const minimos = dias.map((d) => d.minima).filter((v): v is number => v !== null);
  const maximos = dias.map((d) => d.maxima).filter((v): v is number => v !== null);
  const piso = minimos.length ? Math.min(...minimos) : 0;
  const techo = maximos.length ? Math.max(...maximos) : 1;
  const rango = techo - piso || 1;
  return (
    <ol className="gd-clima__dias" aria-label={`Pronóstico de ${dias.length} días`}>
      {dias.map((d, i) => {
        const desde = d.minima === null ? 0 : ((d.minima - piso) / rango) * 100;
        const hasta = d.maxima === null ? 100 : ((d.maxima - piso) / rango) * 100;
        return (
          <li key={d.fecha}>
            <b className="gd-clima__dia">{i === 0 ? 'Hoy' : diaCorto(d.fecha)}</b>
            <IconoClima icono={d.icono} tamano={22} />
            <span className="gd-clima__descripcion">{d.descripcion}</span>
            <span className="gd-etiqueta num gd-clima__lluvia" title="Probabilidad de lluvia">
              {porcentaje(d.prob_lluvia)}
            </span>
            <span className="num gd-clima__min">{grados(d.minima)}</span>
            <span className="gd-clima__rango" aria-hidden="true">
              <i style={{ '--desde': `${desde}%`, '--hasta': `${hasta}%` } as CSSProperties} />
            </span>
            <span className="num">{grados(d.maxima)}</span>
          </li>
        );
      })}
    </ol>
  );
}

export function ContenidoClima({ clima, maximizado, ahora }: { clima: EstadoFuente<Clima> | null; maximizado: boolean; ahora: number }) {
  const d = clima?.datos ?? null;
  if (!d) {
    return (
      <div className="gd-vista gd-vista--pila gd-clima">
        <p className="gd-etiqueta">Sin datos del clima todavía.</p>
        <EstadoLectura estado={clima} ahora={ahora} />
      </div>
    );
  }
  const a = d.actual;
  const condicion = `${a.descripcion}${a.es_dia ? '' : ' · noche'}`;

  if (!maximizado) {
    return (
      <div className="gd-vista gd-vista--pila gd-clima">
        <div className="gd-clima__actual">
          <div className="gd-clima__texto">
            <Lugar lugar={d.lugar} />
            <p className="gd-clima__temp num">{grados(a.temperatura)}</p>
            <p className="gd-etiqueta">{condicion}</p>
          </div>
          <IconoClima icono={a.icono} dia={a.es_dia} tamano={52} />
        </div>
        <dl className="gd-clima__extremos">
          <div>
            <dt>Máx</dt>
            <dd className="num">{grados(a.maxima)}</dd>
          </div>
          <div>
            <dt>Mín</dt>
            <dd className="num">{grados(a.minima)}</dd>
          </div>
          <div>
            <dt>Humedad</dt>
            <dd className="num">{porcentaje(a.humedad)}</dd>
          </div>
        </dl>
        <Horas horas={d.horas} todas={false} />
        <EstadoLectura estado={clima} ahora={ahora} />
      </div>
    );
  }

  return (
    <div className="gd-vista gd-vista--detalle gd-clima">
      <div className="gd-rejilla-2">
        <section className="gd-tarjeta gd-clima__actual">
          <div className="gd-clima__texto">
            <Lugar lugar={d.lugar} />
            <p className="gd-clima__temp gd-clima__temp--grande num">{grados(a.temperatura)}</p>
            <p className="gd-etiqueta">
              {condicion}
              {a.sensacion !== null && ` · sensación ${grados(a.sensacion)}`}
            </p>
          </div>
          <IconoClima icono={a.icono} dia={a.es_dia} tamano={84} />
        </section>
        <section className="gd-tarjeta">
          <div className="gd-kpis gd-kpis--2">
            <div className="gd-kpi">
              <small>Máx / Mín</small>
              <b className="num">
                {grados(a.maxima)} / {grados(a.minima)}
              </b>
            </div>
            <div className="gd-kpi">
              <small>Humedad</small>
              <b className="num">{porcentaje(a.humedad)}</b>
            </div>
            <div className="gd-kpi">
              <small>Viento</small>
              <b className="num">{a.viento_kmh === null ? '—' : `${Math.round(a.viento_kmh)} km/h`}</b>
            </div>
            <div className="gd-kpi">
              <small>Lluvia hoy</small>
              <b className="num">{porcentaje(d.dias[0]?.prob_lluvia ?? null)}</b>
            </div>
          </div>
          <div className="gd-panel__acciones">
            <span className="gd-etiqueta" title={lugarCompleto(d.lugar)}>
              {d.lugar.automatica ? 'Ubicación aproximada por IP' : lugarCompleto(d.lugar)}
            </span>
            <button
              type="button"
              className="gd-boton gd-boton--sm"
              onClick={() => api.panelAbrir('clima').catch(() => undefined)}
            >
              Cambiar ubicación
            </button>
          </div>
        </section>
      </div>
      <section className="gd-tarjeta">
        <h3>Próximas horas</h3>
        <Horas horas={d.horas} todas />
      </section>
      <section className="gd-tarjeta">
        <h3>Próximos días</h3>
        <Dias dias={d.dias} />
      </section>
      <EstadoLectura estado={clima} ahora={ahora} />
    </div>
  );
}

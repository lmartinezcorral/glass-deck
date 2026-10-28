import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Clima, DatosChatgpt, DatosClaudeAi, DatosCursor, EstadoFuente, FilaClaude, ResumenClaudeCode, ServicioWeb, Tokens } from '../api';
import baseCss from '../tema/base.css?raw';
import vistasCss from './vistas.css?raw';
import { ContenidoClima } from './Clima';
import { WEB_SIN_DATOS, type DatosWeb } from './SeccionesWeb';
import { agruparFilas, ContenidoSpending, totalTokens } from './Spending';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const AHORA = new Date(2026, 9, 28, 12, 0).getTime();
const MIN = 60_000;

const tk = (entrada: number, salida = 0, cache_escritura = 0, cache_lectura = 0): Tokens => ({ entrada, salida, cache_escritura, cache_lectura });

const fila = (dia: string, modelo: string, proyecto: string, entrada: number, costo_usd: number): FilaClaude => ({
  dia,
  modelo,
  proyecto,
  tokens: tk(entrada),
  costo_usd,
});

function resumen(): ResumenClaudeCode {
  const filas = [
    fila('2026-10-28', 'claude-opus-5', 'proyecto-a', 2_000_000, 10),
    fila('2026-10-28', 'claude-sonnet-5', 'proyecto-b', 1_000_000, 3),
    fila('2026-10-20', 'claude-opus-5', 'proyecto-b', 500_000, 2.5),
  ];
  const dias = Array.from({ length: 30 }, (_, i) => {
    const d = new Date(2026, 9, 28 - (29 - i));
    const dia = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
    return { dia, tokens: tk(i * 1000), costo_usd: i === 29 ? 13 : i === 21 ? 2.5 : 0 };
  });
  return {
    hoy: '2026-10-28',
    total_hoy: { tokens: tk(3_000_000), costo_usd: 13 },
    total_mes: { tokens: tk(3_500_000), costo_usd: 15.5 },
    dias,
    filas,
    sin_precio: [],
  };
}

const ok = <T,>(datos: T, hora_ms = AHORA - 3 * MIN): EstadoFuente<T> => ({ datos, hora_ms, error: null, texto_error: null, proximo_intento_ms: null });

function clima(): Clima {
  return {
    lugar: { nombre: 'Ciudad de prueba', region: 'Región', pais: 'País', automatica: true },
    zona_horaria: 'UTC',
    actual: {
      hora: '2026-10-28T12:00',
      temperatura: 21.4,
      sensacion: 20,
      humedad: 40,
      viento_kmh: 12,
      codigo: 2,
      icono: 'parcial',
      descripcion: 'Parcialmente nublado',
      es_dia: true,
      maxima: 25,
      minima: 9,
    },
    horas: Array.from({ length: 24 }, (_, i) => ({
      hora: `2026-10-28T${String((12 + i) % 24).padStart(2, '0')}:00`,
      temperatura: 20 - (i % 6),
      codigo: 0,
      icono: 'despejado' as const,
      prob_lluvia: i,
      es_dia: i < 6,
    })),
    dias: Array.from({ length: 7 }, (_, i) => ({
      fecha: `2026-10-${String(28 + i > 31 ? 28 + i - 31 : 28 + i).padStart(2, '0')}`,
      maxima: 25 - i,
      minima: 9 + (i % 3),
      codigo: 61,
      icono: 'lluvia' as const,
      descripcion: 'Lluvia',
      prob_lluvia: 10 * i,
    })),
  };
}

let contenedor: HTMLDivElement;
let raiz: Root;

beforeEach(() => {
  contenedor = document.createElement('div');
  document.body.appendChild(contenedor);
  raiz = createRoot(contenedor);
});

afterEach(() => {
  act(() => raiz.unmount());
  contenedor.remove();
});

const texto = (): string => contenedor.textContent ?? '';

describe('Spending: Claude Code local (GD-CLA-01, GD-CLA-02)', () => {
  it('muestra costo y tokens de hoy y del mes con la hora de la última lectura', () => {
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={false} ahora={AHORA} />));
    expect(texto()).toContain('≈ $13.00');
    expect(texto()).toContain('≈ $15.50');
    expect(texto()).toContain('3.00 M tokens');
    expect(texto()).toContain('Leído hace 3 min');
  });

  it('desglosa por modelo y por proyecto en la vista maximizada', () => {
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado ahora={AHORA} />));
    const modelo = contenedor.querySelector('table[aria-label^="Por modelo"]')!;
    const proyecto = contenedor.querySelector('table[aria-label^="Por proyecto"]')!;
    const nombres = (t: Element) => [...t.querySelectorAll('tbody .gd-tabla__nombre')].map((c) => c.textContent);
    expect(nombres(modelo)).toEqual(['claude-opus-5', 'claude-sonnet-5']);
    expect(nombres(proyecto)).toEqual(['proyecto-a', 'proyecto-b']);
    expect(contenedor.querySelectorAll('.gd-barras__barra')).toHaveLength(30);
    expect(contenedor.querySelector('.gd-barras__barra--hoy')).not.toBeNull();
  });

  it('agrupa sumando tokens y costo, ordenado por costo', () => {
    const g = agruparFilas(resumen().filas, 'proyecto');
    expect(g.map((x) => [x.nombre, x.tokens, x.costo_usd])).toEqual([
      ['proyecto-a', 2_000_000, 10],
      ['proyecto-b', 1_500_000, 5.5],
    ]);
    expect(totalTokens(tk(1, 2, 3, 4))).toBe(10);
  });

  it('con error muestra el motivo y la última lectura válida, sin inventar números (GD-CUR-06)', () => {
    const estado: EstadoFuente<ResumenClaudeCode> = {
      datos: resumen(),
      hora_ms: AHORA - 90 * MIN,
      error: 'formato_cambio',
      texto_error: 'Formato no reconocido',
      proximo_intento_ms: AHORA + 5 * MIN,
    };
    act(() => raiz.render(<ContenidoSpending claude={estado} maximizado={false} ahora={AHORA} />));
    expect(texto()).toContain('Formato no reconocido');
    expect(texto()).toContain('Último dato válido hace 1 h');
    expect(texto()).toContain('reintento a las');
    expect(texto()).toContain('≈ $13.00');
  });

  it('sin ninguna lectura válida no muestra cifras', () => {
    const estado: EstadoFuente<ResumenClaudeCode> = { datos: null, hora_ms: null, error: 'sin_conexion', texto_error: 'Sin conexión', proximo_intento_ms: null };
    act(() => raiz.render(<ContenidoSpending claude={estado} maximizado={false} ahora={AHORA} />));
    expect(texto()).toContain('Sin datos de Claude Code todavía.');
    expect(texto()).toContain('Sin datos válidos todavía');
    expect(texto()).not.toMatch(/\$\d/);
  });

  it('Cursor, Claude.ai y ChatGPT aparecen «Sin conectar» con la etiqueta «no oficial» en ambas vistas (GD-CUR-05)', () => {
    for (const maximizado of [false, true]) {
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} />));
      const chips = [...contenedor.querySelectorAll('.gd-chip')].filter((c) => c.textContent === 'no oficial');
      expect(chips.length).toBeGreaterThanOrEqual(3);
      expect(texto()).toContain('Cursor');
      expect(texto()).toContain('Claude.ai');
      expect(texto()).toContain('ChatGPT');
      expect(texto()).toContain('Sin conectar');
    }
  });

  it('las secciones de la consola de Anthropic y de la API de OpenAI solo existen con su clave (GD-CLA-05, GD-OAI-05)', () => {
    const conClaves = (anthropic_admin: boolean, openai_admin: boolean): DatosWeb => ({
      ...WEB_SIN_DATOS,
      anthropic_consola: anthropic_admin ? ok({ mes: '2026-10', costo_usd: 42.5 }) : null,
      openai_api: openai_admin ? ok({ mes: '2026-10', costo_usd: 7.25 }) : null,
      cuentas: { web: [], claves: { anthropic_admin, openai_admin } },
    });
    for (const maximizado of [false, true]) {
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} />));
      expect(texto()).not.toContain('Consola de Anthropic');
      expect(texto()).not.toContain('API de OpenAI');
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} web={conClaves(true, false)} />));
      expect(texto()).toContain('Consola de Anthropic');
      expect(texto()).toContain('$42.50 este mes');
      expect(texto()).not.toContain('API de OpenAI');
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} web={conClaves(false, true)} />));
      expect(texto()).not.toContain('Consola de Anthropic');
      expect(texto()).toContain('API de OpenAI');
      expect(texto()).toContain('$7.25 este mes');
    }
  });

  it('las cifras quedan dentro de .gd-privado para el modo privacidad', () => {
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado ahora={AHORA} />));
    const montos = [...contenedor.querySelectorAll('*')].filter((e) => e.children.length === 0 && /\$\d/.test(e.textContent ?? ''));
    expect(montos.length).toBeGreaterThan(0);
    for (const m of montos) expect(m.closest('.gd-privado')).not.toBeNull();
    expect(vistasCss).toMatch(/:root\[data-privacidad\] \.gd-privado\s*\{[^}]*blur/);
  });

  it('avisa de los modelos sin precio en la vista maximizada', () => {
    const r = { ...resumen(), sin_precio: ['modelo-nuevo'] };
    act(() => raiz.render(<ContenidoSpending claude={ok(r)} maximizado ahora={AHORA} />));
    expect(texto()).toContain('modelo-nuevo');
  });
});

const CURSOR_PRUEBA: DatosCursor = {
  ciclo: '2026-10',
  ciclo_inicio: '2026-10-05T00:00:00Z',
  ciclo_fin: '2026-11-05T00:00:00Z',
  plan: 'pro',
  gasto_usd: 32.4,
  limite_usd: 100,
  incluido_usado_usd: 20,
  incluido_limite_usd: 20,
  bajo_demanda_usd: 12.4,
  bajo_demanda_limite_usd: 80,
  solicitudes: null,
  solicitudes_limite: null,
};

const enHoras = (h: number): string => new Date(AHORA + h * 60 * MIN).toISOString();

const CLAUDE_AI_PRUEBA: DatosClaudeAi = {
  cinco_horas: { porcentaje: 41, reinicio: enHoras(2) },
  semanal: { porcentaje: 63, reinicio: enHoras(72) },
  extras: [{ nombre: 'Opus', porcentaje: 12, reinicio: null }],
};

const CHATGPT_PRUEBA: DatosChatgpt = {
  plan: 'plus',
  cinco_horas: { porcentaje: 88, reinicio: enHoras(1) },
  semanal: { porcentaje: 25, reinicio: null },
};

const conectadas = (motivo: Partial<Record<ServicioWeb, string>> = {}): DatosWeb['cuentas'] => ({
  web: (['cursor', 'claude_ai', 'chatgpt'] as const).map((servicio) => ({
    servicio,
    estado: motivo[servicio] ? 'reconectar' : 'conectada',
    motivo: motivo[servicio] ?? null,
    conectando: false,
  })),
  claves: { anthropic_admin: false, openai_admin: false },
});

describe('Spending: Cursor, claude.ai y ChatGPT (GD-CUR-04, GD-CUR-05, GD-CLA-04, GD-OAI-04)', () => {
  const web = (): DatosWeb => ({
    ...WEB_SIN_DATOS,
    cursor: ok(CURSOR_PRUEBA),
    claude_ai: ok(CLAUDE_AI_PRUEBA),
    chatgpt: ok(CHATGPT_PRUEBA),
    cuentas: conectadas(),
  });

  it('vista normal: una fila por servicio con su resumen y la etiqueta «no oficial»', () => {
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={false} ahora={AHORA} web={web()} />));
    const fila = (f: string) => contenedor.querySelector(`.gd-spending__remota[data-fuente="${f}"]`)!;
    expect(fila('cursor').textContent).toContain('$32.40 de $100.00');
    expect(fila('claude_ai').textContent).toContain('5 h 41 % · sem 63 %');
    expect(fila('chatgpt').textContent).toContain('5 h 88 % · sem 25 %');
    for (const f of ['cursor', 'claude_ai', 'chatgpt']) expect(fila(f).textContent).toContain('no oficial');
    expect(texto()).not.toContain('Sin conectar');
  });

  it('vista maximizada: desglose de Cursor y barras de las ventanas con su reinicio', () => {
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado ahora={AHORA} web={web()} />));
    const tarjeta = (f: string) => contenedor.querySelector(`.gd-spending__web[data-fuente="${f}"]`)!;
    expect(tarjeta('cursor').textContent).toContain('Bajo demanda');
    expect(tarjeta('cursor').textContent).toContain('$12.40 de $80.00');
    expect(tarjeta('cursor').textContent).toContain('Fecha de corte');
    expect(tarjeta('claude_ai').textContent).toContain('Límite de 5 h');
    expect(tarjeta('claude_ai').textContent).toContain('Semanal · Opus');
    expect(tarjeta('claude_ai').textContent).toContain('se reinicia hoy');
    expect(tarjeta('chatgpt').textContent).toContain('Plan plus');
    expect(tarjeta('chatgpt').textContent).toContain('88 %');
    for (const f of ['cursor', 'claude_ai', 'chatgpt']) expect(tarjeta(f).textContent).toContain('no oficial');
  });

  it('con sesión vencida o Cloudflare muestra el motivo y la última lectura válida (GD-CUR-06, GD-CLA-06, GD-OAI-06)', () => {
    const datos: DatosWeb = {
      ...web(),
      cursor: { ...ok(CURSOR_PRUEBA, AHORA - 90 * MIN), error: 'sesion_expirada', texto_error: 'Sesión expirada', proximo_intento_ms: null },
      chatgpt: { ...ok(CHATGPT_PRUEBA, AHORA - 90 * MIN), error: 'cloudflare', texto_error: 'Cloudflare bloqueó la lectura', proximo_intento_ms: null },
      cuentas: conectadas({ cursor: 'Sesión expirada', chatgpt: 'Cloudflare bloqueó la lectura' }),
    };
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={false} ahora={AHORA} web={datos} />));
    const cursor = contenedor.querySelector('.gd-spending__remota[data-fuente="cursor"]')!;
    expect(cursor.querySelector('.gd-spending__motivo')?.textContent).toBe('Sesión expirada');
    expect(cursor.querySelector('.gd-punto--reconectar')).not.toBeNull();
    expect(cursor.textContent).toContain('$32.40');
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado ahora={AHORA} web={datos} />));
    const chatgpt = contenedor.querySelector('.gd-spending__web[data-fuente="chatgpt"]')!;
    expect(chatgpt.textContent).toContain('Cloudflare bloqueó la lectura');
    expect(chatgpt.textContent).toContain('Último dato válido hace 1 h');
    expect(chatgpt.textContent).toContain('Requiere reconexión');
  });

  it('las cifras de las fuentes web quedan dentro de .gd-privado', () => {
    act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado ahora={AHORA} web={web()} />));
    const valores = [...contenedor.querySelectorAll('.gd-spending__fuentes *')].filter((e) => e.children.length === 0 && /\$\d|\d %/.test(e.textContent ?? ''));
    expect(valores.length).toBeGreaterThan(0);
    for (const v of valores) expect(v.closest('.gd-privado')).not.toBeNull();
  });
});

describe('Clima (GD-WX-01, GD-WX-02, GD-WX-03)', () => {
  it('vista normal: temperatura, icono, máxima, mínima, ciudad y 5 horas', () => {
    act(() => raiz.render(<ContenidoClima clima={ok(clima())} maximizado={false} ahora={AHORA} />));
    expect(texto()).toContain('Ciudad de prueba');
    expect(texto()).toContain('21°');
    expect(texto()).toContain('25°');
    expect(texto()).toContain('9°');
    expect(contenedor.querySelector('.gd-clima__actual .gd-clima-icono')).not.toBeNull();
    expect(contenedor.querySelectorAll('.gd-clima__horas li')).toHaveLength(5);
  });

  it('vista maximizada: 24 horas y 7 días', () => {
    act(() => raiz.render(<ContenidoClima clima={ok(clima())} maximizado ahora={AHORA} />));
    expect(contenedor.querySelectorAll('ol[aria-label^="Pronóstico por hora"] > li')).toHaveLength(24);
    expect(contenedor.querySelectorAll('ol[aria-label="Pronóstico de 7 días"] > li')).toHaveLength(7);
    expect(contenedor.querySelector('ol[aria-label="Pronóstico de 7 días"] > li')?.textContent).toContain('Hoy');
  });

  it('sin red conserva el último dato y dice de qué hora es', () => {
    const estado: EstadoFuente<Clima> = {
      datos: clima(),
      hora_ms: AHORA - 45 * MIN,
      error: 'sin_conexion',
      texto_error: 'Sin conexión',
      proximo_intento_ms: null,
    };
    act(() => raiz.render(<ContenidoClima clima={estado} maximizado={false} ahora={AHORA} />));
    expect(texto()).toContain('21°');
    expect(texto()).toContain('Sin conexión');
    expect(texto()).toMatch(/Último dato válido hace 45 min \(\d{2}:\d{2}\)/);
  });

  it('sin datos no inventa temperaturas', () => {
    act(() => raiz.render(<ContenidoClima clima={null} maximizado={false} ahora={AHORA} />));
    expect(texto()).toContain('Sin datos del clima todavía.');
    expect(texto()).not.toMatch(/\d°/);
  });

  it('los controles de la vista maximizada se alcanzan con Tab y el foco es visible', () => {
    act(() => raiz.render(<ContenidoClima clima={ok(clima())} maximizado ahora={AHORA} />));
    const enfocables = [...contenedor.querySelectorAll<HTMLElement>('button, [tabindex]')].filter((e) => e.tabIndex >= 0 && !(e as HTMLButtonElement).disabled);
    expect(enfocables.map((e) => e.getAttribute('aria-label') ?? e.textContent)).toEqual([
      'Cambiar ubicación',
      'Pronóstico por hora, próximas 24 horas',
    ]);
    for (const e of enfocables) {
      e.focus();
      expect(document.activeElement).toBe(e);
    }
    expect(baseCss).toMatch(/:focus-visible\s*\{[^}]*outline:[^;]*var\(--gd-foco\)/);
  });
});

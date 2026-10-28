import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Clima, EstadoFuente, FilaClaude, ResumenClaudeCode, ResumenCodex, Tokens } from '../api';
import baseCss from '../tema/base.css?raw';
import vistasCss from './vistas.css?raw';
import { ContenidoClima } from './Clima';
import { agruparPorModelo, NOTA_SIN_DESGLOSE } from './SeccionCodex';
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

  it('Cursor y Claude.ai aparecen «Sin conectar» con la etiqueta «no oficial» en ambas vistas (GD-CUR-05)', () => {
    for (const maximizado of [false, true]) {
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} />));
      const chips = [...contenedor.querySelectorAll('.gd-chip')].filter((c) => c.textContent === 'no oficial');
      expect(chips.length).toBeGreaterThanOrEqual(2);
      expect(texto()).toContain('Cursor');
      expect(texto()).toContain('Claude.ai');
      expect(texto()).toContain('Sin conectar');
    }
  });

  it('la sección de la consola de la API no existe sin clave (GD-CLA-05)', () => {
    for (const maximizado of [false, true]) {
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} />));
      expect(texto()).not.toContain('Consola de la API');
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} maximizado={maximizado} ahora={AHORA} consolaConfigurada />));
      expect(texto()).toContain('Consola de la API');
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

function resumenCodex(): ResumenCodex {
  const dias = Array.from({ length: 30 }, (_, i) => {
    const d = new Date(2026, 9, 28 - (29 - i));
    const dia = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
    return { dia, tokens: i === 29 ? 290_000 : i === 28 ? 7_000 : 0, costo_usd: null };
  });
  return {
    hoy: '2026-10-28',
    total_hoy: { tokens: 290_000, costo_usd: null },
    total_mes: { tokens: 1_297_000, costo_usd: null },
    dias,
    filas: [
      { dia: '2026-10-27', modelo: 'gpt-5.1-codex-mini', sesiones: 1, tokens: 7_000, costo_usd: null },
      { dia: '2026-10-28', modelo: 'gpt-5.1-codex-mini', sesiones: 2, tokens: 250_000, costo_usd: null },
      { dia: '2026-10-28', modelo: 'sin modelo', sesiones: 1, tokens: 40_000, costo_usd: null },
    ],
    con_desglose: false,
    sin_precio: [],
  };
}

describe('Spending: Codex local (GD-OAI-01, GD-OAI-03)', () => {
  it('muestra tokens de hoy y del mes con la nota sin costo y sin ningún monto', () => {
    for (const maximizado of [false, true]) {
      act(() => raiz.render(<ContenidoSpending claude={null} codex={ok(resumenCodex())} maximizado={maximizado} ahora={AHORA} />));
      const seccion = contenedor.querySelector('section[aria-label="Codex"]')!;
      expect(seccion.textContent).toContain('290 k');
      expect(seccion.textContent).toContain('1.30 M');
      expect(seccion.textContent).toContain(NOTA_SIN_DESGLOSE);
      expect(seccion.textContent).not.toMatch(/\$\d/);
    }
  });

  it('maximizado: gráfica de 30 días y desglose por modelo con «sin modelo»', () => {
    act(() => raiz.render(<ContenidoSpending claude={null} codex={ok(resumenCodex())} maximizado ahora={AHORA} />));
    const seccion = contenedor.querySelector('section[aria-label="Codex"]')!;
    expect(seccion.querySelectorAll('.gd-barras__barra')).toHaveLength(30);
    const tabla = seccion.querySelector('table[aria-label^="Codex por modelo"]')!;
    expect([...tabla.querySelectorAll('tbody .gd-tabla__nombre')].map((c) => c.textContent)).toEqual(['gpt-5.1-codex-mini', 'sin modelo']);
    expect(tabla.querySelectorAll('thead th')).toHaveLength(3);
    for (const celda of seccion.querySelectorAll('b.num, tbody')) expect(celda.closest('.gd-privado')).not.toBeNull();
  });

  it('con error muestra el motivo y la última lectura válida sin perder los tokens', () => {
    const estado: EstadoFuente<ResumenCodex> = {
      datos: resumenCodex(),
      hora_ms: AHORA - 20 * MIN,
      error: 'interno',
      texto_error: 'Error interno',
      proximo_intento_ms: AHORA + MIN,
    };
    for (const maximizado of [false, true]) {
      act(() => raiz.render(<ContenidoSpending claude={ok(resumen())} codex={estado} maximizado={maximizado} ahora={AHORA} />));
      const seccion = contenedor.querySelector('section[aria-label="Codex"]')!;
      expect(seccion.textContent).toContain('Error interno');
      expect(seccion.textContent).toContain('Último dato válido hace 20 min');
      expect(seccion.textContent).toContain('290 k');
    }
  });

  it('sin lectura no inventa cifras', () => {
    act(() => raiz.render(<ContenidoSpending claude={null} codex={null} maximizado={false} ahora={AHORA} />));
    expect(contenedor.querySelector('section[aria-label="Codex"]')!.textContent).toContain('Sin datos de Codex todavía.');
  });

  it('si una versión de Codex trae desglose y el modelo tiene precio, muestra el costo y quita la nota', () => {
    const r = resumenCodex();
    const conPrecio: ResumenCodex = {
      ...r,
      con_desglose: true,
      total_hoy: { tokens: 290_000, costo_usd: 1.5 },
      filas: r.filas.map((f) => ({ ...f, costo_usd: f.modelo === 'sin modelo' ? null : 0.75 })),
      sin_precio: ['sin modelo'],
    };
    act(() => raiz.render(<ContenidoSpending claude={null} codex={ok(conPrecio)} maximizado ahora={AHORA} />));
    const seccion = contenedor.querySelector('section[aria-label="Codex"]')!;
    expect(seccion.textContent).toContain('≈ $1.50');
    expect(seccion.textContent).not.toContain(NOTA_SIN_DESGLOSE);
    expect(seccion.textContent).toContain('Sin precio en la tabla (sin costo): sin modelo');
    expect(agruparPorModelo(conPrecio.filas)).toEqual([
      { modelo: 'gpt-5.1-codex-mini', sesiones: 3, tokens: 257_000, costo_usd: 1.5 },
      { modelo: 'sin modelo', sesiones: 1, tokens: 40_000, costo_usd: null },
    ]);
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

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Config, Lectura } from '../api';
import componentesCss from '../componentes/componentes.css?raw';
import baseCss from '../tema/base.css?raw';
import tokensCss from '../tema/tokens.css?raw';
import vistasCss from './vistas.css?raw';
import { VistaCpu } from './Cpu';
import { VistaCustomize } from './Customize';
import { VistaDisco } from './Disco';
import { VistaDock } from './Dock';
import { VistaLimpieza } from './Limpieza';
import { VistaProcesos } from './Procesos';
import { VistaRam } from './Ram';
import { VistaRed } from './Red';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
globalThis.ResizeObserver ??= class {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
};

const tauri = vi.hoisted(() => ({
  oyentes: new Map<string, Set<(e: { payload: unknown }) => void>>(),
  respuestas: {} as Record<string, unknown>,
}));

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => true,
  invoke: vi.fn(async (comando: string) => tauri.respuestas[comando]),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (evento: string, manejar: (e: { payload: unknown }) => void) => {
    if (!tauri.oyentes.has(evento)) tauri.oyentes.set(evento, new Set());
    tauri.oyentes.get(evento)!.add(manejar);
    return () => tauri.oyentes.get(evento)?.delete(manejar);
  }),
}));

const widget = { activo: true, estado: 'normal', fijado: false, monitor: null, x: null, y: null, ancho: 240, alto: 200 } as const;
const CONFIG: Config = {
  version: 2,
  widgets: Object.fromEntries(['cpu', 'ram', 'disco', 'red', 'procesos'].map((id) => [id, { ...widget }])),
  dock: { visible: true, x: null, y: null, monitor: null },
  apariencia: { tema: 'oscuro', acento: '#22d3ee', opacidad: 0.44, desenfoque: 24 },
  intervalos: {
    cpu_ram_red_ms: 2000,
    procesos_ms: 3000,
    disco_ms: 5000,
    spending_ms: 900000,
    clima_ms: 1800000,
    correo_ms: 300000,
    calendario_ms: 600000,
    rotacion_ms: 600000,
  },
  lista_negra_usuario: ['programa-qa.exe'],
  atajos: { mostrar_ocultar: 'Ctrl+Alt+M', privacidad: 'Super+Alt+P' },
  arranque_windows: false,
  capa_defecto: 'escritorio',
  pausar_con_bateria: true,
  iman: false,
  privacidad: false,
  precios_modelos: [{ patron: 'sonnet', entrada: 3, salida: 15, cache_escritura: 3.75, cache_lectura: 0.3 }],
  clima: { modo: 'automatica', ubicacion: null },
  umbrales: { notificaciones: true, cursor: [50, 80, 100], claude_5h: [80], claude_semanal: [80], presupuesto_cursor_usd: null },
};

const LECTURAS: Lectura[] = [
  { fuente: 'cpu', datos: { total: 23, nucleos: [20, 26, 18, 30], nombre: 'CPU de prueba' } },
  { fuente: 'ram', datos: { total: 16e9, usada: 9e9, en_cache: 5e9, libre: 2e9, comprometida: 12e9, limite_comprometida: 24e9 } },
  { fuente: 'disco', datos: [{ unidad: 'C:', etiqueta: 'Local', total: 5e11, libre: 5e10, extraible: false, lectura_bps: 1e6, escritura_bps: 2e6 }] },
  { fuente: 'red', datos: { adaptador: 'Wi-Fi', tipo: 'Wi-Fi', ip: '10.0.0.2', bajada_bps: 1e5, subida_bps: 2e4 } },
  {
    fuente: 'procesos',
    datos: [
      { pid: 100, nombre: 'notepad.exe', cpu: 3, ram: 3e7, usuario: 'usuario-a', nivel: 'normal', reiniciar_explorer: false },
      { pid: 200, nombre: 'explorer.exe', cpu: 1, ram: 9e7, usuario: 'usuario-a', nivel: 'normal', reiniciar_explorer: true },
      { pid: 300, nombre: 'lsass.exe', cpu: 0, ram: 1e7, usuario: null, nivel: 'protegido', reiniciar_explorer: false },
      { pid: 400, nombre: 'node.exe', cpu: 5, ram: 6e7, usuario: 'usuario-a', nivel: 'reforzado', reiniciar_explorer: false },
    ],
  },
];

Object.assign(tauri.respuestas, {
  config_leer: CONFIG,
  avisos_historial: [],
  boveda_estado: [],
  catalogo_widgets: ['cpu', 'ram', 'disco', 'red', 'procesos'].map((id) => ({ id, titulo: id.toUpperCase(), subtitulo: '', activo_defecto: true })),
  historial: [],
  procesos_fijos: ['lsass.exe', 'csrss.exe'],
  atajos_estado: [
    { accion: 'mostrar_ocultar', combinacion: 'Ctrl+Alt+M', registrado: true, error: null },
    { accion: 'privacidad', combinacion: 'Super+Alt+P', registrado: true, error: null },
  ],
  limpieza_vista_previa: [
    { id: 'temp_usuario', nombre: 'Temporales del usuario', bytes: 1e6, archivos: 3, requiere_admin: false, estimado: false, marcado_defecto: true },
    { id: 'temp_windows', nombre: 'Temporales de Windows', bytes: 2e6, archivos: 4, requiere_admin: true, estimado: true, marcado_defecto: true },
    { id: 'papelera', nombre: 'Papelera', bytes: 0, archivos: 0, requiere_admin: false, estimado: false, marcado_defecto: false },
  ],
  estado_entorno: { con_bateria: false, pausado: false, version: '0.1.0' },
});

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

async function emitir(evento: string, carga: unknown): Promise<void> {
  await act(async () => {
    for (const f of tauri.oyentes.get(evento) ?? []) f({ payload: carga });
  });
}

async function montar(vista: ReactNode, maximizado?: string): Promise<void> {
  await act(async () => raiz.render(vista));
  await act(async () => undefined);
  if (maximizado) await emitir('gd://maximizado', { id: maximizado, activo: true });
  for (const l of LECTURAS) await emitir('gd://datos', l);
}

const ENFOCABLE = 'button, input, select, textarea, a[href], [tabindex]';

function nombre(el: Element): string {
  const texto = el.getAttribute('aria-label') ?? el.getAttribute('title') ?? el.textContent?.trim().slice(0, 30) ?? '';
  return `${el.tagName.toLowerCase()}${el.className ? `.${String(el.className).split(' ')[0]}` : ''} «${texto}»`;
}

/** Secuencia de Tab del navegador: tabindex positivo primero, luego el orden del documento. */
function secuenciaTab(raizDom: HTMLElement): HTMLElement[] {
  const candidatos = [...raizDom.querySelectorAll<HTMLElement>(ENFOCABLE)].filter(
    (el) => el.tabIndex >= 0 && !(el as HTMLButtonElement).disabled && !el.closest('[hidden], [inert], [aria-hidden="true"]'),
  );
  const positivos = candidatos.filter((el) => el.tabIndex > 0).sort((a, b) => a.tabIndex - b.tabIndex);
  return [...positivos, ...candidatos.filter((el) => el.tabIndex === 0)];
}

function propsReact(el: Element): Record<string, unknown> | undefined {
  const clave = Object.keys(el).find((k) => k.startsWith('__reactProps$'));
  return clave ? ((el as unknown as Record<string, Record<string, unknown>>)[clave]) : undefined;
}

/**
 * Elementos con manejador de clic o teclado que no entran en la secuencia de Tab. Un grupo cuyo
 * único manejador es `onKeyDown` (flechas entre sus botones) cuenta si sus hijos son alcanzables.
 */
function inalcanzables(raizDom: HTMLElement, secuencia: HTMLElement[]): string[] {
  const enSecuencia = new Set<Element>(secuencia);
  return [...raizDom.querySelectorAll('*')]
    .filter((el) => {
      const p = propsReact(el);
      if (!p) return false;
      const manejadores = ['onClick', 'onKeyDown', 'onPointerDown', 'onMouseDown', 'onDoubleClick'].filter((k) => typeof p[k] === 'function');
      if (manejadores.length === 0 || enSecuencia.has(el) || (el as HTMLButtonElement).disabled || el.closest('label')) return false;
      const grupoConFlechas = manejadores.every((k) => k === 'onKeyDown') && secuencia.some((s) => el.contains(s));
      return !grupoConFlechas;
    })
    .map(nombre);
}

const REGLAS = [baseCss, tokensCss, componentesCss, vistasCss]
  .join('\n')
  .replace(/\/\*[\s\S]*?\*\//g, '')
  .matchAll(/([^{}@]+)\{([^{}]*)\}/g);
const LISTA_REGLAS = [...REGLAS].map((m) => ({ selector: m[1].trim(), decl: m[2] }));
const QUITA_CONTORNO = LISTA_REGLAS.filter((r) => /outline\s*:\s*(none|0)\b/.test(r.decl) && !/:focus/.test(r.selector));
const ANILLOS = LISTA_REGLAS.filter((r) => /:focus-visible/.test(r.selector) && /outline\s*:\s*(?!none|0\b)|box-shadow/.test(r.decl));

function coincide(el: Element, selector: string): boolean {
  try {
    return el.matches(selector.trim().replace(/::?(before|after)\b/g, ''));
  } catch {
    return false;
  }
}

/** El anillo global de `:focus-visible` aplica salvo que otra regla quite el contorno sin dar uno propio. */
function sinAnilloDeFoco(secuencia: HTMLElement[]): string[] {
  return secuencia
    .filter((el) => {
      const pierde = QUITA_CONTORNO.some((r) => r.selector.split(',').some((s) => coincide(el, s)));
      if (!pierde) return false;
      return !ANILLOS.some((r) => r.selector.split(',').some((s) => s.includes(':focus-visible') && coincide(el, s.replace(/:focus-visible/g, ''))));
    })
    .map(nombre);
}

interface Reporte {
  vista: string;
  enfocables: number;
  sinFoco: string[];
  inalcanzables: string[];
  sinAnillo: string[];
}

function revisar(vista: string): Reporte {
  const secuencia = secuenciaTab(contenedor);
  const sinFoco = secuencia
    .filter((el) => {
      act(() => el.focus());
      return document.activeElement !== el;
    })
    .map(nombre);
  return { vista, enfocables: secuencia.length, sinFoco, inalcanzables: inalcanzables(contenedor, secuencia), sinAnillo: sinAnilloDeFoco(secuencia) };
}

const WIDGETS: ReadonlyArray<[string, string, () => ReactNode]> = [
  ['CPU', 'cpu', () => <VistaCpu />],
  ['Memoria', 'ram', () => <VistaRam />],
  ['Disco', 'disco', () => <VistaDisco />],
  ['Red', 'red', () => <VistaRed />],
  ['Procesos', 'procesos', () => <VistaProcesos />],
];

describe('teclado y foco visible (GD-NFR-UX-02)', () => {
  it('el CSS dibuja un anillo de foco global con :focus-visible y no lo anula sin alternativa', () => {
    expect(baseCss).toMatch(/:focus-visible\s*\{\s*outline:\s*var\(--gd-foco-ancho\)\s+solid\s+var\(--gd-foco\)/);
    expect(LISTA_REGLAS.filter((r) => /:focus-visible/.test(r.selector) && /outline\s*:\s*(none|0)\b/.test(r.decl) && !/box-shadow/.test(r.decl))).toEqual([]);
  });

  for (const [titulo, id, crear] of WIDGETS) {
    for (const maximizado of [false, true]) {
      it(`${titulo}${maximizado ? ' maximizado' : ''}: todo control es alcanzable con Tab, recibe foco y tiene anillo`, async () => {
        await montar(crear(), maximizado ? id : undefined);
        const r = revisar(titulo);
        console.log(JSON.stringify(r));
        expect(r.enfocables).toBeGreaterThanOrEqual(4);
        expect(r.sinFoco).toEqual([]);
        expect(r.inalcanzables).toEqual([]);
        expect(r.sinAnillo).toEqual([]);
      });
    }
  }

  it('Dock: todo control es alcanzable con Tab, recibe foco y tiene anillo', async () => {
    await montar(<VistaDock />);
    const r = revisar('Dock');
    console.log(JSON.stringify(r));
    expect(r.enfocables).toBeGreaterThanOrEqual(10);
    expect(r).toMatchObject({ sinFoco: [], inalcanzables: [], sinAnillo: [] });
  });

  it('Customize: cada pestaña se alcanza y opera con teclado, con anillo de foco', async () => {
    await montar(<VistaCustomize />);
    const pestanas = [...contenedor.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    expect(pestanas).toHaveLength(9);
    for (const p of pestanas) {
      act(() => p.focus());
      expect(document.activeElement).toBe(p);
      await act(async () => p.click());
      const r = revisar(`Customize/${p.textContent}`);
      console.log(JSON.stringify(r));
      expect(r.enfocables).toBeGreaterThan(pestanas.length);
      expect(r).toMatchObject({ sinFoco: [], inalcanzables: [], sinAnillo: [] });
    }
  });

  it('Limpiar temporales: todo control es alcanzable con Tab, recibe foco y tiene anillo', async () => {
    await montar(<VistaLimpieza />);
    const r = revisar('Limpieza');
    console.log(JSON.stringify(r));
    expect(r).toMatchObject({ sinFoco: [], inalcanzables: [], sinAnillo: [] });
  });
});

describe('vista detallada solo en el widget maximizado (GD-WIN-07)', () => {
  it('maximizar CPU no cambia la vista de Memoria; maximizar Memoria sí', async () => {
    await montar(<VistaRam />, 'cpu');
    expect(contenedor.textContent).toContain('Memoria RAM en uso (GB)');
    await emitir('gd://maximizado', { id: 'ram', activo: true });
    expect(contenedor.textContent).toContain('Memoria RAM en la última hora');
    await emitir('gd://maximizado', { id: 'cpu', activo: false });
    expect(contenedor.textContent).toContain('Memoria RAM en la última hora');
    await emitir('gd://maximizado', { id: 'ram', activo: false });
    expect(contenedor.textContent).toContain('Memoria RAM en uso (GB)');
  });
});

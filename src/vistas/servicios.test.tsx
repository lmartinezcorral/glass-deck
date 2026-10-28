import { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Config, CuentaGoogle, EstadoCredencial, EstadoCuentas, EstadoCuentaWeb, FilaClaude, ResumenClaudeCode, ServicioWeb, Ubicacion } from '../api';
import { etiquetaIntervalo, pestanaValida } from './Customize';
import { BuscadorCiudad, describirAviso, estimarCosto, normalizarUmbrales, precioPara, SeccionCuentas, SeccionGasto } from './CustomizeServicios';
import { VistaDock } from './Dock';

const invocar = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invocar, isTauri: () => true }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => undefined) }));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const PRECIOS = [
  { patron: 'opus', entrada: 5, salida: 25, cache_escritura: 6.25, cache_lectura: 0.5 },
  { patron: 'sonnet', entrada: 3, salida: 15, cache_escritura: 3.75, cache_lectura: 0.3 },
];

const FILAS: FilaClaude[] = [
  { dia: '2026-10-28', modelo: 'claude-opus-5', proyecto: 'proyecto-a', tokens: { entrada: 1_000_000, salida: 0, cache_escritura: 0, cache_lectura: 0 }, costo_usd: 5 },
  { dia: '2026-10-27', modelo: 'claude-sonnet-5', proyecto: 'proyecto-b', tokens: { entrada: 2_000_000, salida: 0, cache_escritura: 0, cache_lectura: 0 }, costo_usd: 6 },
];

const RESUMEN: ResumenClaudeCode = {
  hoy: '2026-10-28',
  total_hoy: { tokens: FILAS[0]!.tokens, costo_usd: 5 },
  total_mes: { tokens: { entrada: 3_000_000, salida: 0, cache_escritura: 0, cache_lectura: 0 }, costo_usd: 11 },
  dias: [],
  filas: FILAS,
  sin_precio: [],
};

const CONFIG = {
  version: 3,
  widgets: Object.fromEntries(['cpu', 'ram', 'disco', 'red', 'procesos', 'spending', 'clima'].map((id) => [id, { activo: id === 'cpu' }])),
  dock: { visible: true, x: null, y: null, monitor: null },
  apariencia: { tema: 'oscuro', acento: '#22d3ee', opacidad: 0.44, desenfoque: 24 },
  intervalos: { cpu_ram_red_ms: 2000, procesos_ms: 3000, disco_ms: 5000, spending_ms: 900000, clima_ms: 1800000 },
  lista_negra_usuario: [],
  atajos: { mostrar_ocultar: 'Ctrl+Alt+G', privacidad: 'Ctrl+Alt+P' },
  arranque_windows: false,
  capa_defecto: 'escritorio',
  pausar_con_bateria: true,
  iman: true,
  privacidad: false,
  precios_modelos: PRECIOS,
  clima: { modo: 'automatica', ubicacion: null },
  umbrales: { notificaciones: true, cursor: [50, 80, 100], claude_5h: [80], claude_semanal: [80], chatgpt_5h: [80], chatgpt_semanal: [80], presupuesto_cursor_usd: null },
} as unknown as Config;

let credenciales: EstadoCredencial[] = [];
let cuentasWeb: EstadoCuentas | null = null;
let cuentasGoogle: CuentaGoogle[] | null = null;

const cuentaWeb = (servicio: ServicioWeb, estado: EstadoCuentaWeb['estado'], extra: Partial<EstadoCuentaWeb> = {}): EstadoCuentaWeb => ({
  servicio,
  estado,
  motivo: null,
  conectando: false,
  ...extra,
});
const CIUDADES: Ubicacion[] = [{ nombre: 'Ciudad Uno', region: 'Región', pais: 'País', latitud: 1, longitud: 2 }];

function responder(comando: string): unknown {
  switch (comando) {
    case 'servicio_ultimo':
      return { datos: RESUMEN, hora_ms: Date.now(), error: null, texto_error: null, proximo_intento_ms: null };
    case 'avisos_historial':
      return [{ clave: 'cursor:2026-10:80', hora_ms: Date.now() }];
    case 'boveda_estado':
      return credenciales;
    case 'cuentas_estado':
      return cuentasWeb;
    case 'google_cuentas':
      return cuentasGoogle;
    case 'clima_buscar_ciudad':
      return CIUDADES;
    case 'config_leer':
      return CONFIG;
    case 'catalogo_widgets':
      return [
        ['cpu', 'CPU'],
        ['ram', 'Memoria'],
        ['disco', 'Disco'],
        ['red', 'Red'],
        ['procesos', 'Procesos'],
        ['spending', 'Spending'],
        ['clima', 'Clima'],
        ['correo', 'Correo'],
      ].map(([id, titulo]) => ({ id, titulo, subtitulo: '', activo_defecto: true }));
    default:
      return null;
  }
}

let contenedor: HTMLDivElement;
let raiz: Root;

beforeEach(() => {
  credenciales = [];
  cuentasWeb = null;
  cuentasGoogle = null;
  invocar.mockReset();
  invocar.mockImplementation(async (comando: string) => responder(comando));
  contenedor = document.createElement('div');
  document.body.appendChild(contenedor);
  raiz = createRoot(contenedor);
});

afterEach(() => {
  act(() => raiz.unmount());
  contenedor.remove();
  vi.useRealTimers();
});

const esperar = async (): Promise<void> => {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
};

function escribir(input: HTMLInputElement, valor: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  act(() => {
    setter?.call(input, valor);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

const llamadas = (comando: string) => invocar.mock.calls.filter(([c]) => c === comando);
const texto = (): string => contenedor.textContent ?? '';
const boton = (nombre: string): HTMLButtonElement =>
  [...contenedor.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') ?? b.textContent)?.trim() === nombre)!;

describe('precios por modelo (GD-CLA-03)', () => {
  it('gana el patrón más largo y los modelos sin precio cuentan $0', () => {
    const precios = [...PRECIOS, { patron: 'opus-5', entrada: 15, salida: 75, cache_escritura: 0, cache_lectura: 0 }];
    expect(precioPara('claude-opus-5-20261001', precios)?.patron).toBe('opus-5');
    expect(precioPara('claude-opus-4', precios)?.patron).toBe('opus');
    expect(precioPara('modelo-x', precios)).toBeNull();
    expect(estimarCosto(FILAS, PRECIOS)).toBeCloseTo(11);
    expect(estimarCosto(FILAS, PRECIOS, '2026-10-28')).toBeCloseTo(5);
    expect(estimarCosto(FILAS, [])).toBe(0);
  });

  it('al editar un precio la estimación se recalcula al instante', async () => {
    const guardados: Config[] = [];
    function Envoltura() {
      const [config, setConfig] = useState(CONFIG);
      return (
        <SeccionGasto
          config={config}
          guardar={(c) => {
            guardados.push(c);
            setConfig(c);
          }}
        />
      );
    }
    act(() => raiz.render(<Envoltura />));
    await esperar();
    const estimacion = (): string => contenedor.querySelector('.gd-customize__estimacion')!.textContent ?? '';
    expect(estimacion()).toContain('hoy ≈ $5.00');
    expect(estimacion()).toContain('últimos 30 días ≈ $11.00');

    escribir(contenedor.querySelector<HTMLInputElement>('input[aria-label="Entrada de opus, USD por millón"]')!, '10');
    expect(estimacion()).toContain('hoy ≈ $10.00');
    expect(estimacion()).toContain('últimos 30 días ≈ $16.00');
    expect(guardados.at(-1)?.precios_modelos[0]?.entrada).toBe(10);

    escribir(contenedor.querySelector<HTMLInputElement>('input[aria-label="Entrada de opus, USD por millón"]')!, '-1');
    expect(guardados.at(-1)?.precios_modelos[0]?.entrada).toBe(10);
  });

  it('muestra el historial de avisos y el botón para probar', async () => {
    act(() => raiz.render(<SeccionGasto config={CONFIG} guardar={() => undefined} />));
    await esperar();
    expect(texto()).toContain('Cursor · 80 % · ciclo 2026-10');
    act(() => boton('Probar aviso').click());
    expect(llamadas('avisos_probar')).toHaveLength(1);
  });

  it('umbrales y avisos se normalizan y se describen', () => {
    expect(normalizarUmbrales([80, 50.4, 120, 0, 50, 100, 90, 70, 60])).toEqual([50, 60, 70, 80, 90]);
    expect(describirAviso('claude_5h:2026-10-28T10:00:80')).toBe('Claude · 5 h · 80 % · ciclo 2026-10-28T10:00');
    expect(describirAviso('raro')).toBe('raro');
    expect(describirAviso('chatgpt_semanal:2026-11-02T00:00:80')).toBe('ChatGPT · semanal · 80 % · ciclo 2026-11-02T00:00');
  });

  it('ChatGPT tiene sus propios umbrales de 5 h y semanal (GD-ALERT-01)', async () => {
    const guardados: Config[] = [];
    act(() => raiz.render(<SeccionGasto config={CONFIG} guardar={(c) => guardados.push(c)} />));
    await esperar();
    const campo = contenedor.querySelector<HTMLInputElement>('input[aria-label="Nuevo umbral para ChatGPT · límite semanal"]')!;
    expect(contenedor.querySelector('input[aria-label="Nuevo umbral para ChatGPT · límite de 5 h"]')).not.toBeNull();
    escribir(campo, '50');
    await act(async () => {
      campo.form!.requestSubmit();
      await Promise.resolve();
    });
    expect(guardados.at(-1)?.umbrales.chatgpt_semanal).toEqual([50, 80]);
    expect(guardados.at(-1)?.umbrales.chatgpt_5h).toEqual([80]);
  });
});

describe('buscador de ciudad (GD-WX-02)', () => {
  it('espera 400 ms sin teclear antes de buscar y busca una sola vez', async () => {
    vi.useFakeTimers();
    const elegir = vi.fn();
    act(() => raiz.render(<BuscadorCiudad alElegir={elegir} />));
    const campo = contenedor.querySelector<HTMLInputElement>('input[aria-label="Buscar ciudad"]')!;
    escribir(campo, 'C');
    act(() => vi.advanceTimersByTime(1000));
    expect(llamadas('clima_buscar_ciudad')).toHaveLength(0);
    escribir(campo, 'Ci');
    act(() => vi.advanceTimersByTime(200));
    escribir(campo, 'Ciu');
    act(() => vi.advanceTimersByTime(399));
    expect(llamadas('clima_buscar_ciudad')).toHaveLength(0);
    act(() => vi.advanceTimersByTime(1));
    expect(llamadas('clima_buscar_ciudad')).toEqual([['clima_buscar_ciudad', { texto: 'Ciu' }]]);
    await esperar();
    act(() => contenedor.querySelector<HTMLButtonElement>('.gd-buscador__opcion')!.click());
    expect(elegir).toHaveBeenCalledWith(CIUDADES[0]);
  });
});

describe('cuentas (GD-DOCK-02, GD-CLA-05)', () => {
  it('sin cliente importado, «Conectar Google» abre la guía de 7 pasos', async () => {
    act(() => raiz.render(<SeccionCuentas />));
    await esperar();
    act(() => boton('Conectar Google').click());
    const pasos = document.querySelectorAll('.gd-guia li');
    expect(pasos).toHaveLength(7);
    expect(llamadas('google_cliente_importar')).toHaveLength(0);
  });

  it('la clave de la consola se guarda sin mostrarse y luego solo se ofrece borrarla', async () => {
    act(() => raiz.render(<SeccionCuentas />));
    await esperar();
    const campo = contenedor.querySelector<HTMLInputElement>('input[aria-label^="Clave de administrador"]')!;
    expect(campo.type).toBe('password');
    escribir(campo, 'valor-sintetico');
    credenciales = [{ clave: 'anthropic_admin', presente: true }];
    await act(async () => {
      campo.form!.requestSubmit();
      await Promise.resolve();
    });
    await esperar();
    expect(llamadas('boveda_guardar')).toEqual([['boveda_guardar', { clave: 'anthropic_admin', valor: 'valor-sintetico' }]]);
    expect(texto()).toContain('Clave guardada');
    expect(texto()).not.toContain('valor-sintetico');
    expect(contenedor.querySelector('input[aria-label="Clave de administrador de la consola de Anthropic"]')).toBeNull();
    expect(boton('Borrar clave de la consola de Anthropic')).toBeTruthy();
  });

  it('la clave opcional de OpenAI se guarda aparte, sin mostrarse (GD-OAI-05)', async () => {
    act(() => raiz.render(<SeccionCuentas />));
    await esperar();
    const campo = contenedor.querySelector<HTMLInputElement>('input[aria-label="Clave de administrador de la API de OpenAI"]')!;
    expect(campo.type).toBe('password');
    escribir(campo, 'clave-de-prueba');
    credenciales = [{ clave: 'openai_admin', presente: true }];
    await act(async () => {
      campo.form!.requestSubmit();
      await Promise.resolve();
    });
    await esperar();
    expect(llamadas('boveda_guardar')).toEqual([['boveda_guardar', { clave: 'openai_admin', valor: 'clave-de-prueba' }]]);
    expect(texto()).not.toContain('clave-de-prueba');
    expect(contenedor.querySelector('input[aria-label="Clave de administrador de la API de OpenAI"]')).toBeNull();
    expect(contenedor.querySelector('input[aria-label="Clave de administrador de la consola de Anthropic"]')).not.toBeNull();
    await act(async () => boton('Borrar clave de la API de OpenAI').click());
    expect(llamadas('boveda_borrar')).toEqual([['boveda_borrar', { clave: 'openai_admin' }]]);
  });

  it('«Conectar» abre el login de cada servicio y nunca pide la cookie a mano (GD-CUR-01, GD-CLA-04, GD-OAI-04)', async () => {
    act(() => raiz.render(<SeccionCuentas />));
    await esperar();
    for (const nombre of ['Cursor', 'Claude.ai', 'ChatGPT']) await act(async () => boton(`Conectar ${nombre}`).click());
    expect(llamadas('cuenta_conectar')).toEqual([
      ['cuenta_conectar', { servicio: 'cursor' }],
      ['cuenta_conectar', { servicio: 'claude_ai' }],
      ['cuenta_conectar', { servicio: 'chatgpt' }],
    ]);
    expect(boton('Desconectar Cursor')).toBeUndefined();
    const etiquetas = [...contenedor.querySelectorAll('input')].map((i) => i.getAttribute('aria-label') ?? '');
    expect(etiquetas.every((e) => e.startsWith('Clave de administrador'))).toBe(true);
    expect(texto().match(/no oficial/g)?.length).toBeGreaterThanOrEqual(3);
  });

  it('«Desconectar» borra la sesión y el perfil del servicio; con sesión vencida ofrece reconectar (GD-NFR-SEC-04)', async () => {
    cuentasWeb = {
      web: [
        cuentaWeb('cursor', 'conectada'),
        cuentaWeb('claude_ai', 'reconectar', { motivo: 'Sesión expirada' }),
        cuentaWeb('chatgpt', 'sin_conectar', { conectando: true }),
      ],
      claves: { anthropic_admin: false, openai_admin: false },
    };
    act(() => raiz.render(<SeccionCuentas />));
    await esperar();
    await esperar();
    expect(boton('Conectar Cursor')).toBeUndefined();
    expect(texto()).toContain('Claude.ai: sesión expirada · vuelve a conectar');
    await act(async () => boton('Desconectar Cursor').click());
    expect(llamadas('cuenta_desconectar')).toEqual([['cuenta_desconectar', { servicio: 'cursor' }]]);
    await act(async () => boton('Reconectar Claude.ai').click());
    expect(llamadas('cuenta_conectar')).toEqual([['cuenta_conectar', { servicio: 'claude_ai' }]]);
    expect(boton('Conectar ChatGPT').disabled).toBe(true);
    expect(boton('Conectar ChatGPT').textContent).toBe('Esperando inicio de sesión…');
  });

  it('con cliente importado ofrece quitarlo o reemplazarlo', async () => {
    credenciales = [{ clave: 'google_cliente', presente: true }];
    act(() => raiz.render(<SeccionCuentas />));
    await esperar();
    expect(texto()).toContain('Cliente importado');
    act(() => boton('Reemplazar cliente…').click());
    expect(llamadas('google_cliente_importar')).toHaveLength(1);
  });
});

describe('dock (GD-DOCK-02)', () => {
  it('incluye Spending y Clima y resume las cuentas con puntos de color', async () => {
    credenciales = [{ clave: 'cursor_sesion', presente: true }];
    act(() => raiz.render(<VistaDock />));
    await esperar();
    await esperar();
    expect(boton('Spending: inactivo')).toBeTruthy();
    expect(boton('Clima: inactivo')).toBeTruthy();
    const cuentas = contenedor.querySelector<HTMLButtonElement>('.gd-dock__cuentas')!;
    expect(cuentas.getAttribute('aria-label')).toContain('1 de 4 conectadas');
    expect(cuentas.querySelectorAll('.gd-punto--conectada')).toHaveLength(1);
    expect(cuentas.querySelectorAll('.gd-punto--sin_conectar')).toHaveLength(3);
    act(() => cuentas.click());
    expect(llamadas('panel_abrir')).toEqual([['panel_abrir', { panel: 'cuentas' }]]);
  });

  it('cuenta cada cuenta de Google junto con Cursor, Claude y ChatGPT', async () => {
    cuentasGoogle = [
      { id: 'cuenta-a', correo: 'ana@ejemplo.test', estado: 'conectada' },
      { id: 'cuenta-b', correo: 'bea@ejemplo.test', estado: 'reconectar' },
    ];
    cuentasWeb = {
      web: [cuentaWeb('cursor', 'conectada'), cuentaWeb('claude_ai', 'sin_conectar'), cuentaWeb('chatgpt', 'conectada')],
      claves: { anthropic_admin: false, openai_admin: false },
    };
    act(() => raiz.render(<VistaDock />));
    await esperar();
    await esperar();
    const cuentas = contenedor.querySelector<HTMLButtonElement>('.gd-dock__cuentas')!;
    expect(cuentas.getAttribute('aria-label')).toContain('3 de 5 conectadas');
    expect([...cuentas.querySelectorAll('.gd-dock__avatar')].map((a) => a.getAttribute('data-cuenta'))).toEqual(['google', 'google', 'cursor', 'claude', 'chatgpt']);
    expect(cuentas.querySelector('.gd-dock__cuentas-texto')?.textContent).toContain('3 de 5');
    expect(boton('Conectar Claude.ai')).toBeTruthy();
  });

  it('con modo privacidad no muestra los correos de las cuentas de Google', async () => {
    cuentasGoogle = [
      { id: 'cuenta-a', correo: 'ana@ejemplo.test', estado: 'conectada' },
      { id: 'cuenta-b', correo: 'bea@ejemplo.test', estado: 'conectada' },
    ];
    invocar.mockImplementation(async (comando: string) => (comando === 'config_leer' ? { ...CONFIG, privacidad: true } : responder(comando)));
    act(() => raiz.render(<VistaDock />));
    await esperar();
    await esperar();
    const cuentas = contenedor.querySelector<HTMLButtonElement>('.gd-dock__cuentas')!;
    const textos = `${cuentas.getAttribute('aria-label')} ${cuentas.title}`;
    expect(textos).not.toContain('@ejemplo.test');
    expect(textos).toContain('Google 1');
    expect(textos).toContain('Google 2');
  });

  it('se pone ámbar con sesión vencida o Cloudflare y ofrece reconectar directo', async () => {
    cuentasWeb = {
      web: [
        cuentaWeb('cursor', 'conectada'),
        cuentaWeb('claude_ai', 'sin_conectar'),
        cuentaWeb('chatgpt', 'reconectar', { motivo: 'Cloudflare bloqueó la lectura' }),
      ],
      claves: { anthropic_admin: false, openai_admin: false },
    };
    act(() => raiz.render(<VistaDock />));
    await esperar();
    await esperar();
    const cuentas = contenedor.querySelector<HTMLButtonElement>('.gd-dock__cuentas')!;
    expect(cuentas.querySelector('[data-cuenta="chatgpt"] .gd-punto--reconectar')).not.toBeNull();
    expect(cuentas.getAttribute('aria-label')).toContain('ChatGPT: requiere reconexión');
    await act(async () => boton('Reconectar ChatGPT').click());
    expect(llamadas('cuenta_conectar')).toEqual([['cuenta_conectar', { servicio: 'chatgpt' }]]);
    expect(boton('Conectar Claude.ai')).toBeUndefined();
  });

  it('mientras hay un login abierto no ofrece otro', async () => {
    cuentasWeb = { web: [cuentaWeb('cursor', 'sin_conectar', { conectando: true })], claves: { anthropic_admin: false, openai_admin: false } };
    act(() => raiz.render(<VistaDock />));
    await esperar();
    await esperar();
    expect(contenedor.querySelector('.gd-dock__conectar')).toBeNull();
  });

  it('todos sus botones habilitados se alcanzan con Tab', async () => {
    act(() => raiz.render(<VistaDock />));
    await esperar();
    await esperar();
    const botones = [...contenedor.querySelectorAll<HTMLButtonElement>('button')].filter((b) => !b.disabled);
    expect(botones.length).toBe(8 + 5 + 2);
    for (const b of botones) {
      expect(b.tabIndex).toBe(0);
      expect(b.getAttribute('aria-label')).toBeTruthy();
    }
  });
});

describe('Customize', () => {
  it('abre en la pestaña pedida y etiqueta los intervalos largos en minutos', () => {
    expect(pestanaValida('cuentas')).toBe('cuentas');
    expect(pestanaValida('otra')).toBeNull();
    expect(etiquetaIntervalo(2000)).toBe('2 s');
    expect(etiquetaIntervalo(900_000)).toBe('15 min');
    expect(etiquetaIntervalo(3_600_000)).toBe('1 h');
  });
});

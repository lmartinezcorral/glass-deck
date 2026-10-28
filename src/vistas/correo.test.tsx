import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AgendaCuenta, BuzonCuenta, Config, CuentaGoogle, EstadoCredencial } from '../api';
import { resumenCuentas } from './Cuentas';
import { CuentasGoogle, resumenConGoogle } from './CuentasGoogle';
import { elementosAlerta, urlCorreo, urlEvento, VistaCorreo } from './Correo';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
globalThis.ResizeObserver ??= class {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
};

const tauri = vi.hoisted(() => ({
  oyentes: new Map<string, Set<(e: { payload: unknown }) => void>>(),
  invocar: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invocar, isTauri: () => true }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (evento: string, manejar: (e: { payload: unknown }) => void) => {
    if (!tauri.oyentes.has(evento)) tauri.oyentes.set(evento, new Set());
    tauri.oyentes.get(evento)!.add(manejar);
    return () => tauri.oyentes.get(evento)?.delete(manejar);
  }),
}));

const AHORA = Date.now();
const MIN = 60_000;
const ENLACE_EVENTO = 'https://www.google.com/calendar/event?eid=ZXZlbnRvLWZpY3RpY2lv';

const buzon = (cuenta: string, correo: string, mensajes: BuzonCuenta['mensajes'], estado: BuzonCuenta['estado'] = 'conectada'): BuzonCuenta => ({
  cuenta,
  correo,
  estado,
  error: estado === 'conectada' ? null : 'Reconecta la cuenta',
  no_leidos: mensajes.length,
  mensajes,
});

const mensaje = (id: string, remitente: string, asunto: string) => ({ id, hilo: `hilo-${id}`, remitente, asunto, fecha_ms: AHORA - 10 * MIN });

let buzones: BuzonCuenta[];
let agendas: AgendaCuenta[];
let alerta: string | null;
let privacidad: boolean;
let credenciales: EstadoCredencial[];
let cuentasGoogle: CuentaGoogle[] | null;

function lectura<T>(datos: T) {
  return { datos, hora_ms: AHORA, error: null, texto_error: null, proximo_intento_ms: null };
}

function config(): Config {
  return {
    version: 3,
    widgets: { correo: { activo: true, estado: 'normal', fijado: false, monitor: null, x: null, y: null, ancho: 300, alto: 250 } },
    intervalos: { correo_ms: 300_000, calendario_ms: 600_000, rotacion_ms: 600_000 },
    atajos: { mostrar_ocultar: 'Ctrl+Alt+M', privacidad: 'Super+Alt+P' },
    privacidad,
  } as unknown as Config;
}

function responder(comando: string, args?: { fuente?: string }): unknown {
  switch (comando) {
    case 'config_leer':
      return config();
    case 'servicio_ultimo':
      if (args?.fuente === 'correo') return lectura({ cuentas: buzones });
      if (args?.fuente === 'calendario') return lectura({ cuentas: agendas });
      if (args?.fuente === 'alerta') return lectura({ clave: alerta, desde_ms: AHORA, proxima_ms: AHORA + 5 * MIN });
      return null;
    case 'boveda_estado':
      return credenciales;
    case 'google_cuentas':
      return cuentasGoogle;
    default:
      return null;
  }
}

let contenedor: HTMLDivElement;
let raiz: Root;

beforeEach(() => {
  buzones = [
    buzon('cuenta-a', 'ana@ejemplo.test', [mensaje('a1', 'Remitente Uno', 'Asunto ficticio uno'), mensaje('a2', 'Remitente Dos', 'Asunto ficticio dos')]),
    buzon('cuenta-b', 'bea@ejemplo.test', [mensaje('b1', 'Remitente Tres', 'Asunto ficticio tres')]),
  ];
  agendas = [
    { cuenta: 'cuenta-a', correo: 'ana@ejemplo.test', estado: 'conectada', error: null, eventos: [] },
    {
      cuenta: 'cuenta-b',
      correo: 'bea@ejemplo.test',
      estado: 'conectada',
      error: null,
      eventos: [
        { id: 'e1', titulo: 'Junta ficticia', ubicacion: 'Sala 1', inicio_ms: AHORA + 20 * MIN, fin_ms: AHORA + 50 * MIN, todo_el_dia: false, enlace: ENLACE_EVENTO },
      ],
    },
  ];
  alerta = 'correo:cuenta-a:a2';
  privacidad = false;
  credenciales = [];
  cuentasGoogle = null;
  tauri.oyentes.clear();
  tauri.invocar.mockReset();
  tauri.invocar.mockImplementation(async (comando: string, args?: { fuente?: string }) => responder(comando, args));
  contenedor = document.createElement('div');
  document.body.appendChild(contenedor);
  raiz = createRoot(contenedor);
});

afterEach(() => {
  act(() => raiz.unmount());
  contenedor.remove();
});

const esperar = async (): Promise<void> => {
  for (let i = 0; i < 3; i++) {
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
  }
};

function emitir(evento: string, carga: unknown): void {
  act(() => tauri.oyentes.get(evento)?.forEach((f) => f({ payload: carga })));
}

const llamadas = (comando: string) => tauri.invocar.mock.calls.filter(([c]) => c === comando);
const texto = (): string => contenedor.textContent ?? '';
const boton = (nombre: string): HTMLButtonElement =>
  [...contenedor.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') ?? b.textContent)?.trim() === nombre)!;
const tarjeta = (): HTMLButtonElement => contenedor.querySelector<HTMLButtonElement>('.gd-correo__tarjeta')!;

async function montarWidget(): Promise<void> {
  act(() => raiz.render(<VistaCorreo />));
  await esperar();
}

describe('widget Correo y agenda (GD-ROT-01..03, GD-MAIL-04)', () => {
  it('con 2 cuentas muestra la alerta que eligió el núcleo y, maximizado, una lista por cuenta', async () => {
    await montarWidget();
    expect(tarjeta().textContent).toContain('Remitente Dos');
    expect(tarjeta().textContent).toContain('Asunto ficticio dos');
    expect(texto()).toContain('3 no leídos · 1 evento');
    expect(contenedor.querySelectorAll('.gd-correo__puntos i')).toHaveLength(4);
    expect(contenedor.querySelector('.gd-correo__punto--activo')).toBe(contenedor.querySelectorAll('.gd-correo__puntos i')[2]);

    emitir('gd://maximizado', { id: 'correo', activo: true });
    const bloques = [...contenedor.querySelectorAll('.gd-correo__bloque')];
    expect(bloques.map((b) => b.getAttribute('aria-label'))).toEqual(['Cuenta ana@ejemplo.test', 'Cuenta bea@ejemplo.test']);
    expect(bloques[0]!.querySelectorAll('.gd-correo__tarjeta')).toHaveLength(2);
    expect(bloques[1]!.querySelectorAll('.gd-correo__tarjeta')).toHaveLength(1);
    expect(texto()).toContain('Agenda · hoy y próximas 24 h');
    expect(texto()).toContain('Junta ficticia');
    expect(texto()).toContain('Nunca descarga ni muestra el cuerpo');
  });

  it('una cuenta en ámbar no oculta los correos de la otra (GD-MAIL-06)', async () => {
    buzones = [buzones[0]!, buzon('cuenta-b', 'bea@ejemplo.test', [], 'reconectar')];
    await montarWidget();
    expect(contenedor.querySelector('.gd-correo__reconectar')?.textContent).toContain('Una cuenta perdió el acceso');
    expect(tarjeta().textContent).toContain('Remitente Dos');
    emitir('gd://maximizado', { id: 'correo', activo: true });
    const [a, b] = [...contenedor.querySelectorAll('.gd-correo__bloque')];
    expect(a!.querySelector('.gd-chip')?.textContent).toBe('2 no leídos');
    expect(b!.querySelector('.gd-chip--aviso')?.textContent).toBe('Reconectar');
  });

  it('con privacidad, remitente y asunto no llegan al DOM y quedan en .gd-privado', async () => {
    privacidad = true;
    await montarWidget();
    expect(texto()).not.toContain('Remitente Dos');
    expect(texto()).not.toContain('Asunto ficticio dos');
    expect(texto()).not.toContain('ana');
    expect(tarjeta().querySelectorAll('.gd-privado').length).toBeGreaterThanOrEqual(2);
    expect(tarjeta().getAttribute('aria-label')).toBe('Correo no leído oculto por privacidad. Abrir en Gmail');
    expect(boton('Privacidad: ocultar remitente y asunto').getAttribute('aria-pressed')).toBe('true');

    emitir('gd://privacidad', false);
    expect(tarjeta().textContent).toContain('Asunto ficticio dos');

    act(() => boton('Privacidad: ocultar remitente y asunto').click());
    expect(llamadas('privacidad_alternar')).toHaveLength(1);
  });

  it('el clic abre el hilo en la cuenta correcta y el evento con su authuser', async () => {
    await montarWidget();
    act(() => tarjeta().click());
    expect(llamadas('google_abrir')).toEqual([['google_abrir', { url: 'https://mail.google.com/mail/u/ana@ejemplo.test/#inbox/hilo-a2' }]]);

    act(() => boton('Alerta anterior').click());
    act(() => boton('Alerta anterior').click());
    expect(tarjeta().textContent).toContain('Junta ficticia');
    expect(tarjeta().textContent).toContain('En 20 min');
    act(() => tarjeta().click());
    expect(llamadas('google_abrir')[1]).toEqual(['google_abrir', { url: `${ENLACE_EVENTO}&authuser=bea%40ejemplo.test` }]);
  });

  it('sin cuentas conectadas ofrece «Conectar Google» y lleva a Cuentas', async () => {
    buzones = [];
    agendas = [];
    alerta = null;
    await montarWidget();
    act(() => boton('Conectar Google').click());
    expect(llamadas('panel_abrir')).toEqual([['panel_abrir', { panel: 'cuentas' }]]);
  });

  it('los eventos van primero por hora de inicio y los terminados no se muestran', () => {
    const terminado = { id: 'e0', titulo: 'Ya pasó', ubicacion: null, inicio_ms: AHORA - 60 * MIN, fin_ms: AHORA - MIN, todo_el_dia: false, enlace: '' };
    const temprano = { ...terminado, id: 'e2', titulo: 'Antes', inicio_ms: AHORA + 5 * MIN, fin_ms: AHORA + 15 * MIN };
    agendas[0]!.eventos = [terminado, temprano];
    const claves = elementosAlerta(buzones, agendas, AHORA).map((e) => e.clave);
    expect(claves).toEqual(['evento:cuenta-a:e2', 'evento:cuenta-b:e1', 'correo:cuenta-a:a1', 'correo:cuenta-a:a2', 'correo:cuenta-b:b1']);
  });

  it('las URL escapan el hilo y rechazan enlaces inválidos', () => {
    expect(urlCorreo('ana+trabajo@ejemplo.test', 'a/b')).toBe('https://mail.google.com/mail/u/ana%2Btrabajo@ejemplo.test/#inbox/a%2Fb');
    expect(urlEvento('no es una url', 'ana@ejemplo.test')).toBeNull();
  });
});

describe('Customize › Cuentas › Google (GD-MAIL-01, GD-MAIL-07)', () => {
  it('sin cliente importado, «Conectar Google» abre la guía y no intenta conectar', async () => {
    act(() => raiz.render(<CuentasGoogle credenciales={[]} recargar={() => undefined} />));
    await esperar();
    act(() => boton('Conectar Google').click());
    expect(document.querySelectorAll('.gd-guia li')).toHaveLength(7);
    expect(llamadas('google_conectar')).toHaveLength(0);
  });

  it('con cliente lista cada cuenta, conecta otra y desconecta por id', async () => {
    cuentasGoogle = [
      { id: 'cuenta-a', correo: 'ana@ejemplo.test', estado: 'conectada' },
      { id: 'cuenta-b', correo: 'bea@ejemplo.test', estado: 'reconectar' },
    ];
    act(() => raiz.render(<CuentasGoogle credenciales={[{ clave: 'google_cliente', presente: true }]} recargar={() => undefined} />));
    await esperar();
    expect(texto()).toContain('ana@ejemplo.test');
    expect(contenedor.querySelector('h3')?.textContent).toContain('Requiere reconexión');
    act(() => boton('Conectar otra cuenta').click());
    expect(llamadas('google_conectar')).toHaveLength(1);
    expect(texto()).toContain('Autoriza en la ventana del navegador');
    await esperar();
    act(() => boton('Desconectar bea@ejemplo.test').click());
    expect(llamadas('google_desconectar')).toEqual([['google_desconectar', { id: 'cuenta-b' }]]);
    await esperar();
    expect(texto()).toContain('Cuenta desconectada y acceso revocado');
  });

  it('el dock muestra una entrada por cuenta, en verde o ámbar (GD-DOCK-02)', () => {
    const base = resumenCuentas([]);
    expect(resumenConGoogle(base, null)).toEqual(base);
    expect(resumenConGoogle(base, [])).toEqual(base);
    const con = resumenConGoogle(base, [
      { id: 'cuenta-a', correo: 'ana@ejemplo.test', estado: 'conectada' },
      { id: 'cuenta-b', correo: 'bea@ejemplo.test', estado: 'reconectar' },
    ]);
    const google = con.filter((c) => c.id === 'google');
    expect(google.map((c) => [c.clave, c.estado])).toEqual([
      ['google:cuenta-a', 'conectada'],
      ['google:cuenta-b', 'reconectar'],
    ]);
    expect(con).toHaveLength(base.length + 1);
  });
});

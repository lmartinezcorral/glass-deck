import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { api, enTauri, escuchar, EVENTOS, type ClaveCredencial, type EstadoCredencial, type EstadoCuentas, type ServicioWeb } from '../api';
import { IconoChatgpt, IconoClaude, IconoCursor } from '../iconos';

export type EstadoCuenta = 'conectada' | 'reconectar' | 'sin_conectar';

export interface ResumenCuenta {
  id: 'google' | 'cursor' | 'claude' | 'chatgpt';
  nombre: string;
  estado: EstadoCuenta;
  detalle: string;
  icono: ReactNode;
}

const TEXTO_ESTADO: Record<EstadoCuenta, string> = {
  conectada: 'Conectada',
  reconectar: 'Requiere reconexión',
  sin_conectar: 'Sin conectar',
};

export const textoEstadoCuenta = (e: EstadoCuenta): string => TEXTO_ESTADO[e];

export const NOMBRE_SERVICIO: Record<ServicioWeb, string> = { cursor: 'Cursor', claude_ai: 'Claude.ai', chatgpt: 'ChatGPT' };

const CLAVE_SERVICIO: Record<ServicioWeb, ClaveCredencial> = { cursor: 'cursor_sesion', claude_ai: 'claude_ai_sesion', chatgpt: 'chatgpt_sesion' };

export function credencialPresente(credenciales: readonly EstadoCredencial[] | null, clave: ClaveCredencial): boolean {
  return credenciales?.find((c) => c.clave === clave)?.presente ?? false;
}

/** El núcleo dice si la sesión sigue vigente; mientras no responda, basta con saber si hay sesión guardada. */
export function estadoServicio(servicio: ServicioWeb, credenciales: readonly EstadoCredencial[] | null, web: EstadoCuentas | null): EstadoCuenta {
  const real = web?.web.find((c) => c.servicio === servicio);
  if (real) return real.estado;
  return credencialPresente(credenciales, CLAVE_SERVICIO[servicio]) ? 'conectada' : 'sin_conectar';
}

function detalleWeb(estado: EstadoCuenta, web: EstadoCuentas | null, servicio: ServicioWeb): string {
  if (estado === 'reconectar') return `${web?.web.find((c) => c.servicio === servicio)?.motivo ?? 'Sesión expirada'} · vuelve a conectar`;
  return estado === 'conectada' ? 'Sesión guardada · lectura no oficial' : 'Sin sesión · lectura no oficial';
}

/** GD-DOCK-02: ámbar si la última lectura dio sesión vencida o Cloudflare. Google sigue gris hasta el bloque 14. */
export function resumenCuentas(credenciales: readonly EstadoCredencial[] | null, web: EstadoCuentas | null = null): ResumenCuenta[] {
  const cliente = credencialPresente(credenciales, 'google_cliente');
  const cursor = estadoServicio('cursor', credenciales, web);
  const claudeAi = estadoServicio('claude_ai', credenciales, web);
  const chatgpt = estadoServicio('chatgpt', credenciales, web);
  return [
    {
      id: 'google',
      nombre: 'Google',
      estado: 'sin_conectar',
      detalle: cliente ? 'Cliente importado · falta iniciar sesión' : 'Falta importar el cliente OAuth',
      icono: <span className="gd-cuenta__letra">G</span>,
    },
    {
      id: 'cursor',
      nombre: 'Cursor',
      estado: cursor,
      detalle: detalleWeb(cursor, web, 'cursor'),
      icono: <IconoCursor />,
    },
    {
      id: 'claude',
      nombre: 'Claude',
      estado: claudeAi,
      detalle:
        claudeAi === 'sin_conectar' ? 'Claude Code local · Claude.ai sin sesión' : `Claude.ai: ${detalleWeb(claudeAi, web, 'claude_ai').toLowerCase()}`,
      icono: <IconoClaude />,
    },
    {
      id: 'chatgpt',
      nombre: 'ChatGPT',
      estado: chatgpt,
      detalle: detalleWeb(chatgpt, web, 'chatgpt'),
      icono: <IconoChatgpt />,
    },
  ];
}

/** El que el dock ofrece conectar directo: primero uno que pida reconexión, luego uno sin conectar; ninguno si hay un login abierto. */
export function cuentaPorConectar(credenciales: readonly EstadoCredencial[] | null, web: EstadoCuentas | null): { servicio: ServicioWeb; estado: EstadoCuenta } | null {
  if (web?.web.some((c) => c.conectando)) return null;
  const servicios: ServicioWeb[] = ['cursor', 'claude_ai', 'chatgpt'];
  const estados = servicios.map((servicio) => ({ servicio, estado: estadoServicio(servicio, credenciales, web) }));
  return estados.find((e) => e.estado === 'reconectar') ?? estados.find((e) => e.estado === 'sin_conectar') ?? null;
}

export function PuntoCuenta({ estado }: { estado: EstadoCuenta }) {
  return <i className={`gd-punto gd-punto--${estado}`} aria-hidden="true" />;
}

const REVISION_MS = 5 * 60_000;

function useRecarga<T>(leer: () => Promise<T>, eventos: readonly string[]): [T | null, () => void] {
  const [valor, setValor] = useState<T | null>(null);
  const recargar = useCallback(() => {
    if (!enTauri()) return;
    leer()
      .then((v) => setValor(v ?? null))
      .catch(() => undefined);
  }, [leer]);
  const claveEventos = eventos.join('|');
  useEffect(() => {
    recargar();
    const quitar = claveEventos.split('|').map((e) => escuchar<null>(e, recargar));
    const id = window.setInterval(recargar, REVISION_MS);
    window.addEventListener('focus', recargar);
    return () => {
      quitar.forEach((q) => q.then((f) => f()));
      window.clearInterval(id);
      window.removeEventListener('focus', recargar);
    };
  }, [recargar, claveEventos]);
  return [valor, recargar];
}

/** Estado de la bóveda: al montar, con `gd://config` o `gd://cuentas`, al enfocar la ventana y cada 5 min. */
export function useCredenciales(): [EstadoCredencial[] | null, () => void] {
  return useRecarga(api.bovedaEstado, [EVENTOS.config, EVENTOS.cuentas]);
}

/** Estado de las cuentas web y de las claves opcionales; se recarga con `gd://cuentas` (login, lectura con otro resultado). */
export function useCuentasWeb(): [EstadoCuentas | null, () => void] {
  return useRecarga(api.cuentasEstado, [EVENTOS.cuentas]);
}

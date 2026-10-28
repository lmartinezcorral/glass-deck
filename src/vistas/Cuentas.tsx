import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { api, enTauri, escuchar, EVENTOS, type ClaveCredencial, type EstadoCredencial } from '../api';
import { IconoClaude, IconoCursor } from '../iconos';

export type EstadoCuenta = 'conectada' | 'reconectar' | 'sin_conectar';

export interface ResumenCuenta {
  id: 'google' | 'cursor' | 'claude';
  /** Distingue varias cuentas del mismo servicio (p. ej. dos de Google). */
  clave?: string;
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

export function credencialPresente(credenciales: readonly EstadoCredencial[] | null, clave: ClaveCredencial): boolean {
  return credenciales?.find((c) => c.clave === clave)?.presente ?? false;
}

/**
 * GD-DOCK-02. Hasta los bloques 13 y 14 el núcleo solo informa si hay credencial guardada, no si sigue vigente:
 * por eso nadie pasa a ámbar todavía y Google sigue gris aunque el cliente OAuth esté importado.
 */
export function resumenCuentas(credenciales: readonly EstadoCredencial[] | null): ResumenCuenta[] {
  const cliente = credencialPresente(credenciales, 'google_cliente');
  const cursor = credencialPresente(credenciales, 'cursor_sesion');
  const claudeAi = credencialPresente(credenciales, 'claude_ai_sesion');
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
      estado: cursor ? 'conectada' : 'sin_conectar',
      detalle: cursor ? 'Sesión guardada · lectura no oficial' : 'Sin sesión · lectura no oficial',
      icono: <IconoCursor />,
    },
    {
      id: 'claude',
      nombre: 'Claude',
      estado: claudeAi ? 'conectada' : 'sin_conectar',
      detalle: claudeAi ? 'Claude.ai con sesión guardada' : 'Claude Code local · Claude.ai sin sesión',
      icono: <IconoClaude />,
    },
  ];
}

export function PuntoCuenta({ estado }: { estado: EstadoCuenta }) {
  return <i className={`gd-punto gd-punto--${estado}`} aria-hidden="true" />;
}

const REVISION_MS = 5 * 60_000;

/** Estado de la bóveda: al montar, con `gd://config`, al enfocar la ventana y cada 5 min. */
export function useCredenciales(): [EstadoCredencial[] | null, () => void] {
  const [credenciales, setCredenciales] = useState<EstadoCredencial[] | null>(null);
  const recargar = useCallback(() => {
    if (!enTauri()) return;
    api
      .bovedaEstado()
      .then(setCredenciales)
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    recargar();
    const quitar = escuchar<null>(EVENTOS.config, recargar);
    const id = window.setInterval(recargar, REVISION_MS);
    window.addEventListener('focus', recargar);
    return () => {
      quitar.then((f) => f());
      window.clearInterval(id);
      window.removeEventListener('focus', recargar);
    };
  }, [recargar]);
  return [credenciales, recargar];
}

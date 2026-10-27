import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type EstadoWidget = 'normal' | 'pildora' | 'oculto';
export type Capa = 'escritorio' | 'siempre_encima';
export type TemaConfig = 'oscuro' | 'claro' | 'sistema';
export type Nivel = 'protegido' | 'reforzado' | 'normal';
export type ResultadoCierre = 'cerrado' | 'rechazado' | 'sin_permiso' | 'ya_no_existia';
export type Rechazo = 'protegido' | 'requiere_confirmacion' | 'ya_no_existe';
export type AccionAtajo = 'mostrar_ocultar' | 'privacidad';

export interface ConfigWidget {
  activo: boolean;
  estado: EstadoWidget;
  fijado: boolean;
  monitor: string | null;
  x: number | null;
  y: number | null;
  ancho: number;
  alto: number;
}

export interface Config {
  version: number;
  widgets: Record<string, ConfigWidget>;
  dock: { visible: boolean; x: number | null; y: number | null; monitor: string | null };
  apariencia: { tema: TemaConfig; acento: string; opacidad: number; desenfoque: number };
  intervalos: { cpu_ram_red_ms: number; procesos_ms: number; disco_ms: number };
  lista_negra_usuario: string[];
  atajos: { mostrar_ocultar: string; privacidad: string };
  arranque_windows: boolean;
  capa_defecto: Capa;
  pausar_con_bateria: boolean;
  iman: boolean;
  privacidad: boolean;
}

export interface MuestraCpu {
  total: number;
  nucleos: number[];
  nombre: string;
}

export interface MuestraRam {
  total: number;
  usada: number;
  en_cache: number;
  libre: number;
  comprometida: number;
  limite_comprometida: number;
}

export interface UnidadDisco {
  unidad: string;
  etiqueta: string;
  total: number;
  libre: number;
  extraible: boolean;
  lectura_bps: number;
  escritura_bps: number;
}

export interface MuestraRed {
  adaptador: string;
  tipo: string;
  ip: string | null;
  bajada_bps: number;
  subida_bps: number;
}

export interface FilaProceso {
  pid: number;
  nombre: string;
  cpu: number;
  ram: number;
  usuario: string | null;
  nivel: Nivel;
  reiniciar_explorer: boolean;
}

export type Lectura =
  | { fuente: 'cpu'; datos: MuestraCpu }
  | { fuente: 'ram'; datos: MuestraRam }
  | { fuente: 'disco'; datos: UnidadDisco[] }
  | { fuente: 'red'; datos: MuestraRed }
  | { fuente: 'procesos'; datos: FilaProceso[] };

export interface SerieRemota {
  nombre: string;
  valores: number[];
}

export interface ResultadoProceso {
  pid: number;
  nombre: string;
  resultado: ResultadoCierre;
  motivo: Rechazo | null;
}

export interface OrigenLimpieza {
  id: 'temp_usuario' | 'temp_windows' | 'papelera';
  nombre: string;
  bytes: number;
  archivos: number;
  requiere_admin: boolean;
  estimado: boolean;
  marcado_defecto: boolean;
}

export interface OpcionesLimpieza {
  temp_usuario: boolean;
  temp_windows: boolean;
  papelera: boolean;
}

export interface ResumenLimpieza {
  liberado: number;
  archivos_borrados: number;
  omitidos: number;
  windows_temp: string | null;
  papelera: string | null;
}

export interface EstadoAtajo {
  accion: AccionAtajo;
  combinacion: string;
  registrado: boolean;
  error: string | null;
}

export const enTauri = (): boolean => isTauri();

export const api = {
  configLeer: () => invoke<Config>('config_leer'),
  configGuardar: (config: Config) => invoke<Config>('config_guardar', { config }),
  widgetActivar: (id: string, activo: boolean) => invoke<void>('widget_activar', { id, activo }),
  widgetEstado: (estado: EstadoWidget) => invoke<void>('widget_estado', { estado }),
  widgetMaximizar: (activo: boolean) => invoke<void>('widget_maximizar', { activo }),
  widgetFijar: (fijado: boolean) => invoke<void>('widget_fijar', { fijado }),
  widgetCerrar: () => invoke<void>('widget_cerrar'),
  historial: (fuente: string) => invoke<SerieRemota[]>('historial', { fuente }),
  procesosCerrar: (pids: number[], confirmacion: string | null) =>
    invoke<ResultadoProceso[]>('procesos_cerrar', { pids, confirmacion }),
  procesoEsFijo: (nombre: string) => invoke<boolean>('proceso_es_fijo', { nombre }),
  limpiezaVistaPrevia: () => invoke<OrigenLimpieza[]>('limpieza_vista_previa'),
  limpiezaEjecutar: (opciones: OpcionesLimpieza) => invoke<ResumenLimpieza>('limpieza_ejecutar', { opciones }),
  panelAbrir: (panel: 'customize' | 'limpieza' | 'procesos' | 'dock') => invoke<void>('panel_abrir', { panel }),
  panelCerrar: () => invoke<void>('panel_cerrar'),
  atajosEstado: () => invoke<EstadoAtajo[]>('atajos_estado'),
  atajoCambiar: (accion: AccionAtajo, combinacion: string) =>
    invoke<EstadoAtajo[]>('atajo_cambiar', { accion, combinacion }),
  privacidadAlternar: () => invoke<boolean>('privacidad_alternar'),
  mostrarOcultarTodo: () => invoke<void>('mostrar_ocultar_todo'),
  restablecerPosiciones: () => invoke<void>('restablecer_posiciones'),
  reiniciarExplorer: () => invoke<void>('reiniciar_explorer'),
  abrirAdministradorTareas: () => invoke<void>('abrir_administrador_tareas'),
  estadoEntorno: () => invoke<{ con_bateria: boolean; pausado: boolean; version: string }>('estado_entorno'),
};

export async function escuchar<T>(evento: string, manejar: (carga: T) => void): Promise<UnlistenFn> {
  if (!enTauri()) return () => undefined;
  return listen<T>(evento, (e) => manejar(e.payload));
}

export const EVENTOS = {
  datos: 'gd://datos',
  config: 'gd://config',
  privacidad: 'gd://privacidad',
  bateria: 'gd://bateria',
  maximizado: 'gd://maximizado',
  estado: 'gd://estado',
} as const;

export function mensajeError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Ocurrió un error inesperado';
}

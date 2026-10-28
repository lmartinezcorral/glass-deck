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

/** USD por millón de tokens; `patron` es un fragmento del nombre del modelo (gana el más largo). */
export interface PrecioModelo {
  patron: string;
  entrada: number;
  salida: number;
  cache_escritura: number;
  cache_lectura: number;
}

export type ModoUbicacion = 'automatica' | 'manual';

export interface Ubicacion {
  nombre: string;
  region: string | null;
  pais: string | null;
  latitud: number;
  longitud: number;
}

/** Porcentajes de 1 a 100, ordenados, máximo 5 por medidor; una lista vacía apaga ese medidor. */
export interface Umbrales {
  notificaciones: boolean;
  cursor: number[];
  claude_5h: number[];
  claude_semanal: number[];
  presupuesto_cursor_usd: number | null;
}

export interface Config {
  version: number;
  widgets: Record<string, ConfigWidget>;
  dock: { visible: boolean; x: number | null; y: number | null; monitor: string | null };
  apariencia: { tema: TemaConfig; acento: string; opacidad: number; desenfoque: number };
  intervalos: { cpu_ram_red_ms: number; procesos_ms: number; disco_ms: number; spending_ms: number; clima_ms: number };
  lista_negra_usuario: string[];
  atajos: { mostrar_ocultar: string; privacidad: string };
  arranque_windows: boolean;
  capa_defecto: Capa;
  pausar_con_bateria: boolean;
  iman: boolean;
  privacidad: boolean;
  precios_modelos: PrecioModelo[];
  clima: { modo: ModoUbicacion; ubicacion: Ubicacion | null };
  umbrales: Umbrales;
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

export type Motivo =
  | 'sin_conexion'
  | 'formato_cambio'
  | 'sesion_expirada'
  | 'servicio_no_disponible'
  | 'dominio_no_permitido'
  | 'sin_ubicacion'
  | 'interno';

/** Último dato válido con su hora; si la última lectura falló, `error` dice por qué y `datos` se conserva. */
export interface EstadoFuente<T> {
  datos: T | null;
  hora_ms: number | null;
  error: Motivo | null;
  texto_error: string | null;
  proximo_intento_ms: number | null;
}

export interface Tokens {
  entrada: number;
  salida: number;
  cache_escritura: number;
  cache_lectura: number;
}

export interface TotalesClaude {
  tokens: Tokens;
  costo_usd: number;
}

export interface DiaClaude extends TotalesClaude {
  dia: string;
}

export interface FilaClaude extends TotalesClaude {
  dia: string;
  modelo: string;
  proyecto: string;
}

/** `dias` trae siempre 30 elementos (del más viejo a hoy); `filas` cubre esos mismos 30 días. */
export interface ResumenClaudeCode {
  hoy: string;
  total_hoy: TotalesClaude;
  total_mes: TotalesClaude;
  dias: DiaClaude[];
  filas: FilaClaude[];
  sin_precio: string[];
}

export type IconoClima =
  | 'despejado'
  | 'parcial'
  | 'nublado'
  | 'niebla'
  | 'llovizna'
  | 'lluvia'
  | 'chubascos'
  | 'nieve'
  | 'tormenta'
  | 'desconocido';

export interface Clima {
  lugar: { nombre: string; region: string | null; pais: string | null; automatica: boolean };
  zona_horaria: string | null;
  actual: {
    hora: string;
    temperatura: number;
    sensacion: number | null;
    humedad: number | null;
    viento_kmh: number | null;
    codigo: number | null;
    icono: IconoClima;
    descripcion: string;
    es_dia: boolean;
    maxima: number | null;
    minima: number | null;
  };
  horas: { hora: string; temperatura: number | null; codigo: number | null; icono: IconoClima; prob_lluvia: number | null; es_dia: boolean }[];
  dias: {
    fecha: string;
    maxima: number | null;
    minima: number | null;
    codigo: number | null;
    icono: IconoClima;
    descripcion: string;
    prob_lluvia: number | null;
  }[];
}

export type LecturaServicio =
  | { fuente: 'claude_code'; datos: EstadoFuente<ResumenClaudeCode> }
  | { fuente: 'clima'; datos: EstadoFuente<Clima> };

export type FuenteServicio = LecturaServicio['fuente'];

export type Lectura =
  | { fuente: 'cpu'; datos: MuestraCpu }
  | { fuente: 'ram'; datos: MuestraRam }
  | { fuente: 'disco'; datos: UnidadDisco[] }
  | { fuente: 'red'; datos: MuestraRed }
  | { fuente: 'procesos'; datos: FilaProceso[] }
  | LecturaServicio;

export type ClaveCredencial = 'cursor_sesion' | 'claude_ai_sesion' | 'anthropic_admin' | 'google_cliente';

/** Nunca trae el valor del secreto: solo si existe. */
export interface EstadoCredencial {
  clave: ClaveCredencial;
  presente: boolean;
}

export interface DefWidget {
  id: string;
  titulo: string;
  subtitulo: string;
  activo_defecto: boolean;
}

export type Medidor = 'cursor' | 'claude_5h' | 'claude_semanal';

/** Carga de `gd://aviso`, emitido cuando se manda una notificación de umbral. */
export interface Aviso {
  medidor: Medidor;
  ciclo: string;
  umbral: number;
  porcentaje: number;
  titulo: string;
  cuerpo: string;
}

/** `clave` es `medidor:ciclo:umbral`. */
export interface AvisoEnviado {
  clave: string;
  hora_ms: number;
}

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
  procesosFijos: () => invoke<string[]>('procesos_fijos'),
  limpiezaVistaPrevia: () => invoke<OrigenLimpieza[]>('limpieza_vista_previa'),
  limpiezaEjecutar: (opciones: OpcionesLimpieza) => invoke<ResumenLimpieza>('limpieza_ejecutar', { opciones }),
  /** `cuentas` y `clima` abren Customize en esa pestaña. */
  panelAbrir: (panel: 'customize' | 'cuentas' | 'clima' | 'limpieza' | 'procesos' | 'dock') => invoke<void>('panel_abrir', { panel }),
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
  catalogoWidgets: () => invoke<DefWidget[]>('catalogo_widgets'),
  servicioUltimo: <F extends FuenteServicio>(fuente: F) =>
    invoke<Extract<LecturaServicio, { fuente: F }>['datos'] | null>('servicio_ultimo', { fuente }),
  bovedaEstado: () => invoke<EstadoCredencial[]>('boveda_estado'),
  /** Solo `anthropic_admin` se captura a mano; las demás llegan con su propio inicio de sesión. */
  bovedaGuardar: (clave: ClaveCredencial, valor: string) => invoke<void>('boveda_guardar', { clave, valor }),
  bovedaBorrar: (clave: ClaveCredencial) => invoke<void>('boveda_borrar', { clave }),
  climaBuscarCiudad: (texto: string) => invoke<Ubicacion[]>('clima_buscar_ciudad', { texto }),
  /** `null` vuelve a la ubicación automática por IP. */
  climaFijarUbicacion: (ubicacion: Ubicacion | null) => invoke<Config>('clima_fijar_ubicacion', { ubicacion }),
  avisosProbar: () => invoke<void>('avisos_probar'),
  avisosHistorial: () => invoke<AvisoEnviado[]>('avisos_historial'),
  /** Abre el diálogo de Windows y guarda el cliente en la bóveda; `false` si se cancela. Nunca devuelve el contenido. */
  googleClienteImportar: () => invoke<boolean>('google_cliente_importar'),
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
  aviso: 'gd://aviso',
  pestana: 'gd://pestana',
} as const;

export function mensajeError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Ocurrió un error inesperado';
}

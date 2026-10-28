const UNIDADES = ['B', 'KB', 'MB', 'GB', 'TB'];

export function bytes(valor: number, decimales = 1): string {
  if (!Number.isFinite(valor) || valor <= 0) return '0 B';
  let v = valor;
  let i = 0;
  while (v >= 1024 && i < UNIDADES.length - 1) {
    v /= 1024;
    i += 1;
  }
  const d = i === 0 ? 0 : v >= 100 ? 0 : decimales;
  return `${v.toFixed(d)} ${UNIDADES[i]}`;
}

export function velocidad(bytesPorSegundo: number): string {
  return `${bytes(bytesPorSegundo)}/s`;
}

export function porcentaje(valor: number): string {
  if (!Number.isFinite(valor)) return '0 %';
  return `${Math.round(Math.min(100, Math.max(0, valor)))} %`;
}

export function gb(valor: number): string {
  return (valor / 1024 ** 3).toFixed(1);
}

export function puntosPorMinuto(intervaloMs: number): number {
  const ms = Number.isFinite(intervaloMs) && intervaloMs > 0 ? intervaloMs : 2000;
  return Math.max(10, Math.round(60_000 / ms));
}

export const PUNTOS_HORA_MAX = 3_600;

export function puntosPorHora(intervaloMs: number): number {
  const ms = Number.isFinite(intervaloMs) && intervaloMs > 0 ? intervaloMs : 2000;
  return Math.min(PUNTOS_HORA_MAX, Math.max(1, Math.round(3_600_000 / ms)));
}

export function usd(valor: number): string {
  if (!Number.isFinite(valor)) return '—';
  const decimales = Math.abs(valor) >= 1000 ? 0 : 2;
  return `$${valor.toLocaleString('en-US', { minimumFractionDigits: decimales, maximumFractionDigits: decimales })}`;
}

export function tokens(valor: number): string {
  if (!Number.isFinite(valor) || valor <= 0) return '0';
  if (valor >= 1e9) return `${(valor / 1e9).toFixed(2)} B`;
  if (valor >= 1e6) return `${(valor / 1e6).toFixed(2)} M`;
  if (valor >= 1e3) return `${(valor / 1e3).toFixed(valor >= 1e5 ? 0 : 1)} k`;
  return String(Math.round(valor));
}

export function hace(horaMs: number, ahoraMs: number): string {
  const min = Math.floor(Math.max(0, ahoraMs - horaMs) / 60_000);
  if (min < 1) return 'hace un momento';
  if (min < 60) return `hace ${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return `hace ${h} h`;
  return `hace ${Math.floor(h / 24)} d`;
}

export function horaCorta(horaMs: number): string {
  return new Date(horaMs).toLocaleTimeString('es-MX', { hour: '2-digit', minute: '2-digit', hour12: false });
}

/** Fecha `AAAA-MM-DD` como día local (no UTC), p. ej. «mié 28». */
export function diaCorto(fecha: string): string {
  const [a, m, d] = fecha.split('-').map(Number);
  if (!a || !m || !d) return fecha;
  const dia = new Date(a, m - 1, d);
  return `${dia.toLocaleDateString('es-MX', { weekday: 'short' }).replace('.', '')} ${d}`;
}

export function grados(valor: number | null): string {
  return valor === null || !Number.isFinite(valor) ? '—' : `${Math.round(valor)}°`;
}

export function agregarAcotado(lista: readonly number[], valor: number, maximo: number): number[] {
  const siguiente = lista.length >= maximo ? lista.slice(lista.length - maximo + 1) : lista.slice();
  siguiente.push(valor);
  return siguiente;
}

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

export function agregarAcotado(lista: readonly number[], valor: number, maximo: number): number[] {
  const siguiente = lista.length >= maximo ? lista.slice(lista.length - maximo + 1) : lista.slice();
  siguiente.push(valor);
  return siguiente;
}

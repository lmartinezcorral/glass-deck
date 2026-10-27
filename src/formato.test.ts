import { describe, expect, it } from 'vitest';
import { agregarAcotado, bytes, porcentaje, puntosPorMinuto, velocidad } from './formato';

describe('formato', () => {
  it('convierte bytes a la unidad más legible', () => {
    expect(bytes(0)).toBe('0 B');
    expect(bytes(512)).toBe('512 B');
    expect(bytes(1536)).toBe('1.5 KB');
    expect(bytes(16 * 1024 ** 3)).toBe('16.0 GB');
    expect(bytes(250 * 1024 ** 3)).toBe('250 GB');
  });

  it('formatea velocidades y porcentajes acotados', () => {
    expect(velocidad(2 * 1024 * 1024)).toBe('2.0 MB/s');
    expect(porcentaje(23.4)).toBe('23 %');
    expect(porcentaje(140)).toBe('100 %');
    expect(porcentaje(Number.NaN)).toBe('0 %');
  });

  it('calcula cuántas muestras caben en un minuto según el intervalo', () => {
    expect(puntosPorMinuto(2000)).toBe(30);
    expect(puntosPorMinuto(1000)).toBe(60);
    expect(puntosPorMinuto(5000)).toBe(12);
    expect(puntosPorMinuto(60_000)).toBe(10);
    expect(puntosPorMinuto(0)).toBe(30);
  });

  it('mantiene un historial acotado', () => {
    let h: number[] = [];
    for (let i = 0; i < 70; i += 1) h = agregarAcotado(h, i, 60);
    expect(h).toHaveLength(60);
    expect(h[0]).toBe(10);
    expect(h[59]).toBe(69);
  });
});

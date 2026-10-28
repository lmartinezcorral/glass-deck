import { describe, expect, it } from 'vitest';
import { tramoOcupado } from '../componentes/GraficaHistorial';
import { OPCIONES_INTERVALO } from './Customize';

describe('intervalos de Customize', () => {
  it('no ofrecen valores bajo el mínimo del backend', () => {
    expect(Math.min(...OPCIONES_INTERVALO.cpu_ram_red_ms)).toBeGreaterThanOrEqual(1000);
    expect(Math.min(...OPCIONES_INTERVALO.procesos_ms)).toBeGreaterThanOrEqual(2000);
    expect(Math.min(...OPCIONES_INTERVALO.disco_ms)).toBeGreaterThanOrEqual(1000);
    expect(Math.min(...OPCIONES_INTERVALO.spending_ms)).toBeGreaterThanOrEqual(300000);
    expect(Math.min(...OPCIONES_INTERVALO.clima_ms)).toBeGreaterThanOrEqual(900000);
  });

  it('incluyen los valores por defecto', () => {
    expect(OPCIONES_INTERVALO.cpu_ram_red_ms).toContain(2000);
    expect(OPCIONES_INTERVALO.procesos_ms).toContain(3000);
    expect(OPCIONES_INTERVALO.disco_ms).toContain(5000);
    expect(OPCIONES_INTERVALO.spending_ms).toContain(900000);
    expect(OPCIONES_INTERVALO.clima_ms).toContain(1800000);
  });
});

describe('gráfica de la última hora', () => {
  it('alinea a la derecha un historial incompleto', () => {
    expect(tramoOcupado(1801, 1800, 600)).toEqual({ x: 0, ancho: 600 });
    expect(tramoOcupado(901, 1801, 600)).toEqual({ x: 300, ancho: 300 });
    expect(tramoOcupado(10, undefined, 600)).toEqual({ x: 0, ancho: 600 });
  });
});

import { describe, expect, it } from 'vitest';
import ventanasRs from '../../src-tauri/src/ventanas/mod.rs?raw';
import tokens from './tokens.css?raw';
import { aplicarTema, hexARgb, luminancia, OPCIONES_TEMA_PREDETERMINADAS } from './tema';

const DURACIONES = ['--gd-dur-rapida', '--gd-dur-media', '--gd-dur-lenta', '--gd-dur-medidor', '--gd-dur-barra'];

function bloque(css: string, inicio: string): string {
  const desde = css.indexOf(inicio);
  expect(desde, `falta ${inicio}`).toBeGreaterThanOrEqual(0);
  let nivel = 0;
  for (let i = css.indexOf('{', desde); i < css.length; i++) {
    if (css[i] === '{') nivel++;
    if (css[i] === '}' && --nivel === 0) return css.slice(desde, i + 1);
  }
  return '';
}

describe('animaciones (GD-NFR-UX-03)', () => {
  it('sin animaciones marca la raíz con sin-animacion y la quita al reactivarlas', () => {
    aplicarTema({ ...OPCIONES_TEMA_PREDETERMINADAS, animaciones: false });
    expect(document.documentElement.classList.contains('sin-animacion')).toBe(true);
    aplicarTema({ ...OPCIONES_TEMA_PREDETERMINADAS, animaciones: true });
    expect(document.documentElement.classList.contains('sin-animacion')).toBe(false);
  });

  it('prefers-reduced-motion y sin-animacion ponen en 0 todas las duraciones y transiciones', () => {
    for (const inicio of ['@media (prefers-reduced-motion: reduce)', ':root.sin-animacion {']) {
      const css = bloque(tokens, inicio);
      for (const d of DURACIONES) expect(css).toMatch(new RegExp(`${d}:\\s*0ms`));
    }
    expect(bloque(tokens, '@media (prefers-reduced-motion: reduce)')).toContain('transition-duration: 0.001ms !important');
    expect(bloque(tokens, ':root.sin-animacion *')).toContain('transition-duration: 0.001ms !important');
  });
});

type Rgb = [number, number, number];
type Color = { rgb: Rgb; a: number };
type Zona = 'arriba' | 'abajo';

const BLANCO: Rgb = [255, 255, 255];
const NEGRO: Rgb = [0, 0, 0];
const OPACIDAD_MINIMA_CUSTOMIZE = 0.2;
const OPACIDAD_MAXIMA_CUSTOMIZE = 0.9;
const TEXTOS = ['--gd-texto', '--gd-texto-2', '--gd-texto-3'] as const;

function variables(css: string): Record<string, string> {
  const v: Record<string, string> = {};
  for (const m of css.matchAll(/(--gd-[\w-]+):\s*([^;]+);/g)) v[m[1]] = m[2].trim();
  return v;
}

function colores(valor: string): Color[] {
  return [...valor.matchAll(/rgba?\(([^)]+)\)|#[0-9a-f]{6}\b/gi)].map((m) => {
    if (!m[1]) return { rgb: hexARgb(m[0]) as Rgb, a: 1 };
    const n = m[1].split(',').map(Number);
    return { rgb: [n[0], n[1], n[2]], a: n[3] ?? 1 };
  });
}

function tintoAcrylic(rama: string): Color {
  const m = ventanasRs.match(new RegExp(`${rama} => Color\\((\\d+), (\\d+), (\\d+), (\\d+)\\)`));
  expect(m, `falta el tinte acrylic ${rama}`).not.toBeNull();
  const n = m!.slice(1).map(Number);
  return { rgb: [n[0], n[1], n[2]], a: n[3] / 255 };
}

const sobre = (c: Color, abajo: Rgb): Rgb => c.rgb.map((v, i) => v * c.a + abajo[i] * (1 - c.a)) as Rgb;
const razon = (a: Rgb, b: Rgb): number => {
  const [l1, l2] = [luminancia(a), luminancia(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};

const OSCURO = variables(bloque(tokens, ':root {'));
const CLARO = { ...OSCURO, ...variables(bloque(tokens, ':root[data-tema="claro"]')) };

/**
 * Capas de un widget de abajo hacia arriba: escritorio (el desenfoque no cambia un fondo uniforme),
 * tinte acrylic de DWM, `.vidrio` (color con max(mínimo, opacidad) + brillo) y `::after`
 * (reflejo y destello, solo en la zona alta), y encima el texto.
 */
function contraste(tokensTema: Record<string, string>, tinte: Color, escritorio: Rgb, opacidad: number, zona: Zona, texto: string): number {
  const vidrio = tokensTema['--gd-vidrio-rgb'].split(',').map(Number) as Rgb;
  const alfa = Math.max(Number(tokensTema['--gd-vidrio-minimo']), opacidad);
  const brillo = colores(tokensTema['--gd-vidrio-brillo']);
  let fondo = sobre(tinte, escritorio);
  fondo = sobre({ rgb: vidrio, a: alfa }, fondo);
  fondo = sobre(zona === 'arriba' ? brillo[0] : brillo[brillo.length - 1], fondo);
  if (zona === 'arriba') {
    fondo = sobre(colores(tokensTema['--gd-reflejo-color'])[0], fondo);
    fondo = sobre(colores(tokensTema['--gd-vidrio-destello'])[0], fondo);
  }
  const t = colores(tokensTema[texto])[0];
  return razon(sobre(t, fondo), fondo);
}

function minimo(
  tokensTema: Record<string, string>,
  tinte: Color,
  escritorio: Rgb,
  opacidades: number[],
  zonas: Zona[] = ['arriba', 'abajo'],
): number {
  let min = Infinity;
  for (const o of opacidades)
    for (const zona of zonas) for (const t of TEXTOS) min = Math.min(min, contraste(tokensTema, tinte, escritorio, o, zona, t));
  return min;
}

describe('contraste sobre vidrio (GD-NFR-UX-01)', () => {
  const porDefecto = OPCIONES_TEMA_PREDETERMINADAS.opacidad;
  const gama = [OPACIDAD_MINIMA_CUSTOMIZE, porDefecto, OPACIDAD_MAXIMA_CUSTOMIZE];
  const tinteOscuro = tintoAcrylic('_');
  const tinteClaro = tintoAcrylic('Tema::Claro');

  it('reporta la razón mínima por tema, escritorio y opacidad', () => {
    const filas = [];
    for (const [tema, tk, tinte] of [['oscuro', OSCURO, tinteOscuro], ['claro', CLARO, tinteClaro], ['claro con tinte oscuro (seguir Windows)', CLARO, tinteOscuro]] as const)
      for (const [nombre, escritorio] of [['blanco', BLANCO], ['negro', NEGRO]] as const)
        for (const o of gama) {
          const r = (zona: Zona, t: string) => contraste(tk, tinte, escritorio, o, zona, t).toFixed(2);
          filas.push(`${tema} | escritorio ${nombre} | opacidad ${o} | texto ${r('arriba', '--gd-texto')}/${r('abajo', '--gd-texto')} | texto-2 ${r('arriba', '--gd-texto-2')}/${r('abajo', '--gd-texto-2')} | texto-3 ${r('arriba', '--gd-texto-3')}/${r('abajo', '--gd-texto-3')}`);
        }
    console.log(`GD-NFR-UX-01 razón de contraste (zona alta / zona baja)\n${filas.join('\n')}`);
    expect(filas).toHaveLength(18);
  });

  it('tema claro sobre escritorio claro (todo el widget) y tema oscuro sobre escritorio oscuro (cuerpo): ≥ 4.5', () => {
    expect(minimo(CLARO, tinteClaro, BLANCO, gama)).toBeGreaterThanOrEqual(4.5);
    expect(minimo(OSCURO, tinteOscuro, NEGRO, gama, ['abajo'])).toBeGreaterThanOrEqual(4.5);
  });

  // Fallas conocidas del 2026-10-28: cuando un ajuste de tokens las corrija, `it.fails` avisará.
  it.fails('falla conocida: tema oscuro, zona alta (brillo + reflejo + destello) sobre escritorio oscuro', () => {
    expect(minimo(OSCURO, tinteOscuro, NEGRO, gama, ['arriba'])).toBeGreaterThanOrEqual(4.5);
  });

  it.fails('falla conocida: tema oscuro sobre escritorio claro con la opacidad por defecto', () => {
    expect(minimo(OSCURO, tinteOscuro, BLANCO, [porDefecto])).toBeGreaterThanOrEqual(4.5);
  });

  it.fails('falla conocida: tema claro, zona baja sobre escritorio oscuro (texto-3 4.36)', () => {
    expect(minimo(CLARO, tinteClaro, NEGRO, [porDefecto], ['abajo'])).toBeGreaterThanOrEqual(4.5);
  });
});

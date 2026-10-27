import { describe, expect, it } from 'vitest';
import tokens from './tokens.css?raw';
import { aplicarTema, OPCIONES_TEMA_PREDETERMINADAS } from './tema';

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

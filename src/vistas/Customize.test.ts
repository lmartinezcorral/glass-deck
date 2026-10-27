import { describe, expect, it } from 'vitest';
import { combinacionDeTecla } from './Customize';

const tecla = (code: string, key: string, mods: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) => ({
  code,
  key,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe('combinacionDeTecla', () => {
  it('arma la combinación en el formato del plugin de atajos', () => {
    expect(combinacionDeTecla(tecla('KeyG', 'g', { metaKey: true, altKey: true }))).toBe('Alt+Super+G');
    expect(combinacionDeTecla(tecla('Digit5', '5', { ctrlKey: true, shiftKey: true }))).toBe('Ctrl+Shift+5');
    expect(combinacionDeTecla(tecla('F9', 'F9', { ctrlKey: true }))).toBe('Ctrl+F9');
  });

  it('ignora teclas sin modificador o solo modificadores', () => {
    expect(combinacionDeTecla(tecla('KeyG', 'g'))).toBeNull();
    expect(combinacionDeTecla(tecla('AltLeft', 'Alt', { altKey: true }))).toBeNull();
    expect(combinacionDeTecla(tecla('Comma', ',', { ctrlKey: true }))).toBeNull();
  });
});

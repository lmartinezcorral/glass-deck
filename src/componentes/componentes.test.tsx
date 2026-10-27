import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BarraWidget } from './BarraWidget';
import { Pildora } from './Pildora';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let contenedor: HTMLDivElement;
let raiz: Root;

beforeEach(() => {
  contenedor = document.createElement('div');
  document.body.appendChild(contenedor);
  raiz = createRoot(contenedor);
});

afterEach(() => {
  act(() => raiz.unmount());
  contenedor.remove();
});

describe('BarraWidget (GD-WIN-05)', () => {
  it('muestra el título y 4 botones con aria-label que responden al clic', () => {
    const acciones = { onPin: vi.fn(), onMinimizar: vi.fn(), onMaximizar: vi.fn(), onCerrar: vi.fn() };
    act(() => raiz.render(<BarraWidget titulo="CPU" icono={<svg />} fijado={false} {...acciones} />));

    expect(contenedor.querySelector('h1')?.textContent).toBe('CPU');
    const botones = [...contenedor.querySelectorAll('button')];
    expect(botones).toHaveLength(4);
    for (const b of botones) {
      expect(b.getAttribute('aria-label')?.trim()).toBeTruthy();
      act(() => b.click());
    }
    for (const f of Object.values(acciones)) expect(f).toHaveBeenCalledTimes(1);
  });
});

describe('Pildora (GD-WIN-06)', () => {
  it('muestra icono y dato clave, y restaura al hacer clic', () => {
    const onRestaurar = vi.fn();
    act(() => raiz.render(<Pildora icono={<svg data-icono="" />} texto="CPU 23 %" onRestaurar={onRestaurar} />));

    expect(contenedor.querySelector('[data-icono]')).not.toBeNull();
    expect(contenedor.querySelector('.gd-pildora__texto')?.textContent).toBe('CPU 23 %');
    act(() => contenedor.querySelector('button')?.click());
    expect(onRestaurar).toHaveBeenCalledTimes(1);
  });
});

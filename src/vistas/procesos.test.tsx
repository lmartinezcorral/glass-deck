import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FilaProceso } from '../api';
import { Dialogo } from '../componentes';
import { BarraPanel } from './Limpieza';
import { FilaTablaProceso, filtrarYOrdenar, ListaTopProcesos } from './Procesos';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const fila = (pid: number, nombre: string, cpu: number, ram: number, extra: Partial<FilaProceso> = {}): FilaProceso => ({
  pid,
  nombre,
  cpu,
  ram,
  usuario: 'usuario-a',
  nivel: 'normal',
  reiniciar_explorer: false,
  ...extra,
});

const FILAS: FilaProceso[] = [
  fila(30, 'notepad.exe', 1, 300, { usuario: 'b' }),
  fila(10, 'chrome.exe', 9, 900, { usuario: 'c' }),
  fila(20, 'Notepad.exe', 5, 100, { usuario: 'a' }),
];

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

function escribir(input: HTMLInputElement, valor: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  act(() => {
    setter?.call(input, valor);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

describe('lista completa (GD-PROC-02)', () => {
  it('busca por nombre sin distinguir mayúsculas y por PID exacto', () => {
    expect(filtrarYOrdenar(FILAS, 'notepad', 'pid', true).map((f) => f.pid)).toEqual([20, 30]);
    expect(filtrarYOrdenar(FILAS, '10', 'pid', true).map((f) => f.pid)).toEqual([10]);
  });

  it('ordena por nombre, PID, CPU, RAM y usuario en ambos sentidos', () => {
    const pids = (o: Parameters<typeof filtrarYOrdenar>[2], asc: boolean) => filtrarYOrdenar(FILAS, '', o, asc).map((f) => f.pid);
    expect(pids('pid', true)).toEqual([10, 20, 30]);
    expect(pids('cpu', false)).toEqual([10, 20, 30]);
    expect(pids('ram', true)).toEqual([20, 30, 10]);
    expect(pids('usuario', true)).toEqual([20, 30, 10]);
    expect(pids('nombre', true)[0]).toBe(10);
    expect(FILAS.map((f) => f.pid)).toEqual([30, 10, 20]);
  });
});

describe('top 5 (GD-PROC-01)', () => {
  it('conserva los nodos de cada proceso al actualizar, sin volver a montarlos', () => {
    act(() => raiz.render(<ListaTopProcesos filas={[FILAS[1]!, FILAS[2]!]} orden="cpu" />));
    const antes = contenedor.querySelector('li[data-pid="20"]');
    act(() => raiz.render(<ListaTopProcesos filas={[fila(20, 'Notepad.exe', 12, 100), fila(10, 'chrome.exe', 3, 900)]} orden="cpu" />));
    const despues = contenedor.querySelector('li[data-pid="20"]');
    expect(despues).toBe(antes);
    expect(despues?.textContent).toContain('12 %');
  });
});

describe('fila de la tabla (GD-PROC-03, GD-PROC-09)', () => {
  const renderFila = (f: FilaProceso, onReiniciar = vi.fn()) =>
    act(() =>
      raiz.render(
        <table>
          <tbody>
            <FilaTablaProceso fila={f} marcada={false} onAlternar={vi.fn()} onReiniciarExplorer={onReiniciar} />
          </tbody>
        </table>,
      ),
    );

  it('explorer muestra el enlace para reiniciarlo y no la casilla', () => {
    const onReiniciar = vi.fn();
    renderFila(fila(60, 'explorer.exe', 1, 1, { nivel: 'protegido', reiniciar_explorer: true }), onReiniciar);
    expect(contenedor.querySelector('input[type="checkbox"]')).toBeNull();
    const enlace = [...contenedor.querySelectorAll('button')].find((b) => b.textContent === 'Reiniciar Explorer');
    expect(enlace).toBeDefined();
    act(() => enlace?.click());
    expect(onReiniciar).toHaveBeenCalledTimes(1);
  });

  it('un protegido muestra candado y no casilla; uno normal sí tiene casilla', () => {
    renderFila(fila(700, 'lsass.exe', 0, 1, { nivel: 'protegido' }));
    expect(contenedor.querySelector('.gd-tabla__candado')).not.toBeNull();
    expect(contenedor.querySelector('input[type="checkbox"]')).toBeNull();
    renderFila(fila(800, 'notepad.exe', 0, 1));
    expect(contenedor.querySelector('input[type="checkbox"]')).not.toBeNull();
  });
});

describe('diálogo de cierre (GD-PROC-04, GD-PROC-05)', () => {
  const elementos = [
    { id: '1', titulo: 'Cursor.exe' },
    { id: '2', titulo: 'notepad.exe' },
  ];

  it('lista los seleccionados y cancelar no confirma', () => {
    const onConfirmar = vi.fn();
    const onCancelar = vi.fn();
    act(() =>
      raiz.render(<Dialogo abierto peligro titulo="¿Cerrar?" elementos={elementos} onConfirmar={onConfirmar} onCancelar={onCancelar} />),
    );
    const nombres = [...contenedor.querySelectorAll('.gd-dialogo__elemento b')].map((b) => b.textContent);
    expect(nombres).toEqual(['Cursor.exe', 'notepad.exe']);
    const cancelar = [...contenedor.querySelectorAll('button')].find((b) => b.textContent === 'Cancelar');
    act(() => cancelar?.click());
    expect(onCancelar).toHaveBeenCalledTimes(1);
    expect(onConfirmar).not.toHaveBeenCalled();
  });

  it('el botón sigue deshabilitado hasta escribir CERRAR exacto y entrega lo escrito', () => {
    const onConfirmar = vi.fn();
    act(() =>
      raiz.render(
        <Dialogo
          abierto
          peligro
          titulo="¿Cerrar?"
          elementos={elementos}
          confirmacionEscrita="CERRAR"
          textoConfirmar="Cerrar"
          onConfirmar={onConfirmar}
          onCancelar={vi.fn()}
        />,
      ),
    );
    const campo = contenedor.querySelector<HTMLInputElement>('.gd-dialogo__escrito');
    const boton = [...contenedor.querySelectorAll('button')].find((b) => b.textContent === 'Cerrar');
    expect(campo).not.toBeNull();
    expect(boton?.disabled).toBe(true);
    escribir(campo!, 'cerrar');
    expect(boton?.disabled).toBe(true);
    escribir(campo!, 'CERRAR');
    expect(boton?.disabled).toBe(false);
    act(() => boton?.click());
    expect(onConfirmar).toHaveBeenCalledWith('CERRAR');
  });
});

describe('BarraPanel', () => {
  it('muestra título y subtítulo, y el botón cierra', () => {
    const onCerrar = vi.fn();
    act(() => raiz.render(<BarraPanel titulo="Customize" subtitulo="Los cambios se aplican al instante" onCerrar={onCerrar} />));
    expect(contenedor.querySelector('h1')?.textContent).toBe('Customize');
    expect(contenedor.querySelector('.gd-panel__subtitulo')?.textContent).toBe('Los cambios se aplican al instante');
    act(() => contenedor.querySelector<HTMLButtonElement>('[aria-label="Cerrar Customize"]')?.click());
    expect(onCerrar).toHaveBeenCalledOnce();
  });
});

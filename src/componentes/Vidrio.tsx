import {
  createElement,
  forwardRef,
  useCallback,
  useEffect,
  useRef,
  type HTMLAttributes,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react';

export type ElementoVidrio = 'div' | 'section' | 'article' | 'aside' | 'header' | 'main';

export interface VidrioProps extends HTMLAttributes<HTMLElement> {
  elemento?: ElementoVidrio;
  reflejo?: boolean;
  solido?: boolean;
  arrastrando?: boolean;
  fijado?: boolean;
  lleno?: boolean;
  entrada?: boolean;
  children?: ReactNode;
}

export const Vidrio = forwardRef<HTMLElement, VidrioProps>(function Vidrio(
  {
    elemento = 'div',
    reflejo = true,
    solido = false,
    arrastrando = false,
    fijado = false,
    lleno = false,
    entrada = false,
    className,
    children,
    onPointerMove,
    ...resto
  },
  ref,
) {
  const nodo = useRef<HTMLElement | null>(null);
  const cuadro = useRef(0);
  const puntero = useRef({ x: 0, y: 0 });

  const asignarRef = useCallback(
    (el: HTMLElement | null) => {
      nodo.current = el;
      if (typeof ref === 'function') ref(el);
      else if (ref) ref.current = el;
    },
    [ref],
  );

  useEffect(() => () => cancelAnimationFrame(cuadro.current), []);

  const manejarMovimiento = (e: ReactPointerEvent<HTMLElement>): void => {
    onPointerMove?.(e);
    if (!reflejo || arrastrando) return;
    puntero.current.x = e.clientX;
    puntero.current.y = e.clientY;
    if (cuadro.current) return;
    cuadro.current = requestAnimationFrame(() => {
      cuadro.current = 0;
      const el = nodo.current;
      if (!el) return;
      const caja = el.getBoundingClientRect();
      el.style.setProperty('--mx', `${Math.round(puntero.current.x - caja.left)}px`);
      el.style.setProperty('--my', `${Math.round(puntero.current.y - caja.top)}px`);
    });
  };

  const clases = [
    'vidrio',
    solido && 'vidrio--solido',
    arrastrando && 'vidrio--arrastrando',
    fijado && 'vidrio--fijado',
    lleno && 'vidrio--lleno',
    entrada && 'vidrio--entrada',
    className,
  ]
    .filter(Boolean)
    .join(' ');

  return createElement(
    elemento,
    { ...resto, ref: asignarRef, className: clases, onPointerMove: manejarMovimiento },
    children,
  );
});

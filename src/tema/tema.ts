export type Tema = 'oscuro' | 'claro' | 'sistema';
export type TemaResuelto = Exclude<Tema, 'sistema'>;

export interface OpcionesTema {
  tema: Tema;
  acento: string;
  opacidad: number;
  desenfoque: number;
  animaciones: boolean;
}

export const ACENTOS: ReadonlyArray<{ nombre: string; valor: string }> = [
  { nombre: 'Cian eléctrico', valor: '#22d3ee' },
  { nombre: 'Violeta', valor: '#a78bfa' },
  { nombre: 'Verde', valor: '#4ade80' },
  { nombre: 'Ámbar', valor: '#fbbf24' },
  { nombre: 'Rosa', valor: '#f472b6' },
  { nombre: 'Azul', valor: '#60a5fa' },
];

export const OPCIONES_TEMA_PREDETERMINADAS: OpcionesTema = {
  tema: 'oscuro',
  acento: '#22d3ee',
  opacidad: 0.44,
  desenfoque: 24,
  animaciones: true,
};

const CONSULTA_OSCURO = '(prefers-color-scheme: dark)';
const TEXTO_OSCURO = '#03141b';
const TEXTO_CLARO = '#ffffff';
const LUMINANCIA_CORTE = 0.19;

let temaPreferido: Tema = OPCIONES_TEMA_PREDETERMINADAS.tema;

const limitar = (v: number, min: number, max: number): number =>
  Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : min;

export function hexARgb(hex: string): [number, number, number] | null {
  const limpio = hex.trim().replace(/^#/, '');
  const completo = limpio.length === 3 ? limpio.replace(/./g, (c) => c + c) : limpio;
  if (!/^[0-9a-f]{6}$/i.test(completo)) return null;
  const n = Number.parseInt(completo, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function luminancia([r, g, b]: [number, number, number]): number {
  const canal = (v: number): number => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * canal(r) + 0.7152 * canal(g) + 0.0722 * canal(b);
}

function temaDelSistema(): TemaResuelto {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'oscuro';
  return window.matchMedia(CONSULTA_OSCURO).matches ? 'oscuro' : 'claro';
}

export function resolverTema(tema: Tema): TemaResuelto {
  return tema === 'sistema' ? temaDelSistema() : tema;
}

export function aplicarTema(opts: OpcionesTema): TemaResuelto {
  const raiz = document.documentElement;
  const resuelto = resolverTema(opts.tema);
  temaPreferido = opts.tema;

  raiz.dataset.tema = resuelto;
  raiz.dataset.temaPreferido = opts.tema;

  const rgb = hexARgb(opts.acento);
  if (rgb) {
    const hex = `#${rgb.map((c) => c.toString(16).padStart(2, '0')).join('')}`;
    raiz.style.setProperty('--gd-acento', hex);
    raiz.style.setProperty('--gd-acento-rgb', rgb.join(', '));
    raiz.style.setProperty(
      '--gd-acento-contraste',
      luminancia(rgb) > LUMINANCIA_CORTE ? TEXTO_OSCURO : TEXTO_CLARO,
    );
  }

  raiz.style.setProperty('--gd-vidrio-opacidad', String(limitar(opts.opacidad, 0, 1)));
  raiz.style.setProperty('--gd-desenfoque', `${limitar(opts.desenfoque, 0, 80)}px`);
  raiz.classList.toggle('sin-animacion', !opts.animaciones);

  return resuelto;
}

export function seguirTemaSistema(alCambiar?: (tema: TemaResuelto) => void): () => void {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return () => undefined;
  const consulta = window.matchMedia(CONSULTA_OSCURO);
  const manejar = (): void => {
    if (temaPreferido !== 'sistema') return;
    const resuelto: TemaResuelto = consulta.matches ? 'oscuro' : 'claro';
    document.documentElement.dataset.tema = resuelto;
    alCambiar?.(resuelto);
  };
  consulta.addEventListener('change', manejar);
  return () => consulta.removeEventListener('change', manejar);
}

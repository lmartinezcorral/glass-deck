import { lazy, Suspense, useEffect, useMemo, type ReactNode } from 'react';
import { api, enTauri, EVENTOS } from './api';
import { useConfig, useEvento, usePrivacidad } from './hooks';
import { aplicarTema, OPCIONES_TEMA_PREDETERMINADAS, seguirTemaSistema } from './tema/tema';
import { VistaCpu } from './vistas/Cpu';
import { VistaDisco } from './vistas/Disco';
import { VistaProcesos } from './vistas/Procesos';
import { VistaRam } from './vistas/Ram';
import { VistaRed } from './vistas/Red';
import { VistaDock } from './vistas/Dock';
import { VistaSpending } from './vistas/Spending';
import { VistaClima } from './vistas/Clima';
import { VistaCorreo } from './vistas/Correo';
import { VistaPoc } from './vistas/Poc';

const VistaCustomize = lazy(() => import('./vistas/Customize').then((m) => ({ default: m.VistaCustomize })));
const VistaLimpieza = lazy(() => import('./vistas/Limpieza').then((m) => ({ default: m.VistaLimpieza })));
const Galeria = lazy(() => import('./vistas/Galeria'));

function vistaPara(nombre: string, parametros: URLSearchParams): ReactNode {
  switch (nombre) {
    case 'cpu':
      return <VistaCpu />;
    case 'ram':
      return <VistaRam />;
    case 'disco':
      return <VistaDisco />;
    case 'red':
      return <VistaRed />;
    case 'procesos':
      return <VistaProcesos />;
    case 'spending':
      return <VistaSpending />;
    case 'clima':
      return <VistaClima />;
    case 'correo':
      return <VistaCorreo />;
    case 'dock':
      return <VistaDock />;
    case 'customize':
      return <VistaCustomize />;
    case 'limpieza':
      return <VistaLimpieza />;
    case 'poc':
      return <VistaPoc parametros={parametros} />;
    default:
      return <Galeria />;
  }
}

export function App() {
  const parametros = useMemo(() => new URLSearchParams(window.location.search), []);
  const vista = parametros.get('vista') ?? 'galeria';
  const [config] = useConfig();
  const bateria = useEvento<boolean | null>(EVENTOS.bateria, null);
  const modoPrivado = usePrivacidad(config);
  const enBateria = bateria ?? false;

  useEffect(() => {
    document.documentElement.dataset.vista = vista;
    if (!enTauri() || vista === 'galeria') return;
    api
      .estadoEntorno()
      .then((e) => {
        if (e.con_bateria) document.documentElement.dataset.bateria = '1';
      })
      .catch(() => undefined);
  }, [vista]);

  useEffect(() => {
    if (vista === 'galeria') return;
    const apariencia = config?.apariencia ?? OPCIONES_TEMA_PREDETERMINADAS;
    const ahorrar = (config?.pausar_con_bateria ?? true) && (enBateria || document.documentElement.dataset.bateria === '1');
    const sinTransicion = parametros.get('modo') === 'sin-transicion';
    aplicarTema({ ...apariencia, animaciones: !ahorrar && !sinTransicion });
  }, [config, enBateria, vista, parametros]);

  useEffect(() => {
    if (bateria !== null) document.documentElement.dataset.bateria = bateria ? '1' : '0';
  }, [bateria]);

  useEffect(() => seguirTemaSistema(), []);

  useEffect(() => {
    document.documentElement.toggleAttribute('data-privacidad', modoPrivado);
  }, [modoPrivado]);

  return <Suspense fallback={null}>{vistaPara(vista, parametros)}</Suspense>;
}

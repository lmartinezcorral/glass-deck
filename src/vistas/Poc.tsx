import { useEffect, useState } from 'react';
import { Medidor, Sparkline, Vidrio } from '../componentes';
import { agregarAcotado } from '../formato';

const INTERVALO_MS = 1000;
const PUNTOS = 60;

function useSimulado(semilla: number, estatico: boolean): [number, number[]] {
  const [valor, setValor] = useState(20 + ((semilla * 7) % 50));
  const [serie, setSerie] = useState<number[]>(() => Array.from({ length: PUNTOS }, (_, i) => 30 + ((i * 13 + semilla * 5) % 40)));
  useEffect(() => {
    if (estatico) return;
    const id = window.setInterval(() => {
      const siguiente = Math.min(100, Math.max(0, valor + (Math.random() - 0.5) * 18));
      setValor(siguiente);
      setSerie((s) => agregarAcotado(s, siguiente, PUNTOS));
    }, INTERVALO_MS);
    return () => window.clearInterval(id);
  }, [estatico, valor]);
  return [valor, serie];
}

function TarjetaPoc({ n, estatico }: { n: number; estatico: boolean }) {
  const [valor, serie] = useSimulado(n, estatico);
  return (
    <div className="gd-poc__tarjeta">
      <div className="gd-poc__titulos">
        <b>Widget de prueba {n + 1}</b>
        <span className="gd-etiqueta">Datos simulados · prueba de RAM</span>
      </div>
      <Medidor valor={valor} etiqueta={`Widget de prueba ${n + 1}`} subetiqueta="%" tamano={84} />
      <Sparkline datos={serie} maxPuntos={PUNTOS} max={100} etiqueta={`Serie ${n + 1}`} />
    </div>
  );
}

export function VistaPoc({ parametros }: { parametros: URLSearchParams }) {
  const agrupados = Number(parametros.get('agrupados') ?? 0);
  const n = Number(parametros.get('n') ?? 0);
  const modo = parametros.get('modo') ?? '';
  const estatico = modo === 'estatico';

  if (agrupados > 0) {
    return (
      <Vidrio elemento="main" lleno className="gd-poc gd-poc--agrupado" data-tauri-drag-region="">
        {Array.from({ length: agrupados }, (_, i) => (
          <Vidrio key={i} className="gd-poc__celda">
            <TarjetaPoc n={i} estatico={estatico} />
          </Vidrio>
        ))}
      </Vidrio>
    );
  }
  return (
    <Vidrio elemento="main" lleno className="gd-poc" data-tauri-drag-region="">
      <TarjetaPoc n={n} estatico={estatico} />
    </Vidrio>
  );
}

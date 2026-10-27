import { useEffect, useState } from 'react';
import { Medidor, Sparkline, Vidrio } from '../componentes';
import { agregarAcotado } from '../formato';

const INTERVALO_MS = 1000;
const PUNTOS = 60;

function useSimulado(semilla: number): [number, number[]] {
  const [valor, setValor] = useState(20 + (semilla * 7) % 50);
  const [serie, setSerie] = useState<number[]>([]);
  useEffect(() => {
    const id = window.setInterval(() => {
      setValor((v) => {
        const siguiente = Math.min(100, Math.max(0, v + (Math.random() - 0.5) * 18));
        setSerie((s) => agregarAcotado(s, siguiente, PUNTOS));
        return siguiente;
      });
    }, INTERVALO_MS);
    return () => window.clearInterval(id);
  }, []);
  return [valor, serie];
}

function TarjetaPoc({ n }: { n: number }) {
  const [valor, serie] = useSimulado(n);
  return (
    <div className="gd-poc__tarjeta">
      <Medidor valor={valor} etiqueta={`Widget de prueba ${n + 1}`} subetiqueta="%" tamano={84} />
      <Sparkline datos={serie} maxPuntos={PUNTOS} max={100} etiqueta={`Serie ${n + 1}`} />
    </div>
  );
}

export function VistaPoc({ parametros }: { parametros: URLSearchParams }) {
  const agrupados = Number(parametros.get('agrupados') ?? 0);
  const n = Number(parametros.get('n') ?? 0);
  if (agrupados > 0) {
    return (
      <Vidrio elemento="main" lleno className="gd-poc gd-poc--agrupado" data-tauri-drag-region="">
        {Array.from({ length: agrupados }, (_, i) => (
          <Vidrio key={i} className="gd-poc__celda">
            <TarjetaPoc n={i} />
          </Vidrio>
        ))}
      </Vidrio>
    );
  }
  return (
    <Vidrio elemento="main" lleno className="gd-poc" data-tauri-drag-region="">
      <TarjetaPoc n={n} />
    </Vidrio>
  );
}

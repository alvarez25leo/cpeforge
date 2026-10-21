'use client';
import { useEffect, useMemo, useState } from 'react';
import { Search } from 'lucide-react';
import { formatoNumero } from '@/lib/numeros';
import { basePath } from '@/lib/shared';

type Codigo = [codigo: string, mensaje: string];

type Rango = 'todos' | 'excepcion' | 'rechazo' | 'observacion';

const rangos: { id: Rango; nombre: string; detalle: string }[] = [
  { id: 'todos', nombre: 'Todos', detalle: '' },
  { id: 'excepcion', nombre: 'Excepciones', detalle: '0100–1999' },
  { id: 'rechazo', nombre: 'Rechazos', detalle: '2000–3999' },
  { id: 'observacion', nombre: 'Observaciones', detalle: '4000+' },
];

/** Clasificación de `TaxStatus` según el código. */
function rangoDe(codigo: string): Exclude<Rango, 'todos'> | null {
  const n = Number(codigo);
  if (n >= 100 && n <= 1999) return 'excepcion';
  if (n >= 2000 && n <= 3999) return 'rechazo';
  if (n >= 4000) return 'observacion';
  return null;
}

const estilos: Record<Exclude<Rango, 'todos'>, { texto: string; clase: string }> = {
  excepcion: { texto: 'Exception', clase: 'border-fd-warning/50 text-fd-warning' },
  rechazo: { texto: 'Rejected', clase: 'border-fd-error/50 text-fd-error' },
  observacion: { texto: 'AcceptedWithObservations', clase: 'border-fd-info/50 text-fd-info' },
};

function normalizar(texto: string) {
  return texto.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

const MAX_VISIBLES = 100;

/** Buscador de los códigos de retorno de SUNAT de `cpeforge-xcodes`. */
export function XcodesSearch() {
  const [codigos, setCodigos] = useState<Codigo[] | null>(null);
  const [error, setError] = useState(false);
  const [consulta, setConsulta] = useState('');
  const [rango, setRango] = useState<Rango>('todos');

  useEffect(() => {
    let vigente = true;
    fetch(`${basePath}/data/xcodes.json`)
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
      .then((data: { codigos: Codigo[] }) => vigente && setCodigos(data.codigos))
      .catch(() => vigente && setError(true));
    return () => {
      vigente = false;
    };
  }, []);

  const indexados = useMemo(
    () => codigos?.map(([c, m]) => ({ c, m, texto: normalizar(`${c} ${m}`), rango: rangoDe(c) })) ?? [],
    [codigos],
  );

  const resultados = useMemo(() => {
    const q = normalizar(consulta.trim());
    const palabras = q.split(/\s+/).filter(Boolean);
    const esCodigo = /^\d+$/.test(q);
    const lista = indexados.filter((item) => {
      if (rango !== 'todos' && item.rango !== rango) return false;
      if (esCodigo) return item.c.includes(q);
      return palabras.every((p) => item.texto.includes(p));
    });
    // Un código exacto primero (con o sin ceros a la izquierda).
    if (esCodigo) {
      lista.sort((a, b) => Number(Number(b.c) === Number(q)) - Number(Number(a.c) === Number(q)));
    }
    return lista;
  }, [indexados, consulta, rango]);

  const boton = (activo: boolean) =>
    `rounded-full border px-3 py-1 text-xs font-medium transition-colors ${
      activo
        ? 'border-fd-primary bg-fd-primary text-fd-primary-foreground'
        : 'bg-fd-card text-fd-muted-foreground hover:bg-fd-accent'
    }`;

  return (
    <div className="not-prose my-6">
      <label className="mb-3 flex items-center gap-2 rounded-lg border bg-fd-card px-3 py-2 focus-within:ring-2 focus-within:ring-fd-ring">
        <Search className="size-4 shrink-0 text-fd-muted-foreground" aria-hidden="true" />
        <span className="sr-only">Buscar un código de SUNAT</span>
        <input
          type="search"
          value={consulta}
          onChange={(event) => setConsulta(event.target.value)}
          placeholder="Escribe un código (2017, 0100) o palabras del mensaje (RUC, fecha, IGV)…"
          className="w-full bg-transparent text-sm outline-none placeholder:text-fd-muted-foreground"
        />
      </label>

      <div className="mb-4 flex flex-wrap gap-2" role="group" aria-label="Filtrar por tipo de código">
        {rangos.map((r) => (
          <button
            key={r.id}
            type="button"
            aria-pressed={rango === r.id}
            className={boton(rango === r.id)}
            onClick={() => setRango(r.id)}
          >
            {r.nombre}
            {r.detalle && <span className="ml-1 opacity-70">{r.detalle}</span>}
          </button>
        ))}
      </div>

      {error ? (
        <p className="rounded-lg border bg-fd-card p-6 text-center text-sm text-fd-muted-foreground">
          No se pudo cargar el catálogo de códigos.
        </p>
      ) : codigos === null ? (
        <p className="rounded-lg border bg-fd-card p-6 text-center text-sm text-fd-muted-foreground">
          Cargando los códigos…
        </p>
      ) : (
        <>
          <p className="mb-3 text-xs text-fd-muted-foreground" aria-live="polite">
            {resultados.length === 0
              ? 'Ningún código coincide con la búsqueda.'
              : resultados.length > MAX_VISIBLES
                ? `${formatoNumero(resultados.length)} códigos; se muestran los primeros ${MAX_VISIBLES}. Afina la búsqueda para ver el resto.`
                : `${formatoNumero(resultados.length)} ${resultados.length === 1 ? 'código' : 'códigos'}`}
          </p>
          <ul className="divide-y rounded-xl border bg-fd-card">
            {resultados.slice(0, MAX_VISIBLES).map((item) => (
              <li key={item.c} className="flex flex-col gap-1 px-4 py-3 sm:flex-row sm:items-start sm:gap-4">
                <code className="w-14 shrink-0 font-mono text-sm font-semibold">{item.c}</code>
                <span className="flex-1 text-sm">{item.m}</span>
                {item.rango && (
                  <span
                    className={`w-fit shrink-0 rounded-full border px-2 py-0.5 font-mono text-[11px] ${estilos[item.rango].clase}`}
                  >
                    {estilos[item.rango].texto}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

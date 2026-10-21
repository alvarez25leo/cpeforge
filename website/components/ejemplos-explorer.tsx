'use client';
import Link from 'next/link';
import { useMemo, useState } from 'react';
import { Search } from 'lucide-react';
import type { Ejemplo, Grupo } from '@/lib/ejemplos';

const chip = 'inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium';

/** Quita tildes y pasa a minúsculas para buscar sin importar la escritura. */
function normalizar(texto: string) {
  return texto.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

export function EjemplosExplorerClient({
  ejemplos,
  grupos,
}: {
  ejemplos: Ejemplo[];
  grupos: Grupo[];
}) {
  const [consulta, setConsulta] = useState('');
  const [grupo, setGrupo] = useState<Grupo | null>(null);
  const [soloRust, setSoloRust] = useState(false);

  const visibles = useMemo(() => {
    const palabras = normalizar(consulta).split(/\s+/).filter(Boolean);
    return ejemplos.filter((e) => {
      if (grupo && e.grupo !== grupo) return false;
      if (soloRust && !e.soloRust) return false;
      let texto = normalizar(`${e.id} ${e.titulo} ${e.descripcion} ${e.via}`);
      // Quien busca «dólares» espera también los casos en USD.
      if (/\busd\b/.test(texto)) texto += ' dolares';
      return palabras.every((p) => texto.includes(p));
    });
  }, [ejemplos, consulta, grupo, soloRust]);

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
        <span className="sr-only">Buscar un caso</span>
        <input
          type="search"
          value={consulta}
          onChange={(event) => setConsulta(event.target.value)}
          placeholder="Busca por nombre o tema: detracción, ICBPER, crédito, dólares…"
          className="w-full bg-transparent text-sm outline-none placeholder:text-fd-muted-foreground"
        />
      </label>

      <div className="mb-4 flex flex-wrap gap-2" role="group" aria-label="Filtrar por tipo">
        <button type="button" className={boton(grupo === null)} onClick={() => setGrupo(null)}>
          Todos ({ejemplos.length})
        </button>
        {grupos.map((g) => (
          <button
            key={g}
            type="button"
            className={boton(grupo === g)}
            aria-pressed={grupo === g}
            onClick={() => setGrupo(grupo === g ? null : g)}
          >
            {g} ({ejemplos.filter((e) => e.grupo === g).length})
          </button>
        ))}
        <button
          type="button"
          className={boton(soloRust)}
          aria-pressed={soloRust}
          onClick={() => setSoloRust(!soloRust)}
        >
          Solo en CPEForge
        </button>
      </div>

      <p className="mb-3 text-xs text-fd-muted-foreground" aria-live="polite">
        {visibles.length === ejemplos.length
          ? `${ejemplos.length} casos`
          : `${visibles.length} de ${ejemplos.length} casos`}
      </p>

      {visibles.length === 0 ? (
        <p className="rounded-lg border bg-fd-card p-6 text-center text-sm text-fd-muted-foreground">
          Ningún caso coincide con la búsqueda.
        </p>
      ) : (
        <ul className="grid gap-3 sm:grid-cols-2">
          {visibles.map((e) => (
            <li key={e.id} className="flex flex-col gap-2 rounded-xl border bg-fd-card p-4">
              <div className="flex items-start justify-between gap-2">
                <Link href={e.url} className="font-medium hover:text-fd-primary">
                  {e.titulo}
                </Link>
                <span className="shrink-0 text-xs text-fd-muted-foreground">{e.grupo}</span>
              </div>
              <code className="w-fit rounded bg-fd-muted px-1.5 py-0.5 text-xs">{e.id}</code>
              <p className="flex-1 text-sm text-fd-muted-foreground">{e.descripcion}</p>
              <div className="flex flex-wrap items-center gap-2">
                <span className={chip}>{e.via}</span>
                {e.soloRust && (
                  <span className={`${chip} border-fd-primary/40 text-fd-primary`}>Solo en CPEForge</span>
                )}
                {e.soloXml && <span className={chip}>Solo genera XML</span>}
                <span className="ml-auto flex gap-3 text-xs">
                  <Link href={e.url} className="text-fd-primary underline-offset-4 hover:underline">
                    Guía
                  </Link>
                  <a
                    href={e.github}
                    className="text-fd-muted-foreground underline-offset-4 hover:underline"
                  >
                    Código
                  </a>
                </span>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

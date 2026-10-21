'use client';
import { use, useEffect, useId, useState } from 'react';
import { useTheme } from 'next-themes';

/** Diagrama Mermaid renderizado en el cliente, con el tema claro u oscuro activo. */
export function Mermaid({ chart }: { chart: string }) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) return <div className="mermaid-diagram min-h-24" aria-busy="true" />;
  return <MermaidContent chart={chart} />;
}

const cache = new Map<string, Promise<unknown>>();

function cachePromise<T>(key: string, create: () => Promise<T>): Promise<T> {
  const cached = cache.get(key);
  if (cached) return cached as Promise<T>;

  const promise = create();
  cache.set(key, promise);
  return promise;
}

function MermaidContent({ chart }: { chart: string }) {
  // Mermaid usa el id como selector CSS: solo letras y números.
  const id = `mermaid-${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const { resolvedTheme } = useTheme();
  const { default: mermaid } = use(cachePromise('mermaid', () => import('mermaid')));

  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    fontFamily: 'inherit',
    themeCSS: 'margin: 1.5rem auto 0;',
    theme: resolvedTheme === 'dark' ? 'dark' : 'default',
  });

  const { svg, bindFunctions } = use(
    cachePromise(`${chart}-${resolvedTheme}`, () =>
      mermaid.render(id, chart.replaceAll('\\n', '\n')),
    ),
  );

  return (
    <div
      className="mermaid-diagram my-6"
      ref={(container) => {
        if (container) bindFunctions?.(container);
      }}
      // El SVG lo genera Mermaid desde un texto escrito en el propio sitio.
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}

import { DynamicCodeBlock } from 'fumadocs-ui/components/dynamic-codeblock';
import { comandoEjemplo, ejemplos, getEjemplo, grupos, type Ejemplo } from '@/lib/ejemplos';
import { EjemplosExplorerClient } from '@/components/ejemplos-explorer';

const chip = 'inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium';

export function EtiquetasEjemplo({ ejemplo }: { ejemplo: Ejemplo }) {
  return (
    <>
      <span className={chip}>{ejemplo.via}</span>
      {ejemplo.retenciones && <span className={chip}>Servicio de retenciones</span>}
      {ejemplo.soloRust && (
        <span className={`${chip} border-fd-primary/40 text-fd-primary`}>Solo en CPEForge</span>
      )}
      {ejemplo.soloXml && <span className={chip}>Solo genera XML</span>}
    </>
  );
}

/** Ficha de un caso de ejemplo: etiquetas, cómo ejecutarlo y enlace al código. */
export function Caso({ id }: { id: string }) {
  const ejemplo = getEjemplo(id);
  const comandos = ejemplo.soloXml
    ? comandoEjemplo(id)
    : `${comandoEjemplo(id)}\n${comandoEjemplo(id, true)}`;
  return (
    <div className="not-prose my-4 rounded-xl border bg-fd-card p-4">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <code className="rounded bg-fd-primary/10 px-1.5 py-0.5 text-sm font-semibold text-fd-primary">
          {ejemplo.id}
        </code>
        <EtiquetasEjemplo ejemplo={ejemplo} />
        <a
          href={ejemplo.github}
          className="ml-auto text-xs text-fd-muted-foreground underline-offset-4 hover:underline"
        >
          Ver en GitHub
        </a>
      </div>
      <p className="mb-3 text-sm text-fd-muted-foreground">{ejemplo.descripcion}</p>
      <DynamicCodeBlock lang="bash" code={comandos} />
    </div>
  );
}

/** Explorador filtrable de los casos del proyecto de ejemplo. */
export function EjemplosExplorer() {
  return <EjemplosExplorerClient ejemplos={ejemplos} grupos={grupos} />;
}

/** Tabla de los casos de una guía, con enlace a cada sección. */
export function CasosDeGuia({ guia }: { guia: string }) {
  const lista = ejemplos.filter((e) => e.guia === guia);
  return (
    <table>
      <thead>
        <tr>
          <th>Caso</th>
          <th>Qué muestra</th>
        </tr>
      </thead>
      <tbody>
        {lista.map((e) => (
          <tr key={e.id}>
            <td>
              <a href={`#${e.id.toLowerCase()}`}>
                <code>{e.id}</code>
              </a>
            </td>
            <td>{e.titulo}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

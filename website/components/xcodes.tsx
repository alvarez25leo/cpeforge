import { formatoNumero } from '@/lib/numeros';
import { mensajeCodigo, xcodesConteo, xcodesVersion } from '@/lib/xcodes';

/** Un código de SUNAT con su mensaje oficial, leído del catálogo. */
export function CodigoSunat({ codigo }: { codigo: string }) {
  return (
    <>
      <code>{codigo}</code>: «{mensajeCodigo(codigo)}»
    </>
  );
}

/** Cifras del catálogo de códigos de retorno. */
export function XcodesResumen() {
  const cifras = [
    { valor: xcodesConteo.total, texto: 'códigos en total' },
    { valor: xcodesConteo.excepciones, texto: 'excepciones (0100–1999)' },
    { valor: xcodesConteo.rechazos, texto: 'rechazos (2000–3999)' },
    { valor: xcodesConteo.observaciones, texto: 'observaciones (4000+)' },
  ];
  return (
    <div className="not-prose my-4">
      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {cifras.map((c) => (
          <div key={c.texto} className="rounded-xl border bg-fd-card p-3 text-center">
            <dd className="text-2xl font-bold tabular-nums text-fd-primary">
              {formatoNumero(c.valor)}
            </dd>
            <dt className="text-xs text-fd-muted-foreground">{c.texto}</dt>
          </div>
        ))}
      </dl>
      <p className="mt-2 text-xs text-fd-muted-foreground">
        Reglas de validación de SUNAT actualizadas al {xcodesVersion}.
      </p>
    </div>
  );
}

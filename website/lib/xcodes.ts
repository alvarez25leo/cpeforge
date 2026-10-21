import data from '@/generated/xcodes.json';

/** Versión de las reglas de validación de SUNAT del catálogo. */
export const xcodesVersion: string = data.version;

/** Número de códigos del catálogo, por rango de `TaxStatus`. */
export const xcodesConteo = (() => {
  let excepciones = 0;
  let rechazos = 0;
  let observaciones = 0;
  for (const [codigo] of data.codigos) {
    const n = Number(codigo);
    if (n >= 100 && n <= 1999) excepciones++;
    else if (n >= 2000 && n <= 3999) rechazos++;
    else if (n >= 4000) observaciones++;
  }
  return { total: data.codigos.length, excepciones, rechazos, observaciones };
})();

/** Mensaje oficial de un código, para citarlo en la documentación. */
export function mensajeCodigo(codigo: string): string {
  const entrada = data.codigos.find(([c]) => c === codigo);
  if (!entrada) throw new Error(`Código de SUNAT desconocido en la documentación: ${codigo}`);
  return entrada[1];
}

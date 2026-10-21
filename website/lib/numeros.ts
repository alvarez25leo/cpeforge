/**
 * Número en el estilo del sitio: miles separados por un espacio fino no
 * separable y coma decimal (`2 077`, `63,6`), igual en el servidor y en el
 * navegador, sin depender de los datos de idioma del entorno.
 */
export function formatoNumero(valor: number, decimales?: number): string {
  const texto = decimales === undefined ? String(valor) : valor.toFixed(decimales);
  const [entero, fraccion] = texto.split('.');
  const conMiles = entero.replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
  return fraccion ? `${conMiles},${fraccion}` : conMiles;
}

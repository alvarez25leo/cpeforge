import data from '@/generated/ejemplos.json';
import { repoFile } from '@/lib/shared';

export type Grupo =
  | 'Facturas'
  | 'Boletas'
  | 'Notas'
  | 'Resúmenes, bajas y reversiones'
  | 'Retención y percepción'
  | 'Liquidaciones de compra'
  | 'Guías de remisión';

export type Via = 'sendBill' | 'sendSummary' | 'API de guías';

interface Ficha {
  titulo: string;
  grupo: Grupo;
  /** Página de la guía donde se explica, sin `/docs/guias/` ni el ancla. */
  guia: string;
  via: Via;
  /** Servicio SOAP de retenciones y percepciones en lugar del de facturas. */
  retenciones?: boolean;
  /** Caso propio de CPEForge, sin equivalente en la referencia PHP. */
  soloRust?: boolean;
  /** Solo genera el XML: nunca se envía. */
  soloXml?: boolean;
}

/**
 * Dónde se documenta cada caso de `example/rust/src/casos`. Un caso nuevo sin
 * ficha rompe el build, para que no quede fuera del explorador.
 */
const fichas: Record<string, Ficha> = {
  'inicio-factura-gravada': { titulo: 'Factura gravada al contado', grupo: 'Facturas', guia: 'factura', via: 'sendBill' },
  factura: { titulo: 'Factura gravada y exonerada', grupo: 'Facturas', guia: 'factura', via: 'sendBill' },
  'factura-rel-guia-remision': { titulo: 'Factura con guía de remisión relacionada', grupo: 'Facturas', guia: 'factura', via: 'sendBill' },
  'factura-beatose': { titulo: 'Factura con errores intencionales', grupo: 'Facturas', guia: 'factura', via: 'sendBill', soloXml: true },
  'inicio-factura-exonerada': { titulo: 'Factura exonerada', grupo: 'Facturas', guia: 'factura/afectacion-igv', via: 'sendBill' },
  'factura-exonerado': { titulo: 'Factura exonerada (demo)', grupo: 'Facturas', guia: 'factura/afectacion-igv', via: 'sendBill' },
  'factura-inafecta': { titulo: 'Factura inafecta', grupo: 'Facturas', guia: 'factura/afectacion-igv', via: 'sendBill' },
  'factura-exportacion': { titulo: 'Factura de exportación en USD', grupo: 'Facturas', guia: 'factura/afectacion-igv', via: 'sendBill' },
  'factura-gratuita': { titulo: 'Transferencia gratuita', grupo: 'Facturas', guia: 'factura/afectacion-igv', via: 'sendBill' },
  'factura-compleja': { titulo: 'Gravada, exonerada, inafecta y gratuitas', grupo: 'Facturas', guia: 'factura/afectacion-igv', via: 'sendBill' },
  'factura-credito': { titulo: 'Factura a crédito en dos cuotas', grupo: 'Facturas', guia: 'factura/credito', via: 'sendBill' },
  'inicio-factura-credito': { titulo: 'Crédito con líneas mixtas y bolsa plástica', grupo: 'Facturas', guia: 'factura/credito', via: 'sendBill' },
  'factura-detraccion': { titulo: 'Operación sujeta a detracción', grupo: 'Facturas', guia: 'factura/detraccion-percepcion-retencion', via: 'sendBill' },
  'factura-percepcion': { titulo: 'Venta sujeta a percepción', grupo: 'Facturas', guia: 'factura/detraccion-percepcion-retencion', via: 'sendBill' },
  'factura-retencion': { titulo: 'Factura con retención del 3 %', grupo: 'Facturas', guia: 'factura/detraccion-percepcion-retencion', via: 'sendBill' },
  'factura-anticipo': { titulo: 'Factura con deducción de anticipo', grupo: 'Facturas', guia: 'factura/anticipos', via: 'sendBill' },
  'factura-descuento-linea': { titulo: 'Descuento por línea que afecta la base', grupo: 'Facturas', guia: 'factura/descuentos-y-cargos', via: 'sendBill' },
  'factura-descuento-global': { titulo: 'Descuento global que afecta la base', grupo: 'Facturas', guia: 'factura/descuentos-y-cargos', via: 'sendBill' },
  'factura-descuento-nobase': { titulo: 'Descuentos que no afectan la base', grupo: 'Facturas', guia: 'factura/descuentos-y-cargos', via: 'sendBill' },
  'factura-cargos': { titulo: 'Recargo al consumo del 10 %', grupo: 'Facturas', guia: 'factura/descuentos-y-cargos', via: 'sendBill' },
  'factura-icbper': { titulo: 'Bolsas plásticas (ICBPER) y redondeo', grupo: 'Facturas', guia: 'factura/impuestos-especiales', via: 'sendBill' },
  'factura-icbper-gratuita': { titulo: 'Bolsa plástica gratuita (ICBPER)', grupo: 'Facturas', guia: 'factura/impuestos-especiales', via: 'sendBill' },
  'factura-isc': { titulo: 'ISC, sistema al valor', grupo: 'Facturas', guia: 'factura/impuestos-especiales', via: 'sendBill' },
  'factura-ivap': { titulo: 'Venta sujeta al IVAP', grupo: 'Facturas', guia: 'factura/impuestos-especiales', via: 'sendBill' },
  'factura-contingencia': { titulo: 'Factura de contingencia', grupo: 'Facturas', guia: 'factura/contingencia', via: 'sendBill' },
  boleta: { titulo: 'Boleta a persona con DNI', grupo: 'Boletas', guia: 'boleta', via: 'sendBill' },
  'inicio-boleta': { titulo: 'Boleta con varias líneas y bolsa plástica', grupo: 'Boletas', guia: 'boleta', via: 'sendBill' },
  'boleta-icbper': { titulo: 'Boleta con ICBPER y redondeo', grupo: 'Boletas', guia: 'boleta', via: 'sendBill' },
  'nota-credito': { titulo: 'Nota de crédito por devolución', grupo: 'Notas', guia: 'notas', via: 'sendBill' },
  'nota-credito-boleta': { titulo: 'Nota de crédito que anula una boleta', grupo: 'Notas', guia: 'notas', via: 'sendBill' },
  'nota-credito-forma-pago': { titulo: 'Ajuste de montos y fechas de pago', grupo: 'Notas', guia: 'notas', via: 'sendBill' },
  'nota-debito': { titulo: 'Nota de débito por aumento en el valor', grupo: 'Notas', guia: 'notas', via: 'sendBill' },
  resumen: { titulo: 'Resumen diario de boletas y nota', grupo: 'Resúmenes, bajas y reversiones', guia: 'resumen-diario', via: 'sendSummary' },
  'resumen-contingencia': { titulo: 'Resumen con boleta de contingencia', grupo: 'Resúmenes, bajas y reversiones', guia: 'resumen-diario', via: 'sendSummary' },
  'resumen-usd': { titulo: 'Resumen diario en dólares', grupo: 'Resúmenes, bajas y reversiones', guia: 'resumen-diario', via: 'sendSummary' },
  'comunicacion-baja': { titulo: 'Comunicación de baja', grupo: 'Resúmenes, bajas y reversiones', guia: 'comunicacion-baja', via: 'sendSummary' },
  reversion: { titulo: 'Reversión de retención y percepción', grupo: 'Resúmenes, bajas y reversiones', guia: 'reversion', via: 'sendSummary', retenciones: true },
  'reversion-liquidacion': { titulo: 'Reversión de una liquidación de compra', grupo: 'Resúmenes, bajas y reversiones', guia: 'reversion', via: 'sendSummary', retenciones: true, soloRust: true },
  retencion: { titulo: 'Comprobante de retención', grupo: 'Retención y percepción', guia: 'retencion-percepcion', via: 'sendBill', retenciones: true },
  percepcion: { titulo: 'Comprobante de percepción', grupo: 'Retención y percepción', guia: 'retencion-percepcion', via: 'sendBill', retenciones: true },
  'liquidacion-compra': { titulo: 'Liquidación de compra (compra interna)', grupo: 'Liquidaciones de compra', guia: 'liquidacion-compra', via: 'sendBill', soloRust: true },
  'liquidacion-compra-oro': { titulo: 'Liquidación de compra de oro', grupo: 'Liquidaciones de compra', guia: 'liquidacion-compra', via: 'sendBill', soloRust: true },
  'guia-remision': { titulo: 'Guía remitente, transporte público', grupo: 'Guías de remisión', guia: 'guia-remitente', via: 'API de guías' },
  'guia-transportePrivado': { titulo: 'Guía remitente, transporte privado', grupo: 'Guías de remisión', guia: 'guia-remitente', via: 'API de guías' },
  'guia-transporteM1L': { titulo: 'Guía remitente en vehículo M1 o L', grupo: 'Guías de remisión', guia: 'guia-remitente', via: 'API de guías' },
  'guia-misma-empresa': { titulo: 'Traslado entre establecimientos propios', grupo: 'Guías de remisión', guia: 'guia-remitente', via: 'API de guías' },
  'guia-extra-atributos': { titulo: 'Guía con documento adicional y atributos', grupo: 'Guías de remisión', guia: 'guia-remitente', via: 'API de guías' },
  'guia-transportista': { titulo: 'Guía de remisión transportista', grupo: 'Guías de remisión', guia: 'guia-transportista', via: 'API de guías', soloRust: true },
};

export interface Ejemplo extends Ficha {
  id: string;
  /** Descripción del propio caso (primer párrafo de su comentario `//!`). */
  descripcion: string;
  /** Ruta del archivo en el repositorio. */
  archivo: string;
  /** URL de la guía con el ancla del caso. */
  url: string;
  github: string;
}

export const grupos: Grupo[] = [
  'Facturas',
  'Boletas',
  'Notas',
  'Resúmenes, bajas y reversiones',
  'Retención y percepción',
  'Liquidaciones de compra',
  'Guías de remisión',
];

export const ejemplos: Ejemplo[] = data.ejemplos.map((e) => {
  const ficha = fichas[e.id];
  if (!ficha) {
    throw new Error(`El caso de ejemplo «${e.id}» no tiene ficha en lib/ejemplos.ts`);
  }
  return {
    ...ficha,
    id: e.id,
    descripcion: e.description,
    archivo: e.file,
    url: `/docs/guias/${ficha.guia}#${e.id.toLowerCase()}`,
    github: repoFile(e.file),
  };
});

for (const id of Object.keys(fichas)) {
  if (!data.ejemplos.some((e) => e.id === id)) {
    throw new Error(`lib/ejemplos.ts tiene una ficha para «${id}», que ya no existe`);
  }
}

/** Nombres de los casos de rechazo (`--errores`). */
export const rechazos: string[] = data.rechazos;

export function getEjemplo(id: string): Ejemplo {
  const ejemplo = ejemplos.find((e) => e.id === id);
  if (!ejemplo) throw new Error(`Caso de ejemplo desconocido en la documentación: ${id}`);
  return ejemplo;
}

export function comandoEjemplo(id: string, enviar = false) {
  const flag = enviar ? ' --enviar' : '';
  return `cargo run --release --manifest-path example/rust/Cargo.toml --${flag} ${id}`;
}

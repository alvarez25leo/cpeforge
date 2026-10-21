import data from '@/generated/crates.json';

export interface CrateInfo {
  name: string;
  dir: string;
  version: string;
  description: string;
  msrv: string;
  features: string[];
  examples: string[];
}

export const crates: CrateInfo[] = data.crates;
export const msrv: string = data.msrv;
/** Número de casos de `example/rust/src/casos`. */
export const exampleCases: number = data.casos;

/** Orden de presentación: del paquete principal a los de apoyo. */
const order = [
  'cpeforge',
  'cpeforge-core',
  'cpeforge-xml',
  'cpeforge-xmldsig',
  'cpeforge-ws',
  'cpeforge-gre-api',
  'cpeforge-report',
  'cpeforge-xml-parser',
  'cpeforge-validator',
  'cpeforge-xcodes',
  'cpeforge-ubl-validator',
  'cpeforge-cpe-validator',
  'cpeforge-xml-limits',
];

export const orderedCrates: CrateInfo[] = [...crates].sort(
  (a, b) => order.indexOf(a.name) - order.indexOf(b.name),
);

export function getCrate(name: string): CrateInfo {
  const crate = crates.find((c) => c.name === name);
  if (!crate) throw new Error(`Crate desconocido en la documentación: ${name}`);
  return crate;
}

/** Requisito de Cargo `mayor.menor` (o `0.menor` en 0.x), como en los README. */
export function cargoRequirement(version: string): string {
  const [major, minor] = version.split('.');
  return major === '0' ? `0.${minor}` : `${major}.${minor}`;
}

export const cratesIo = (name: string) => `https://crates.io/crates/${name}`;
export const docsRs = (name: string) => `https://docs.rs/${name}`;

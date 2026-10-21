// Genera `generated/crates.json` a partir de los Cargo.toml del workspace,
// para que el sitio nunca muestre versiones escritas a mano.
//
// Uso: node scripts/generate.mjs (se ejecuta antes de `dev` y `build`).

import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const website = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repo = path.resolve(website, '..');
const cratesDir = path.join(repo, 'crates');

/** Lee las claves `clave = "valor"` de una sección TOML (sin dependencias). */
function section(toml, name) {
  const lines = toml.split(/\r?\n/);
  const values = {};
  let inside = false;
  for (const line of lines) {
    const header = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (header) {
      inside = header[1] === name;
      continue;
    }
    if (!inside) continue;
    const kv = line.match(/^\s*([A-Za-z0-9_.-]+)\s*=\s*(.+?)\s*$/);
    if (kv) values[kv[1]] = kv[2];
  }
  return values;
}

const unquote = (value) => value?.match(/^"(.*)"$/)?.[1];

async function examplesOf(dir) {
  try {
    const files = await readdir(path.join(dir, 'examples'));
    return files.filter((f) => f.endsWith('.rs')).map((f) => f.replace(/\.rs$/, '')).sort();
  } catch {
    return [];
  }
}

const workspace = section(await readFile(path.join(repo, 'Cargo.toml'), 'utf8'), 'workspace.package');
const defaultMsrv = unquote(workspace['rust-version']);

const crates = [];
for (const dir of (await readdir(cratesDir)).sort()) {
  const crateDir = path.join(cratesDir, dir);
  let toml;
  try {
    toml = await readFile(path.join(crateDir, 'Cargo.toml'), 'utf8');
  } catch {
    continue;
  }
  const pkg = section(toml, 'package');
  const features = Object.keys(section(toml, 'features')).filter((f) => f !== 'default');
  crates.push({
    name: unquote(pkg.name),
    dir,
    version: unquote(pkg.version),
    description: unquote(pkg.description),
    msrv: unquote(pkg['rust-version']) ?? defaultMsrv,
    features,
    examples: await examplesOf(crateDir),
  });
}

// Casos del proyecto de ejemplo: la tabla `CASOS` de `casos/mod.rs` da el
// nombre de cada caso y su módulo; el primer párrafo `//!` del módulo, su
// descripción.
const ejemploSrc = path.join(repo, 'example', 'rust', 'src');
const casosDir = path.join(ejemploSrc, 'casos');
const modRs = await readFile(path.join(casosDir, 'mod.rs'), 'utf8');

/** Pares `("nombre", modulo::documento)` de una tabla de casos. */
function caseTable(source) {
  return [...source.matchAll(/\(\s*"([^"]+)",\s*([a-z0-9_]+)(?:::documento)?,?\s*\)/g)].map(
    ([, id, fn]) => ({ id, fn }),
  );
}

/** Primer párrafo de los comentarios `//!` de un archivo, en una línea. */
function docSummary(source) {
  const lines = [];
  for (const line of source.split(/\r?\n/)) {
    if (!line.startsWith('//!')) break;
    const text = line.replace(/^\/\/!\s?/, '');
    if (text.trim() === '') break;
    lines.push(text.trim());
  }
  return lines.join(' ');
}

const ejemplos = [];
for (const { id, fn: module } of caseTable(modRs)) {
  const file = `example/rust/src/casos/${module}.rs`;
  const source = await readFile(path.join(repo, file), 'utf8');
  ejemplos.push({ id, module, file, description: docSummary(source) });
}
if (ejemplos.length === 0) throw new Error('No se encontró la tabla CASOS en casos/mod.rs');

// Casos de rechazo (`CASOS_ERROR` de errores.rs).
const erroresRs = await readFile(path.join(ejemploSrc, 'errores.rs'), 'utf8');
const rechazos = caseTable(erroresRs.slice(erroresRs.indexOf('CASOS_ERROR'))).map((c) => c.id);

// Catálogo de códigos de retorno de SUNAT (`cpeforge-xcodes`).
const xcodesDir = path.join(cratesDir, 'xcodes', 'src');
const xcodesLib = await readFile(path.join(xcodesDir, 'lib.rs'), 'utf8');
const xmlEntities = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };
const codigos = [
  ...(await readFile(path.join(xcodesDir, 'data', 'CodeErrors.xml'), 'utf8')).matchAll(
    /<error code="([^"]+)">([\s\S]*?)<\/error>/g,
  ),
].map(([, code, text]) => [code, text.replace(/&(\w+);/g, (m, e) => xmlEntities[e] ?? m).trim()]);
const expectedCount = Number(xcodesLib.match(/ENTRY_COUNT: usize = ([\d_]+)/)?.[1].replace(/_/g, ''));
if (codigos.length !== expectedCount) {
  throw new Error(`CodeErrors.xml tiene ${codigos.length} códigos; ENTRY_COUNT dice ${expectedCount}`);
}
const xcodes = {
  version: xcodesLib.match(/CATALOG_VERSION: &str = "([^"]+)"/)?.[1] ?? '',
  codigos,
};

const out = path.join(website, 'generated');
await mkdir(out, { recursive: true });
await writeFile(path.join(out, 'xcodes.json'), `${JSON.stringify(xcodes)}\n`);
// El buscador de códigos lo descarga en el navegador, fuera del HTML de la página.
const publicData = path.join(website, 'public', 'data');
await mkdir(publicData, { recursive: true });
await writeFile(path.join(publicData, 'xcodes.json'), JSON.stringify(xcodes));
await writeFile(
  path.join(out, 'crates.json'),
  `${JSON.stringify({ msrv: defaultMsrv, casos: ejemplos.length, crates }, null, 2)}\n`,
);
await writeFile(
  path.join(out, 'ejemplos.json'),
  `${JSON.stringify({ ejemplos, rechazos }, null, 2)}\n`,
);
console.log(
  `generated/: ${crates.length} crates, ${ejemplos.length} casos de ejemplo, ${rechazos.length} de rechazo, ${codigos.length} códigos SUNAT`,
);

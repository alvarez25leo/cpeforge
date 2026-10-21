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

// Casos del proyecto de ejemplo (`example/rust/src/casos/*.rs`).
const casosDir = path.join(repo, 'example', 'rust', 'src', 'casos');
const casos = (await readdir(casosDir)).filter((f) => f.endsWith('.rs') && f !== 'mod.rs').length;

const out = path.join(website, 'generated');
await mkdir(out, { recursive: true });
await writeFile(
  path.join(out, 'crates.json'),
  `${JSON.stringify({ msrv: defaultMsrv, casos, crates }, null, 2)}\n`,
);
console.log(`generated/crates.json: ${crates.length} crates, ${casos} casos de ejemplo`);

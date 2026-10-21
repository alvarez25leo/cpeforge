// Next 16 exporta los fragmentos RSC de cada ruta en carpetas
// (`docs/x/__next.docs/$oc$slug/__PAGE__.txt`), pero el router del cliente los
// pide con puntos (`docs/x/__next.docs.$oc$slug.__PAGE__.txt`). Sin esa copia,
// la precarga devuelve 404 en un hosting estático como GitHub Pages.
//
// Uso: node scripts/flatten-rsc.mjs (se ejecuta después de `next build`).

import { copyFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const out = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'out');
let copied = 0;

/** Copia cada archivo de `dir` (carpeta `__next.*`) como `<base>.<ruta con puntos>`. */
async function flatten(dir, parent, prefix) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const name = `${prefix}.${entry.name}`;
    if (entry.isDirectory()) {
      await flatten(full, parent, name);
    } else {
      await copyFile(full, path.join(parent, name));
      copied++;
    }
  }
}

async function walk(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name === '_next') continue;
    const full = path.join(dir, entry.name);
    if (entry.name.startsWith('__next.')) await flatten(full, dir, entry.name);
    else await walk(full);
  }
}

await walk(out);
console.log(`flatten-rsc: ${copied} fragmentos RSC copiados`);

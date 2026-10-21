// Comprueba los enlaces internos del sitio exportado (out/): cada página
// enlazada existe y cada ancla (#id), propia o de otra página, tiene su id.
//
// Uso: node scripts/check-links.mjs (después de `pnpm build`). Sale con 1 si
// encuentra algún enlace roto.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const out = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'out');
const basePath = process.env.DOCS_BASE_PATH ?? '';

/** Ruta de out/ → index.html de cada página. */
const pages = new Map();
(function walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== '_next') walk(full);
    } else if (entry.name === 'index.html') {
      const route = `/${path.relative(out, path.dirname(full)).split(path.sep).join('/')}`;
      pages.set(route === '/.' ? '/' : route.replace(/\/?$/, '/'), full);
    }
  }
})(out);

const ids = new Map();
function idsOf(route) {
  if (!ids.has(route)) {
    const html = readFileSync(pages.get(route), 'utf8');
    ids.set(route, new Set([...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1])));
  }
  return ids.get(route);
}

const broken = [];
for (const [route, file] of pages) {
  const html = readFileSync(file, 'utf8');
  for (const [, href] of html.matchAll(/href="([^"]+)"/g)) {
    if (/^(https?:|mailto:)/.test(href) || href.startsWith('//')) continue;
    const [rawPath, rawHash] = href.split('#');
    if (rawPath.startsWith(`${basePath}/_next`) || rawPath.endsWith('.svg')) continue;

    let target = route;
    if (rawPath) {
      if (!rawPath.startsWith('/')) continue;
      const local = decodeURI(rawPath.slice(basePath.length) || '/');
      if (/\.[a-z0-9]+$/i.test(local)) {
        if (!existsSync(path.join(out, local))) broken.push(`${route} → ${href}`);
        continue;
      }
      target = local.replace(/\/?$/, '/');
      if (!pages.has(target)) {
        broken.push(`${route} → ${href} (página inexistente)`);
        continue;
      }
    }
    if (rawHash && !idsOf(target).has(decodeURIComponent(rawHash))) {
      broken.push(`${route} → ${href} (ancla inexistente)`);
    }
  }
}

console.log(`check-links: ${pages.size} páginas revisadas`);
if (broken.length > 0) {
  console.error([...new Set(broken)].join('\n'));
  process.exit(1);
}

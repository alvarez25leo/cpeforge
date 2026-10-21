import { fileURLToPath } from 'node:url';
import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();

// Raíz del repositorio: las páginas incluyen código de ../example y ../crates.
const repoRoot = fileURLToPath(new URL('..', import.meta.url));

// En GitHub Pages el sitio vive en /cpeforge; en local, en la raíz.
const basePath = process.env.DOCS_BASE_PATH ?? '';

/** @type {import('next').NextConfig} */
const config = {
  output: 'export',
  reactStrictMode: true,
  basePath,
  trailingSlash: true,
  images: { unoptimized: true },
  turbopack: { root: repoRoot },
  env: {
    NEXT_PUBLIC_BASE_PATH: basePath,
  },
};

export default withMDX(config);

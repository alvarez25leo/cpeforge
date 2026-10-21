import { createGetUrl } from 'fumadocs-core/source';

export const appName = 'CPEForge';
export const appDescription =
  'Facturación electrónica SUNAT en Rust: construye, firma y envía comprobantes de pago electrónicos de Perú.';
export const docsRoute = '/docs';
export const docsImageRoute = '/og/docs';
export const docsContentRoute = '/llms.mdx/docs';

/** Prefijo del sitio publicado (`/cpeforge` en GitHub Pages, vacío en local). */
export const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? '';

/** URL pública del sitio, para metadatos y Open Graph. */
export const siteUrl = process.env.DOCS_SITE_URL ?? 'https://alvarez25leo.github.io';

export const gitConfig = {
  user: 'alvarez25leo',
  repo: 'cpeforge',
  branch: 'main',
};

export const repoUrl = `https://github.com/${gitConfig.user}/${gitConfig.repo}`;

/** Enlace a un archivo del repositorio en GitHub. */
export function repoFile(file: string) {
  return `${repoUrl}/blob/${gitConfig.branch}/${file}`;
}

const getContentUrl = createGetUrl(docsContentRoute);

export function getPageMarkdownUrl(page: { slugs: string[]; locale?: string }) {
  const segments = [...page.slugs, 'content.md'];

  return { segments, url: getContentUrl(segments, page.locale) };
}

const getImageUrl = createGetUrl(docsImageRoute);

export function getPageImageUrl(page: { slugs: string[]; locale?: string }) {
  const segments = [...page.slugs, 'image.png'];

  return { segments, url: getImageUrl(segments, page.locale) };
}

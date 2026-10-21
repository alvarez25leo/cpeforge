import type { MetadataRoute } from 'next';
import { source } from '@/lib/source';
import { basePath, siteUrl } from '@/lib/shared';

export const dynamic = 'force-static';

/** URL absoluta de una ruta del sitio, con la barra final de `trailingSlash`. */
function absoluta(ruta: string) {
  return `${siteUrl}${basePath}${ruta.replace(/\/?$/, '/')}`;
}

export default function sitemap(): MetadataRoute.Sitemap {
  return [
    { url: absoluta('/'), priority: 1 },
    ...source.getPages().map((page) => ({
      url: absoluta(page.url),
      priority: page.url === '/docs' ? 0.9 : 0.7,
    })),
  ];
}

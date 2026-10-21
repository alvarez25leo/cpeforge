# Documentación de CPEForge

Sitio de documentación de CPEForge, hecho con [Fumadocs](https://fumadocs.dev)
y Next.js en modo de export estático. Se publica en GitHub Pages con el
workflow `.github/workflows/docs.yml`.

## Desarrollo

Requiere Node 24 y pnpm 11.

```bash
cd website
pnpm install
pnpm dev        # http://localhost:3000
```

```bash
pnpm build      # genera el sitio estático en out/
pnpm start      # sirve out/ en local
pnpm types:check
```

Para probar el sitio tal como queda en GitHub Pages:

```bash
DOCS_BASE_PATH=/cpeforge pnpm build
```

Para revisar el resultado exactamente como en GitHub Pages:

```bash
DOCS_BASE_PATH=/cpeforge pnpm build && DOCS_BASE_PATH=/cpeforge pnpm check:links
```

En Git Bash (Windows), añade `MSYS_NO_PATHCONV=1` delante: si no, convierte
`/cpeforge` en una ruta de Windows.

## Publicación

El workflow `.github/workflows/docs.yml` hace, en cada PR que toca el sitio:
`pnpm install --frozen-lockfile`, `pnpm types:check`, `pnpm build` y
`pnpm check:links`. En `main`, además, publica `out/` en GitHub Pages
(`https://alvarez25leo.github.io/cpeforge/`).

Para activarlo por primera vez, en GitHub: **Settings → Pages → Build and
deployment → Source: GitHub Actions**. Después basta con hacer push a `main` o
lanzar el workflow a mano (**Actions → Documentación → Run workflow**).

Con un dominio propio, cambia `DOCS_SITE_URL` y `DOCS_BASE_PATH` (vacío) en el
workflow.

## Estructura

| Ruta | Contenido |
| --- | --- |
| `content/docs/` | Páginas en MDX. El orden del menú lateral está en cada `meta.json` |
| `app/(home)/page.tsx` | Página de inicio |
| `components/` | Componentes propios para MDX (`CargoDeps`, `CrateGrid`, `Mermaid`, `BenchmarkTable`…) |
| `lib/crates.ts` | Lee `generated/crates.json` |
| `scripts/generate.mjs` | Antes de `dev` y `build`, genera desde el código: `generated/crates.json` (versiones), `generated/ejemplos.json` (casos de `example/rust`) y el catálogo de códigos SUNAT (`generated/xcodes.json` y `public/data/xcodes.json`) |
| `lib/ejemplos.ts` | Guía y grupo de cada caso de ejemplo. Un caso nuevo sin ficha hace fallar el build |
| `scripts/flatten-rsc.mjs` | Copia los fragmentos RSC con el nombre que pide el router en un hosting estático (después de `build`) |
| `scripts/check-links.mjs` | `pnpm check:links`: comprueba enlaces internos y anclas de `out/` |

## Escribir documentación

- **Versiones:** nunca las escribas a mano. Usa `<Version crate="cpeforge" />`,
  `<CargoDeps crates={['cpeforge']} />` o `<CrateBadges crate="cpeforge" />`.
- **Código de ejemplo:** incluye el archivo real en vez de copiarlo, para que
  no quede desactualizado. La ruta es relativa a `website/`:

  ```mdx
  <include cwd lang="rust" meta='title="example/rust/src/casos/factura.rs"'>../example/rust/src/casos/factura.rs</include>
  ```

  Para incluir solo un fragmento, márcalo en el `.rs` con `// #region nombre`
  y `// #endregion nombre`, y añade `#nombre` a la ruta.
- **Diagramas:** `<Mermaid chart={`flowchart LR ...`} />`.
- **Casos de ejemplo:** en una guía, `### Título [#id-del-caso]`, luego
  `<Caso id="id-del-caso" />` y el `<include>` del archivo. `<CasosDeGuia guia="ruta" />`
  lista los casos de una guía.
- **Códigos de SUNAT:** `<CodigoSunat codigo="2800" />` cita el mensaje oficial
  del catálogo (falla el build si el código no existe).
- **Enlaces internos:** rutas absolutas desde `/docs/...`; el prefijo de GitHub
  Pages se añade solo.

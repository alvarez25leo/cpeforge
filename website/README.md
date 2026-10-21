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

## Estructura

| Ruta | Contenido |
| --- | --- |
| `content/docs/` | Páginas en MDX. El orden del menú lateral está en cada `meta.json` |
| `app/(home)/page.tsx` | Página de inicio |
| `components/` | Componentes propios para MDX (`CargoDeps`, `CrateGrid`, `Mermaid`, `BenchmarkTable`…) |
| `lib/crates.ts` | Lee `generated/crates.json` |
| `scripts/generate.mjs` | Genera `generated/crates.json` desde los `Cargo.toml` (antes de `dev` y `build`) |
| `scripts/flatten-rsc.mjs` | Copia los fragmentos RSC con el nombre que pide el router en un hosting estático (después de `build`) |

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
- **Enlaces internos:** rutas absolutas desde `/docs/...`; el prefijo de GitHub
  Pages se añade solo.

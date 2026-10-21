# Plan: sitio de documentación de CPEForge con Fumadocs

> Fecha del análisis: 2026-10-21.
>
> **Estado (2026-10-21): F0–F4 hechas** en `website/`, más el workflow de
> GitHub Pages (`.github/workflows/docs.yml`) adelantado de F7. F4 añadió las 9
> páginas de conceptos y el índice más las 13 páginas de paquetes, cada una con
> su ejemplo real incluido. Decisiones tomadas: `website/` en el repo, GitHub
> Pages en `/cpeforge`, solo español, con la página de Greenter. Pendiente: F5
> (guías por comprobante y explorador de ejemplos), F6 (referencia y
> comunidad) y el resto de F7 (comprobación de enlaces en CI). Cambios
> respecto al plan:
>
> - Fumadocs MDX 15 ya no usa `source.config.ts`: las colecciones se definen en
>   `lib/source.ts` con `fumadocs-mdx/macro`.
> - `fumadocs-ui` se instala como `@fumadocs/base-ui` (variante de Base UI que
>   genera la plantilla oficial).
> - `<include>` ya admite archivos de código: no hizo falta un plugin propio.
>   Para leer `../example` y `../crates`, `next.config.mjs` fija
>   `turbopack.root` en la raíz del repo.
> - Next 16 exporta los fragmentos RSC en carpetas y el router los pide con
>   puntos: `scripts/flatten-rsc.mjs` crea esas copias después del build.
> - El logo es provisional (un comprobante con una chispa, en naranja).

---

## 1. Qué se analizó

### 1.1 El proyecto

CPEForge es un SDK en Rust (edition 2024, MSRV 1.85; 1.88 para `cpeforge` y
`cpeforge-ws`) para emitir comprobantes de pago electrónicos de Perú. Hace todo
el ciclo: construye el XML UBL, lo firma (XMLDSig), lo comprime en ZIP, lo
envía a SUNAT (SOAP o API REST de guías), lee el CDR, clasifica la respuesta y
genera la representación impresa en HTML con QR.

- Repositorio: `https://github.com/alvarez25leo/cpeforge`. Licencia MIT.
- Más de 370 pruebas. La CI corre en Rust 1.85, 1.88 y estable, más 9 jobs de
  paridad contra la referencia PHP.
- Es la referencia funcional de Greenter v5.3.0 portada a Rust, con
  identificadores propios (`CPEForgeSign`, `#CPEFORGE-SIGN`).

### 1.2 Los 13 crates publicados

Todos están publicados en crates.io y la versión publicada coincide con la del
repositorio. Las descargas son las de crates.io a la fecha del análisis.

| Crate | Versión | Descargas | Para qué sirve | Ejemplos ejecutables |
| --- | --- | ---: | --- | --- |
| `cpeforge` | 0.3.1 | 65 | Paquete principal con las fachadas `See` (SOAP) y `Api` (GRE). Incluye importe en letras y la feature `xcodes` | `fachadas_sin_red`, `importe_en_letras`, `see_with_xcodes` |
| `cpeforge-core` | 0.2.2 | 124 | Modelos (`Invoice`, `Note`, `Despatch`, `Summary`…), contratos (`ports`) y la feature `serde` | `invoice_model` |
| `cpeforge-xml` | 0.3.2 | 85 | XML UBL 2.0/2.1 sin firma: 7 builders y 13 plantillas | `invoice_xml`, `despatch_2022_xml` |
| `cpeforge-xmldsig` | 0.2.2 | 71 | Firma XMLDSig, C14N, X.509, `TrustPolicy` y XMLEnc | `sign_verify_invoice`, `bench` |
| `cpeforge-ws` | 0.3.1 | 80 | SOAP, ZIP, CDR, `TaxStatus` y caché de tokens GRE | `local_cdr` |
| `cpeforge-gre-api` | 0.2.1 | 62 | Cliente HTTP de bajo nivel de la API GRE (OAuth2) | `flujo_gre` |
| `cpeforge-validator` | 0.3.1 | 42 | Reglas de validación de los modelos | `invoice_validation` |
| `cpeforge-xcodes` | 0.4.0 | 52 | Catálogo de los 2 077 códigos de retorno de SUNAT (versión 2026-08-26) | `lookup`, `with_validator` |
| `cpeforge-xml-parser` | 0.3.1 | 42 | De un XML de comprobante a los modelos tipados | `parse_invoice`, `reader` |
| `cpeforge-xml-limits` | 0.1.3 | 112 | Límites de XML comprobados sobre el texto (apoyo, sin dependencias) | — |
| `cpeforge-cpe-validator` | 0.2.1 | 43 | Reglas XSL de SUNAT con libxslt (usa FFI) | `validate_rules` |
| `cpeforge-ubl-validator` | 0.2.0 | 34 | Validación XSD de UBL con libxml2 (sin soporte en Windows) | `validate_ubl` |
| `cpeforge-report` | 0.3.3 | 70 | HTML de 8 plantillas, QR SVG y plantillas propias en MiniJinja | `invoice_html` |

### 1.3 Material que ya existe y se reutiliza

| Fuente | Se usa en |
| --- | --- |
| `README.md` raíz (por qué Rust, tabla Rust vs PHP, factura en 5 minutos, qué cubre, estado y requisitos) | Landing, Introducción y Primeros pasos |
| `crates/*/README.md` (13) | Una página por crate en «Paquetes» |
| `docs/benchmark.md` | Página «Rendimiento» |
| `docs/README.md` (equivalencias con Greenter) | Página «Viniendo de Greenter» |
| `example/README.md` + `example/rust/src/casos/*.rs` (48 casos) + `errores.rs` (4 rechazos) | Sección «Ejemplos» |
| `example/credenciales/` (credenciales públicas de prueba) | Guía de entornos y credenciales |
| `crates/xcodes/src/data/CodeErrors.xml` (2 077 códigos) | Buscador interactivo de códigos SUNAT |
| `crates/report/templates/*.twig` | Guía de representación impresa |
| `AGENTS.md` (puerta de calidad) | Página «Contribuir» |

### 1.4 Hallazgos que condicionan el plan

1. **Los ejemplos deben ser código real.** Los 48 casos y los ejemplos de cada
   crate ya compilan y se prueban. El sitio debe **incluir esos archivos** en
   vez de copiarlos, para que nunca queden desactualizados.
2. **Las versiones cambian a menudo**: hay varios commits recientes de
   «actualizar versiones». Las versiones que muestre el sitio deben salir de
   una única fuente generada desde los `Cargo.toml`, no escribirse a mano.
3. **`docs.rs` ya publica la referencia de la API.** El sitio no la duplica:
   explica conceptos, guías y ejemplos, y enlaza a docs.rs para cada tipo.
4. **Twoslash y `fumadocs-typescript` no sirven aquí**: solo funcionan con
   TypeScript. Para Rust basta Shiki (resaltado) más los enlaces a docs.rs.
5. **Windows tiene limitaciones**: `cpeforge-ubl-validator` no compila en
   Windows y los validadores necesitan libxml2/libxslt. La página de requisitos
   debe separar Linux, macOS y Windows.
6. **Regla de `AGENTS.md`**: los crates no mencionan la referencia PHP. El
   sitio no es un crate, así que puede tener la página «Viniendo de Greenter»
   y el benchmark contra PHP, pero las páginas de los crates siguen esa regla.

---

## 2. Pila tecnológica (últimas versiones a 2026-10-21)

Consultado con `npm view` en la fecha del análisis. Al crear el proyecto se
vuelve a comprobar y se fija la versión exacta en `package.json`.

| Paquete | Versión | Uso |
| --- | --- | --- |
| `next` | 16.4.2 | Framework (App Router, export estático) |
| `react` / `react-dom` | 19.3.1 | UI |
| `fumadocs-core` | 16.17.7 | Árbol de páginas, búsqueda y TOC |
| `fumadocs-ui` | 16.17.7 | Layouts, sidebar y componentes |
| `fumadocs-mdx` | 15.4.9 | Contenido MDX tipado (`source.config.ts`) |
| `fumadocs-docgen` | 3.2.0 | Utilidades remark para MDX |
| `@orama/orama` | 3.1.18 | Búsqueda local estática |
| `tailwindcss` / `@tailwindcss/postcss` | 4.3.3 | Estilos |
| `shiki` | 4.5.0 | Resaltado de Rust, TOML, bash y XML |
| `mermaid` | 12.2.0 | Diagramas de flujo y de arquitectura |
| `lucide-react` | 1.61.0 | Iconos |
| `typescript` | 7.0.2 | Tipado. **Riesgo**: es el compilador nativo nuevo. Si Next 16.4 no lo admite, se fija la última 6.x |
| `@types/react`, `@types/node`, `@types/mdx` | 19.3.0 / 26.6.5 / 2.0.14 | Tipos |

- Gestor de paquetes: **pnpm 11.23** (está instalado). Node 24.12.
- Se arranca con `pnpm create fumadocs-app` (create-fumadocs-app 16.2.25),
  eligiendo Next.js y Fumadocs MDX. Después se suben todas las dependencias a
  la última versión con `pnpm up --latest`.
- Idioma: español. La estructura queda preparada para añadir inglés más
  adelante con i18n de Fumadocs, pero el inglés no forma parte de este plan.

---

## 3. Dónde vive el sitio

Propuesta: una carpeta `website/` dentro del repositorio `cpeforge`. Así los
`<include>` pueden leer `../example/rust/src/casos/*.rs` y
`../crates/*/examples/*.rs`, y un cambio de código y su documentación van en
el mismo PR.

```text
cpeforge/
├── crates/ …                     (sin cambios)
├── example/ …                    (sin cambios)
└── website/
    ├── package.json
    ├── pnpm-lock.yaml
    ├── next.config.mjs           (output: 'export', basePath para GitHub Pages)
    ├── source.config.ts          (colecciones MDX con esquema de frontmatter)
    ├── tsconfig.json
    ├── postcss.config.mjs
    ├── scripts/
    │   ├── sync-versions.mjs     (lee crates/*/Cargo.toml → generated/versions.json)
    │   ├── gen-xcodes.mjs        (CodeErrors.xml → generated/xcodes.json)
    │   └── gen-ejemplos.mjs      (casos/*.rs + example/README.md → generated/ejemplos.json)
    ├── generated/                (salida de los scripts; se regenera en cada build)
    ├── lib/
    │   ├── source.ts             (loader de Fumadocs)
    │   └── layout.shared.tsx     (nav, logo, enlaces a GitHub, crates.io y docs.rs)
    ├── app/
    │   ├── layout.tsx            (RootProvider, fuente, tema)
    │   ├── global.css            (Tailwind 4 + preset de fumadocs-ui + color de marca)
    │   ├── (home)/page.tsx       (landing)
    │   ├── docs/layout.tsx       (DocsLayout)
    │   ├── docs/[[...slug]]/page.tsx
    │   ├── api/search/route.ts   (índice Orama estático)
    │   └── og/[...slug]/route.tsx (imágenes Open Graph por página)
    ├── components/
    │   ├── crate-card.tsx        (nombre, versión, descripción y enlaces)
    │   ├── crate-badges.tsx      (crates.io · docs.rs · MSRV · licencia)
    │   ├── cargo-install.tsx     (pestañas: `cargo add` / `Cargo.toml`)
    │   ├── mermaid.tsx           (diagramas en cliente con tema claro/oscuro)
    │   ├── benchmark-table.tsx   (Rust vs PHP con barras)
    │   ├── ejemplos-explorer.tsx (filtro de los 48 casos por tipo)
    │   ├── xcodes-search.tsx     (buscador de los 2 077 códigos SUNAT)
    │   └── lifecycle.tsx         (pasos del ciclo de vida de un CPE)
    ├── public/                   (logo, favicon, imágenes)
    └── content/docs/ …           (ver sección 5)
```

---

## 4. La página de inicio (landing)

En orden vertical:

1. **Hero**: «Facturación electrónica SUNAT en Rust: rápida, segura y sin
   sorpresas.» Botones **Empezar** (→ primera factura) y **GitHub**. Debajo, el
   bloque `cargo add cpeforge cpeforge-core …` con botón de copiar.
2. **Cifras**: 13 crates · 48 ejemplos aceptados por SUNAT beta · 370+ pruebas
   · 2 077 códigos de error · XML 64× más rápido que PHP.
3. **¿Por qué Rust?**: 6 tarjetas, tomadas del README: el dinero nunca es
   `float`, errores en el tipo, rendimiento predecible, seguro por defecto,
   concurrencia sin miedo y un binario y listo.
4. **Cómo funciona**: diagrama del flujo Modelo → XML UBL → Firma → ZIP →
   SUNAT → CDR → `TaxStatus` → PDF/HTML.
5. **Código de ejemplo**: la factura de 5 minutos, con pestañas Cargo.toml /
   main.rs.
6. **Rust frente a PHP**: tabla resumida del benchmark y enlace a la página
   completa.
7. **Qué cubre**: rejilla de tipos de comprobante (factura, boleta, notas,
   resumen, baja, reversión, retención, percepción, liquidación y guías).
8. **Paquetes**: rejilla de las 13 `CrateCard` con la versión leída de
   `versions.json`.
9. **Pie**: licencia MIT, «no afiliado a SUNAT», enlaces.

---

## 5. Contenido de la documentación (`content/docs`)

Cada carpeta lleva un `meta.json` con el orden y el título del grupo en el
sidebar. Las páginas marcadas con ★ son las prioritarias del MVP.

```text
content/docs/
├── index.mdx                                ★ Bienvenida y mapa del sitio
│
├── introduccion/
│   ├── que-es-cpeforge.mdx                  ★ Qué es, qué problema resuelve y para quién
│   ├── por-que-rust.mdx                     ★ Decimal, Zoned, Result, sin GC, seguridad y un solo binario
│   ├── arquitectura.mdx                     ★ Diagrama de dependencias de los 13 crates
│   ├── que-cubre.mdx                        Comprobantes, envío, firma, validación, lectura e impresión
│   ├── estado-y-semver.mdx                  non_exhaustive, ..Default::default() y API estable
│   └── rendimiento.mdx                      docs/benchmark.md con tablas y gráficos
│
├── primeros-pasos/
│   ├── requisitos.mdx                       ★ Rust, OpenSSL 3 y libxml2/libxslt; pestañas Linux/macOS/Windows
│   ├── instalacion.mdx                      ★ Qué crates añadir según el caso de uso
│   ├── primera-factura.mdx                  ★ Paso a paso (Steps) de la factura de 5 minutos
│   ├── entornos-y-credenciales.mdx          ★ BETA vs producción, Clave SOL, credenciales GRE
│   ├── certificado-digital.mdx              PEM y PKCS#12, conversión con openssl, buenas prácticas
│   └── ejecutar-los-ejemplos.mdx            ★ cargo run del proyecto example/ y sus flags
│
├── conceptos/
│   ├── ciclo-de-vida.mdx                    ★ Construir → XML → firmar → ZIP → enviar → CDR → estado
│   ├── modelos.mdx                          BaseSale, Invoice, SaleDetail, Option<T> y Default
│   ├── dinero-y-fechas.mdx                  Decimal exacto, Zoned y la zona America/Lima
│   ├── errores.mdx                          Errores tipados por capa y cómo hacerles match
│   ├── estado-tributario.mdx                ★ TaxStatus: aceptado, observado, rechazado, pendiente…
│   ├── importe-en-letras.mdx                Leyenda 1000 y amount_words
│   ├── seguridad.mdx                        XML sin DTD, límites, TLS, ZIP, zeroize y unsafe acotado
│   ├── concurrencia-y-volumen.mdx           Una fachada por tarea, spawn_blocking y sin reintentos
│   └── serde-y-json.mdx                     Feature serde, camelCase e importes como texto
│
├── guias/                                   (una guía por tipo de comprobante)
│   ├── factura/
│   │   ├── index.mdx                        ★ Factura gravada al contado
│   │   ├── credito.mdx                      Forma de pago a crédito y cuotas
│   │   ├── exonerada-inafecta.mdx
│   │   ├── exportacion.mdx
│   │   ├── gratuita.mdx
│   │   ├── detraccion.mdx
│   │   ├── percepcion-retencion.mdx         Factura con percepción o retención
│   │   ├── anticipos.mdx
│   │   ├── descuentos-y-cargos.mdx          Por línea, global y sin base
│   │   ├── impuestos-especiales.mdx         ICBPER, ISC e IVAP
│   │   └── contingencia.mdx
│   ├── boleta.mdx                           ★
│   ├── notas-credito-debito.mdx             ★ Motivos y el caso del motivo 13
│   ├── resumen-diario.mdx                   sendSummary, ticket y getStatus
│   ├── comunicacion-baja.mdx
│   ├── reversion.mdx
│   ├── retencion.mdx
│   ├── percepcion.mdx
│   ├── liquidacion-compra.mdx               Compra interna (0501) y compra de oro (0503)
│   ├── guia-remitente.mdx                   ★ GRE 2022 tipo 09 por la API REST
│   ├── guia-transportista.mdx               GRE tipo 31
│   ├── representacion-impresa.mdx           ★ HtmlReport, QR, plantillas propias y PDF con Gotenberg
│   ├── leer-xml.mdx                         xml-parser: de un XML a los modelos
│   ├── validacion.mdx                       Validador de modelos, XSD y XSL
│   ├── codigos-de-error.mdx                 xcodes y la feature de cpeforge
│   └── produccion.mdx                       Checklist, Docker, secretos, TrustPolicy y monitoreo
│
├── paquetes/                                (una página por crate, con la misma plantilla)
│   ├── index.mdx                            ★ Tabla de los 13 y «¿qué crate necesito?»
│   ├── cpeforge.mdx                         ★
│   ├── core.mdx                             ★
│   ├── xml.mdx
│   ├── xmldsig.mdx
│   ├── ws.mdx
│   ├── gre-api.mdx
│   ├── validator.mdx
│   ├── xcodes.mdx
│   ├── xml-parser.mdx
│   ├── xml-limits.mdx
│   ├── cpe-validator.mdx
│   ├── ubl-validator.mdx
│   └── report.mdx
│
├── ejemplos/
│   ├── index.mdx                            ★ Explorador filtrable de los 48 casos + 4 rechazos
│   ├── facturas.mdx                         25 casos: código incluido y explicación de cada uno
│   ├── boletas.mdx
│   ├── notas.mdx
│   ├── resumenes-bajas-reversiones.mdx
│   ├── retencion-percepcion.mdx
│   ├── liquidaciones.mdx
│   ├── guias.mdx
│   ├── casos-de-rechazo.mdx                 3617, 3315, 2992 y 2800, y cómo leerlos
│   └── ejemplos-por-crate.mdx               Los `crates/*/examples/*.rs`
│
├── referencia/
│   ├── codigos-sunat.mdx                    ★ Buscador interactivo de los 2 077 códigos
│   ├── catalogos-sunat.mdx                  Catálogos 01, 05, 06, 07, 51, 52… y dónde se usan
│   ├── endpoints.mdx                        SunatEndpoints (FE_BETA, RETENCION_BETA…)
│   ├── glosario.mdx                         CPE, CDR, UBL, GRE, OSE, Clave SOL, RUC, ubigeo…
│   └── docs-rs.mdx                          Enlaces a la referencia de la API de cada crate
│
├── migracion/
│   └── desde-greenter.mdx                   Equivalencias de paquetes e identificadores
│
└── comunidad/
    ├── contribuir.mdx                       Puerta de calidad (fmt, clippy, test, doc, deny, audit)
    ├── faq.mdx
    ├── changelog.mdx                        Versiones por crate (desde git tags o CHANGELOG)
    └── licencia.mdx                         MIT, NOTICE y «no afiliado a SUNAT»
```

### 5.1 Plantilla común de cada página de crate

1. Encabezado: nombre, `CrateBadges` (versión, docs.rs, MSRV, licencia) e
   instalación con `CargoInstall`.
2. **Para qué sirve**, en un párrafo.
3. **Cuándo usarlo y cuándo no** (por ejemplo: «usa `cpeforge` salvo que solo
   necesites firmar»).
4. **Conceptos y tipos principales**, con enlaces a docs.rs.
5. **Features** de Cargo (`serde`, `reqwest`, `xcodes`…).
6. **Ejemplo** incluido desde `crates/<crate>/examples/*.rs`, con el comando
   `cargo run -p … --example …`.
7. **Seguridad** (los límites y el perfil seguro de su README).
8. **Rendimiento**: la tabla de su README.
9. **Requisitos del sistema**, si los tiene (OpenSSL, libxml2…).
10. Relacionado: crates de los que depende y crates que dependen de él.

### 5.2 Plantilla común de cada guía de comprobante

1. Qué es el comprobante y cuándo se emite (catálogo 01 y normativa).
2. Por qué vía va (`sendBill`, `sendSummary` + ticket o API GRE).
3. Campos obligatorios y cómo cuadran los importes (tabla).
4. Código completo incluido desde `example/rust/src/casos/<caso>.rs`.
5. Cómo enviarlo y leer la respuesta (`SendResult` y `TaxStatus`).
6. Errores frecuentes de SUNAT para ese tipo, enlazados al buscador de
   códigos.
7. Representación impresa de ese tipo (plantilla de `cpeforge-report`).

### 5.3 Componentes MDX que se usan

`Callout`, `Steps`, `Tabs`, `Cards`, `Accordion`, `Files` (árbol de archivos)
y `TypeTable` de fumadocs-ui. Además, los componentes propios de la sección 3
y bloques de código con título (`title="src/main.rs"`), resaltado de líneas y
botón de copiar.

---

## 6. Cómo se mantienen sincronizados código y documentación

| Problema | Solución |
| --- | --- |
| Versiones desactualizadas en el texto | `scripts/sync-versions.mjs` genera `versions.json` desde los `Cargo.toml` antes de cada build. MDX y componentes leen `<Version crate="cpeforge" />` |
| Ejemplos copiados que dejan de compilar | Incluir los `.rs` reales con `<include>` de fumadocs-mdx. Si no admite código fuera de `content/`, se usa un plugin remark propio de ~30 líneas que lea el archivo y lo convierta en un bloque de código |
| Fragmentos cortos escritos en MDX | Job de CI opcional que extrae los bloques ```` ```rust ```` y los compila como doctests en un crate temporal |
| Catálogo de códigos SUNAT | `scripts/gen-xcodes.mjs` convierte `CodeErrors.xml` a JSON en el build |
| Lista de ejemplos | `scripts/gen-ejemplos.mjs` lee `casos/mod.rs`, la primera línea `//!` de cada caso y las tablas de `example/README.md` |
| Enlaces rotos | Comprobación de enlaces internos en el build y de los enlaces a docs.rs en CI |

---

## 7. Despliegue

- **Export estático** (`output: 'export'`): HTML puro, sin servidor.
- **Opción A (recomendada): GitHub Pages** con un workflow
  `.github/workflows/docs.yml`. Se dispara con cambios en `website/`,
  `crates/*/README.md`, `crates/*/examples/` o `example/`. Pasos: pnpm install
  → scripts de generación → `next build` → publicar en Pages. URL:
  `alvarez25leo.github.io/cpeforge` (o un dominio propio).
- **Opción B: Vercel**, con previsualización por PR y sin configurar nada.
- SEO: `metadata` por página, sitemap, `robots.txt`, imágenes Open Graph
  generadas y `llms.txt` / `llms-full.txt` (Fumadocs los genera) para que los
  asistentes de IA lean la documentación.
- En `Cargo.toml` (campo `homepage`) y en los README se añade el enlace al
  sitio. Eso se hace en una fase aparte, porque cambia los crates.

---

## 8. Fases de trabajo

| Fase | Entregable | Criterio de «hecho» |
| --- | --- | --- |
| **F0. Andamiaje** | `website/` con Fumadocs y Next 16, todo en su última versión, tema de marca, logo y layout | `pnpm dev` y `pnpm build` funcionan sin avisos y se ve la página por defecto |
| **F1. Infraestructura** | Scripts de generación, `<include>` de código, componentes propios y Mermaid | Una página de prueba muestra la versión, un ejemplo incluido y un diagrama |
| **F2. Landing** | Página de inicio completa (sección 4) | Se ve bien en móvil y escritorio, en tema claro y oscuro |
| **F3. Introducción y primeros pasos** | Las páginas ★ de esas dos secciones | Alguien nuevo puede emitir la factura en beta siguiendo solo el sitio |
| **F4. Conceptos y paquetes** | Las 9 páginas de conceptos y los 13 crates con su plantilla | Cada crate tiene su ejemplo incluido y enlaces a docs.rs que funcionan |
| **F5. Guías y ejemplos** | Las guías por comprobante y el explorador de los 48 casos | Cada caso del README de ejemplos tiene página o sección |
| **F6. Referencia y comunidad** | Buscador de códigos, glosario, migración, FAQ, contribuir y changelog | El buscador encuentra cualquiera de los 2 077 códigos |
| **F7. Calidad y despliegue** | Workflow de CI/Pages, SEO, `llms.txt` y comprobación de enlaces | El sitio está publicado y el build falla si un enlace o un include se rompe |

Las fases F0 a F3 forman un **MVP publicable**. Las fases F4 a F6 se pueden
publicar de forma incremental.

---

## 9. Decisiones pendientes

1. **Hosting**: GitHub Pages (recomendado) o Vercel.
2. **Ubicación**: `website/` dentro del repo `cpeforge` (recomendado) o un
   repositorio aparte.
3. **Dominio**: subruta de GitHub Pages o dominio propio (por ejemplo
   `cpeforge.dev`).
4. **Logo e identidad**: si ya existe un logo y un color de marca, o se
   propone uno.
5. **Idioma**: solo español ahora (recomendado) o español e inglés desde el
   inicio.
6. **Página «Viniendo de Greenter»**: incluirla (recomendado, ayuda a quien
   migra) u omitirla.

import Link from 'next/link';
import type { ReactNode } from 'react';
import { DynamicCodeBlock } from 'fumadocs-ui/components/dynamic-codeblock';
import {
  BadgeCheck,
  Banknote,
  Boxes,
  Cpu,
  FileCode2,
  Gauge,
  Package,
  ShieldCheck,
  Workflow,
} from 'lucide-react';
import { BenchmarkTable } from '@/components/benchmark';
import { CrateGrid } from '@/components/crates';
import { Mermaid } from '@/components/mermaid';
import { cargoRequirement, crates, exampleCases, getCrate } from '@/lib/crates';
import { repoUrl } from '@/lib/shared';

const install = `cargo add cpeforge cpeforge-core cpeforge-gre-api cpeforge-ws
cargo add tokio --features macros,rt-multi-thread`;

const quickstart = `let mut see = See::new(ReqwestTransport::new()?);
see.set_certificate(&std::fs::read_to_string("certificado.pem")?)?;
see.set_service(SunatEndpoints::FE_BETA);
see.set_clave_sol("20000000001", "MODDATOS", "moddatos");

match see.send(DocumentRef::Invoice(&invoice)).await? {
    SendResult::Bill(bill) => println!("SUNAT: {:?}", TaxStatus::of_bill(&bill)),
    other => println!("respuesta: {other:?}"),
}`;

const flow = `flowchart LR
  A[Modelo tipado] --> B[XML UBL 2.1]
  B --> C[Firma XMLDSig]
  C --> D[ZIP]
  D --> E{{SUNAT}}
  E --> F[CDR o ticket]
  F --> G[TaxStatus]
  A -.-> H[HTML + QR]`;

const reasons: { icon: ReactNode; title: string; body: string }[] = [
  {
    icon: <Banknote />,
    title: 'El dinero nunca es float',
    body: 'Importes con Decimal exacto y fechas Zoned en hora de Lima. Sin redondeos silenciosos ni comprobantes con la fecha de otro día.',
  },
  {
    icon: <BadgeCheck />,
    title: 'Errores que se ven en el tipo',
    body: 'Cada operación devuelve un Result que dice qué falló: XML, firma, transporte o SUNAT. TaxStatus clasifica la respuesta por ti.',
  },
  {
    icon: <Gauge />,
    title: 'Rendimiento predecible',
    body: 'Código nativo, sin intérprete ni recolector de basura. Una factura de 1000 líneas se construye, firma y envía en unos 36 ms.',
  },
  {
    icon: <ShieldCheck />,
    title: 'Seguro por defecto',
    body: 'XML sin DTD y con límites, TLS siempre verificado, ZIP validado antes de abrirlo y secretos borrados de memoria al soltarse.',
  },
  {
    icon: <Workflow />,
    title: 'Concurrencia sin miedo',
    body: 'Tipos Send + Sync y red async sobre Tokio. La caché de tokens separa empresas y ambientes en un servicio multiempresa.',
  },
  {
    icon: <Package />,
    title: 'Un binario y listo',
    body: 'Sin runtime que instalar en el servidor: compilas y despliegas un ejecutable o un contenedor pequeño.',
  },
];

const coverage = [
  ['Factura', '01'],
  ['Boleta', '03'],
  ['Nota de crédito', '07'],
  ['Nota de débito', '08'],
  ['Liquidación de compra', '04'],
  ['Guía remitente', '09'],
  ['Guía transportista', '31'],
  ['Retención', '20'],
  ['Percepción', '40'],
  ['Resumen diario', 'RC'],
  ['Comunicación de baja', 'RA'],
  ['Reversión', 'RR'],
];

function Section({ title, kicker, children }: { title: string; kicker?: string; children: ReactNode }) {
  return (
    <section className="mx-auto w-full max-w-6xl px-4 py-14 md:px-6">
      {kicker && (
        <p className="mb-2 text-sm font-medium uppercase tracking-wider text-fd-primary">{kicker}</p>
      )}
      <h2 className="mb-8 text-2xl font-semibold tracking-tight md:text-3xl">{title}</h2>
      {children}
    </section>
  );
}

export default function HomePage() {
  const stats = [
    { value: String(crates.length), label: 'crates publicados' },
    { value: String(exampleCases), label: 'ejemplos aceptados por SUNAT beta' },
    { value: '370+', label: 'pruebas automáticas' },
    { value: '2 077', label: 'códigos de error de SUNAT' },
  ];
  const cpeforge = getCrate('cpeforge');

  return (
    <main className="flex flex-1 flex-col">
      {/* Hero */}
      <section className="relative overflow-hidden border-b">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(60%_50%_at_50%_0%,color-mix(in_oklab,var(--color-fd-primary)_18%,transparent),transparent)]"
        />
        <div className="relative mx-auto flex w-full max-w-6xl flex-col items-center px-4 py-20 text-center md:px-6 md:py-28">
          <a
            href={`https://crates.io/crates/cpeforge`}
            className="mb-6 inline-flex items-center gap-2 rounded-full border bg-fd-card px-3 py-1 text-xs text-fd-muted-foreground"
          >
            <span className="size-1.5 rounded-full bg-fd-primary" />
            cpeforge {cpeforge.version} · API estable · MIT
          </a>
          <h1 className="max-w-3xl text-4xl font-bold tracking-tight md:text-6xl">
            Facturación electrónica SUNAT en <span className="text-fd-primary">Rust</span>
          </h1>
          <p className="mt-6 max-w-2xl text-lg text-fd-muted-foreground">
            CPEForge construye el XML UBL, lo firma, lo envía a SUNAT, lee el CDR y genera la
            representación impresa. Rápido, seguro y sin sorpresas, para que tu sistema de facturación
            sea tan confiable como el dinero que mueve.
          </p>
          <div className="mt-8 flex flex-wrap justify-center gap-3">
            <Link
              href="/docs/primeros-pasos/primera-factura"
              className="rounded-lg bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
            >
              Emite tu primera factura
            </Link>
            <Link
              href="/docs"
              className="rounded-lg border bg-fd-card px-5 py-2.5 text-sm font-medium transition-colors hover:bg-fd-accent"
            >
              Leer la documentación
            </Link>
            <a
              href={repoUrl}
              className="rounded-lg border bg-fd-card px-5 py-2.5 text-sm font-medium transition-colors hover:bg-fd-accent"
            >
              GitHub
            </a>
          </div>
          <div className="mt-10 w-full max-w-2xl text-left">
            <DynamicCodeBlock lang="bash" code={install} />
          </div>
        </div>
      </section>

      {/* Cifras */}
      <section className="border-b bg-fd-card/50">
        <dl className="mx-auto grid w-full max-w-6xl grid-cols-2 gap-6 px-4 py-10 md:grid-cols-4 md:px-6">
          {stats.map((stat) => (
            <div key={stat.label} className="text-center">
              <dt className="sr-only">{stat.label}</dt>
              <dd className="text-3xl font-bold tabular-nums text-fd-primary">{stat.value}</dd>
              <dd className="mt-1 text-sm text-fd-muted-foreground">{stat.label}</dd>
            </div>
          ))}
        </dl>
      </section>

      <Section kicker="Por qué Rust" title="Facturar no es un CRUD más">
        <p className="-mt-4 mb-8 max-w-3xl text-fd-muted-foreground">
          Cada comprobante tiene importes que deben cuadrar al céntimo, fechas que dependen de la zona
          horaria de Lima, una firma que no admite ni un byte de diferencia y un servidor de SUNAT que
          responde cuando puede. Rust convierte esos riesgos en errores que ves antes de que lleguen a tu
          cliente.
        </p>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {reasons.map((reason) => (
            <div key={reason.title} className="rounded-xl border bg-fd-card p-5">
              <div className="mb-3 inline-flex size-9 items-center justify-center rounded-lg bg-fd-primary/10 text-fd-primary [&_svg]:size-5">
                {reason.icon}
              </div>
              <h3 className="mb-1 font-semibold">{reason.title}</h3>
              <p className="text-sm text-fd-muted-foreground">{reason.body}</p>
            </div>
          ))}
        </div>
        <Link href="/docs/introduccion/por-que-rust" className="mt-6 inline-block text-sm font-medium text-fd-primary">
          Más sobre por qué Rust →
        </Link>
      </Section>

      <Section kicker="Cómo funciona" title="Del modelo al CDR, en un solo SDK">
        <div className="grid items-center gap-8 lg:grid-cols-2">
          <div className="min-w-0 rounded-xl border bg-fd-card p-4">
            <Mermaid chart={flow} />
          </div>
          <div className="min-w-0">
            <p className="mb-4 text-fd-muted-foreground">
              Describes el comprobante con modelos tipados. La fachada <code>See</code> genera el XML UBL,
              lo firma con tu certificado, lo comprime y lo envía a SUNAT. La respuesta llega clasificada
              en un <code>TaxStatus</code>: aceptado, con observaciones, rechazado o pendiente.
            </p>
            <DynamicCodeBlock lang="rust" code={quickstart} codeblock={{ title: 'src/main.rs' }} />
          </div>
        </div>
      </Section>

      <Section kicker="Rendimiento" title="Rust frente a PHP, con los mismos 44 comprobantes">
        <p className="-mt-4 mb-6 max-w-3xl text-fd-muted-foreground">
          El mismo comprobante y la misma respuesta de SUNAT (43 de 43 aceptados en ambos), en la mitad
          del tiempo. Generar el XML es 64 veces más rápido y la primera llamada en frío, 3,6 veces.
        </p>
        <BenchmarkTable />
        <Link href="/docs/introduccion/rendimiento" className="mt-6 inline-block text-sm font-medium text-fd-primary">
          Ver el benchmark completo →
        </Link>
      </Section>

      <Section kicker="Qué cubre" title="Todos los comprobantes electrónicos de SUNAT">
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {coverage.map(([name, code]) => (
            <div key={name} className="flex items-center justify-between gap-2 rounded-lg border bg-fd-card px-4 py-3">
              <span className="text-sm font-medium">{name}</span>
              <span className="rounded bg-fd-muted px-1.5 py-0.5 font-mono text-xs text-fd-muted-foreground">
                {code}
              </span>
            </div>
          ))}
        </div>
        <div className="mt-6 grid gap-4 text-sm text-fd-muted-foreground md:grid-cols-3">
          <p className="flex gap-2">
            <FileCode2 className="mt-0.5 size-4 shrink-0 text-fd-primary" />
            UBL 2.0 y 2.1, firma XMLDSig, envío SOAP y API REST de guías con OAuth2.
          </p>
          <p className="flex gap-2">
            <Cpu className="mt-0.5 size-4 shrink-0 text-fd-primary" />
            Validación de modelos, esquemas XSD y reglas XSL de SUNAT.
          </p>
          <p className="flex gap-2">
            <Boxes className="mt-0.5 size-4 shrink-0 text-fd-primary" />
            Lectura de XML a modelos y representación impresa HTML con QR.
          </p>
        </div>
      </Section>

      <Section kicker="Paquetes" title="Usa la fachada completa o solo la pieza que necesitas">
        <CrateGrid />
        <p className="mt-6 text-sm text-fd-muted-foreground">
          ¿Empiezas? Con <code>cpeforge = &quot;{cargoRequirement(cpeforge.version)}&quot;</code> tienes las
          fachadas <code>See</code> y <code>Api</code>.{' '}
          <Link href="/docs/primeros-pasos/instalacion" className="font-medium text-fd-primary">
            ¿Qué crate necesito? →
          </Link>
        </p>
      </Section>

      <section className="border-t">
        <div className="mx-auto flex w-full max-w-6xl flex-col items-center gap-4 px-4 py-16 text-center md:px-6">
          <h2 className="text-2xl font-semibold tracking-tight md:text-3xl">Empieza en cinco minutos</h2>
          <p className="max-w-xl text-fd-muted-foreground">
            Emite una factura gravada con IGV en el entorno beta de SUNAT, con credenciales públicas de
            prueba.
          </p>
          <Link
            href="/docs/primeros-pasos/primera-factura"
            className="rounded-lg bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
          >
            Emite tu primera factura
          </Link>
        </div>
      </section>

      <footer className="border-t py-8 text-center text-xs text-fd-muted-foreground">
        CPEForge es software libre con licencia MIT. No está afiliado a SUNAT.
      </footer>
    </main>
  );
}

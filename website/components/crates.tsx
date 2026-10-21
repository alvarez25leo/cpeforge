import { DynamicCodeBlock } from 'fumadocs-ui/components/dynamic-codeblock';
import {
  cargoRequirement,
  cratesIo,
  docsRs,
  getCrate,
  orderedCrates,
  type CrateInfo,
} from '@/lib/crates';

/** Versión publicada de un crate, leída de su Cargo.toml en el build. */
export function Version({ crate }: { crate: string }) {
  return <code>{getCrate(crate).version}</code>;
}

/** Insignias de un crate: versión, docs.rs y MSRV. */
export function CrateBadges({ crate }: { crate: string }) {
  const info = getCrate(crate);
  const badge =
    'inline-flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-xs font-medium no-underline';
  return (
    <div className="not-prose mb-4 flex flex-wrap gap-2">
      <a className={`${badge} text-fd-primary`} href={cratesIo(info.name)}>
        crates.io v{info.version}
      </a>
      <a className={badge} href={docsRs(info.name)}>
        docs.rs
      </a>
      <span className={badge}>Rust {info.msrv}+</span>
      <span className={badge}>MIT</span>
    </div>
  );
}

/**
 * Bloque `[dependencies]` con las versiones actuales de los crates indicados.
 * `features` activa features por crate y `extra` añade líneas literales (por
 * ejemplo `tokio`).
 */
export function CargoDeps({
  crates,
  features = {},
  extra = [],
}: {
  crates: string[];
  features?: Record<string, string[]>;
  extra?: string[];
}) {
  const lines = crates.map((name) => {
    const version = cargoRequirement(getCrate(name).version);
    const enabled = features[name];
    if (!enabled?.length) return `${name} = "${version}"`;
    const list = enabled.map((f) => `"${f}"`).join(', ');
    return `${name} = { version = "${version}", features = [${list}] }`;
  });
  const code = ['[dependencies]', ...lines, ...extra].join('\n');
  return <DynamicCodeBlock lang="toml" code={code} codeblock={{ title: 'Cargo.toml' }} />;
}

/** Comando `cargo add` con los crates indicados. */
export function CargoAdd({ crates }: { crates: string[] }) {
  return <DynamicCodeBlock lang="bash" code={`cargo add ${crates.join(' ')}`} />;
}

function CrateCard({ crate }: { crate: CrateInfo }) {
  return (
    <div className="flex flex-col gap-2 rounded-xl border bg-fd-card p-4 text-fd-card-foreground transition-colors hover:bg-fd-accent/40">
      <div className="flex items-baseline justify-between gap-2">
        <a href={cratesIo(crate.name)} className="font-mono text-sm font-semibold no-underline">
          {crate.name}
        </a>
        <span className="shrink-0 rounded-full bg-fd-primary/10 px-2 py-0.5 font-mono text-xs text-fd-primary">
          {crate.version}
        </span>
      </div>
      <p className="m-0 flex-1 text-sm text-fd-muted-foreground">{crate.description}</p>
      <div className="flex gap-3 text-xs">
        <a href={docsRs(crate.name)} className="text-fd-muted-foreground underline-offset-4 hover:underline">
          docs.rs
        </a>
        <a href={cratesIo(crate.name)} className="text-fd-muted-foreground underline-offset-4 hover:underline">
          crates.io
        </a>
      </div>
    </div>
  );
}

/** Rejilla con los 13 crates publicados. */
export function CrateGrid() {
  return (
    <div className="not-prose grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {orderedCrates.map((crate) => (
        <CrateCard key={crate.name} crate={crate} />
      ))}
    </div>
  );
}

/** Tabla de los crates con versión, MSRV, features y ejemplos. */
export function CrateTable() {
  return (
    <table>
      <thead>
        <tr>
          <th>Crate</th>
          <th>Versión</th>
          <th>MSRV</th>
          <th>Contenido</th>
        </tr>
      </thead>
      <tbody>
        {orderedCrates.map((crate) => (
          <tr key={crate.name}>
            <td>
              <a href={docsRs(crate.name)}>
                <code>{crate.name}</code>
              </a>
            </td>
            <td>
              <code>{crate.version}</code>
            </td>
            <td>{crate.msrv}</td>
            <td>{crate.description}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

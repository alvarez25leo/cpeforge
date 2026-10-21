/**
 * Datos de docs/benchmark.md (corrida del 2026-10-20, Apple M5, mediana de 30
 * llamadas por caso, suma de los 44 casos comparables). Al publicar una corrida
 * nueva, actualizar estas cifras junto con docs/benchmark.md.
 */
export const benchmark = {
  date: '2026-10-20',
  environment: 'Apple M5 · PHP 8.4.17 CLI · Rust 1.99.1 (release)',
  rows: [
    { phase: 'Construir el modelo', php: '73 µs', rust: '28 µs', factor: 2.6 },
    { phase: 'Generar el XML UBL', php: '3,0 ms', rust: '47 µs', factor: 63.6 },
    { phase: 'Firmar (XMLDSig)', php: '26,8 ms', rust: '15,7 ms', factor: 1.7 },
    { phase: 'XML firmado de principio a fin', php: '33,1 ms', rust: '16,0 ms', factor: 2.1 },
    { phase: 'Primera llamada, en frío', php: '103,4 ms', rust: '28,9 ms', factor: 3.6 },
  ],
};

const maxLog = Math.log10(Math.max(...benchmark.rows.map((r) => r.factor)));

/** Tabla Rust vs PHP con una barra proporcional (escala logarítmica) al factor. */
export function BenchmarkTable() {
  return (
    <div className="not-prose overflow-x-auto rounded-xl border bg-fd-card">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b text-left text-fd-muted-foreground">
            <th className="px-4 py-2 font-medium">Fase (44 comprobantes)</th>
            <th className="px-4 py-2 text-right font-medium">PHP 8.4</th>
            <th className="px-4 py-2 text-right font-medium">Rust</th>
            <th className="min-w-40 px-4 py-2 font-medium">Rust es</th>
          </tr>
        </thead>
        <tbody>
          {benchmark.rows.map((row) => (
            <tr key={row.phase} className="border-b last:border-0">
              <td className="px-4 py-2">{row.phase}</td>
              <td className="px-4 py-2 text-right font-mono tabular-nums text-fd-muted-foreground">
                {row.php}
              </td>
              <td className="px-4 py-2 text-right font-mono tabular-nums">{row.rust}</td>
              <td className="px-4 py-2">
                <div className="flex items-center gap-2">
                  <div className="h-2 flex-1 rounded-full bg-fd-muted">
                    <div
                      className="h-2 rounded-full bg-fd-primary"
                      style={{ width: `${Math.max(8, (Math.log10(row.factor) / maxLog) * 100)}%` }}
                    />
                  </div>
                  <span className="w-14 text-right font-mono font-semibold tabular-nums">
                    {row.factor.toLocaleString('es-PE')}×
                  </span>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="border-t px-4 py-2 text-xs text-fd-muted-foreground">
        {benchmark.environment}. Corrida del {benchmark.date}. Las barras usan escala logarítmica.
      </p>
    </div>
  );
}

import type { SVGProps } from 'react';

/** Marca de CPEForge: un comprobante con la chispa de la forja. */
export function Logo(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 32 32" fill="none" aria-hidden="true" {...props}>
      <rect width="32" height="32" rx="8" className="fill-fd-primary" />
      <path
        d="M10 7h8.5L23 11.5V24a1 1 0 0 1-1 1H10a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1Z"
        className="fill-fd-primary-foreground"
        opacity="0.95"
      />
      <path d="M18.5 7v4.5H23" className="stroke-fd-primary" strokeWidth="1.4" />
      <path d="m16.8 13-3.6 5.2h2.9l-1 4.3 3.7-5.4h-2.9l.9-4.1Z" className="fill-fd-primary" />
    </svg>
  );
}

'use client';
import { useEffect, useRef, useState, type ReactNode } from 'react';

/**
 * Envuelve una zona con un resplandor difuso que sigue al cursor.
 *
 * Solo se activa con un puntero fino (ratón o trackpad) y cuando el navegador
 * está libre; con «movimiento reducido» no se muestra. La posición se aplica
 * directamente al estilo en cada frame, sin volver a renderizar React.
 */
export function CursorGlow({ children, className = '' }: { children: ReactNode; className?: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const glowRef = useRef<HTMLDivElement>(null);
  const [enabled, setEnabled] = useState(false);

  // Activar tras la carga, sin competir con el primer render.
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    if (!window.matchMedia('(pointer: fine)').matches) return;
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;

    const enable = () => setEnabled(true);
    if (typeof window.requestIdleCallback === 'function') {
      const id = window.requestIdleCallback(enable, { timeout: 2000 });
      return () => window.cancelIdleCallback(id);
    }
    const id = window.setTimeout(enable, 1000);
    return () => window.clearTimeout(id);
  }, []);

  useEffect(() => {
    if (!enabled) return;
    let frame: number | null = null;

    const onMove = (event: PointerEvent) => {
      if (frame !== null) cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        frame = null;
        const container = containerRef.current;
        const glow = glowRef.current;
        if (!container || !glow) return;
        const rect = container.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        glow.style.transform = `translate3d(calc(${x}px - 50%), calc(${y}px - 50%), 0)`;
        glow.dataset.visible = 'true';
      });
    };
    const onLeave = () => {
      if (glowRef.current) glowRef.current.dataset.visible = 'false';
    };

    window.addEventListener('pointermove', onMove, { passive: true });
    document.documentElement.addEventListener('pointerleave', onLeave);
    return () => {
      window.removeEventListener('pointermove', onMove);
      document.documentElement.removeEventListener('pointerleave', onLeave);
      if (frame !== null) cancelAnimationFrame(frame);
    };
  }, [enabled]);

  return (
    <div ref={containerRef} className={`relative isolate ${className}`}>
      {enabled && (
        <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-0 overflow-hidden">
          <div ref={glowRef} className="cursor-glow" data-visible="false" />
        </div>
      )}
      <div className="relative z-10 flex flex-1 flex-col">{children}</div>
    </div>
  );
}

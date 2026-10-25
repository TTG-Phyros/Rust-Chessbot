import type { ReactNode } from 'react';

interface PlaceholderProps {
  title: string;
  phase: number;
  children?: ReactNode;
}

/** Card announcing a feature that a later phase of the roadmap delivers. */
export function Placeholder({ title, phase, children }: PlaceholderProps) {
  return (
    <section className="rounded-xl border border-slate-800 bg-slate-900 p-6">
      <h1 className="text-xl font-semibold">{title}</h1>
      <p className="mt-1 text-sm text-slate-400">Disponible en phase {phase}.</p>
      {children && <div className="mt-6">{children}</div>}
    </section>
  );
}

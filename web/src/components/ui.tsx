import type { ButtonHTMLAttributes, ReactNode } from 'react';

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'primary' | 'secondary' | 'ghost';
};

const VARIANTS: Record<NonNullable<ButtonProps['variant']>, string> = {
  primary: 'bg-emerald-600 text-white hover:bg-emerald-500',
  secondary: 'bg-slate-800 text-slate-100 hover:bg-slate-700',
  ghost: 'text-slate-300 hover:bg-slate-800 hover:text-white',
};

export function Button({ variant = 'secondary', className = '', ...props }: ButtonProps) {
  return (
    <button
      type="button"
      className={`rounded-md px-3 py-1.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${VARIANTS[variant]} ${className}`}
      {...props}
    />
  );
}

export function Panel({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="rounded-xl border border-slate-800 bg-slate-900 p-4">
      {title && (
        <h2 className="mb-3 text-xs font-semibold tracking-wider text-slate-400 uppercase">
          {title}
        </h2>
      )}
      {children}
    </section>
  );
}

/** A two-or-more-option toggle. */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled = false,
}: {
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className="inline-flex rounded-md bg-slate-800 p-0.5">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          disabled={disabled}
          onClick={() => {
            onChange(option.value);
          }}
          className={`rounded px-3 py-1 text-sm transition-colors disabled:opacity-40 ${
            option.value === value ? 'bg-slate-600 text-white' : 'text-slate-400 hover:text-white'
          }`}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function ErrorText({ children }: { children: ReactNode }) {
  return <p className="text-sm text-rose-400">{children}</p>;
}

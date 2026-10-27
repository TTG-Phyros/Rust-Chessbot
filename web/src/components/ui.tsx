import { type ButtonHTMLAttributes, type ReactNode, useEffect, useRef } from 'react';

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'md' | 'sm';
};

const VARIANTS: Record<NonNullable<ButtonProps['variant']>, string> = {
  primary: 'bg-signal font-semibold text-ink-950 hover:bg-[#d6ff7a]',
  secondary: 'border border-line bg-ink-800 text-bone hover:border-ink-600 hover:bg-ink-700',
  ghost: 'text-mute hover:bg-ink-800 hover:text-bone',
  danger: 'text-alert hover:bg-alert/10',
};

const SIZES: Record<NonNullable<ButtonProps['size']>, string> = {
  md: 'rounded-xl px-4 py-2 text-sm',
  sm: 'rounded-lg px-2.5 py-1 text-xs',
};

export function Button({
  variant = 'secondary',
  size = 'md',
  className = '',
  ...props
}: ButtonProps) {
  return (
    <button
      type="button"
      className={`inline-flex items-center justify-center gap-2 font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${SIZES[size]} ${VARIANTS[variant]} ${className}`}
      {...props}
    />
  );
}

/** Small uppercase label heading a card or a group. */
export function Eyebrow({ children, className = '' }: { children: ReactNode; className?: string }) {
  return (
    <h2 className={`text-[11px] font-medium tracking-[0.14em] text-dim uppercase ${className}`}>
      {children}
    </h2>
  );
}

export function Card({
  title,
  aside,
  highlight = false,
  className = '',
  children,
}: {
  title?: ReactNode;
  /** Shown on the right of the title: a unit, a count, a link. */
  aside?: ReactNode;
  /** Outlines the card in the signal color (the user's side, the selection). */
  highlight?: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    // min-w-0: in a grid, wide content (tables) scrolls instead of widening the card.
    <section
      className={`min-w-0 rounded-card border bg-ink-900 p-5 ${
        highlight ? 'border-signal/40 shadow-[0_0_40px_-12px] shadow-signal/25' : 'border-line'
      } ${className}`}
    >
      {(title ?? aside) && (
        <div className="mb-4 flex items-baseline justify-between gap-3">
          {title ? <Eyebrow>{title}</Eyebrow> : <span />}
          {aside && <div className="font-mono text-xs text-dim">{aside}</div>}
        </div>
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
  stretch = false,
  label,
}: {
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
  disabled?: boolean;
  /** Fills the width, options sharing it equally. */
  stretch?: boolean;
  label?: string;
}) {
  return (
    <div
      role="group"
      aria-label={label}
      className={`${stretch ? 'flex w-full' : 'inline-flex'} rounded-xl border border-line bg-ink-950 p-1`}
    >
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          disabled={disabled}
          aria-pressed={option.value === value}
          onClick={() => {
            onChange(option.value);
          }}
          className={`rounded-lg px-3 py-1.5 text-sm transition-colors disabled:opacity-40 ${
            stretch ? 'flex-1' : ''
          } ${
            option.value === value
              ? 'bg-bone font-medium text-ink-950'
              : 'text-mute hover:text-bone'
          }`}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

/** On/off switch with its label on the left. */
export function Toggle({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children: ReactNode;
}) {
  return (
    <label className="flex cursor-pointer items-center justify-between gap-3 text-sm text-bone">
      {children}
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => {
          onChange(!checked);
        }}
        className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${
          checked ? 'bg-think' : 'bg-ink-700'
        }`}
      >
        <span
          className={`absolute top-0.5 size-4 rounded-full bg-bone transition-[left] ${
            checked ? 'left-[18px]' : 'left-0.5'
          }`}
        />
      </button>
    </label>
  );
}

export type Tone = 'signal' | 'think' | 'alert' | 'warn' | 'idle';

const DOT_TONES: Record<Tone, string> = {
  signal: 'bg-signal shadow-[0_0_8px] shadow-signal/70',
  think: 'bg-think shadow-[0_0_8px] shadow-think/70',
  alert: 'bg-alert shadow-[0_0_8px] shadow-alert/70',
  warn: 'bg-warn shadow-[0_0_8px] shadow-warn/70',
  idle: 'bg-ink-600',
};

export function Dot({ tone }: { tone: Tone }) {
  return <span aria-hidden className={`inline-block size-1.5 rounded-full ${DOT_TONES[tone]}`} />;
}

/** Rounded status chip: `● API v0.1.0`. */
export function StatusPill({
  tone,
  title,
  children,
}: {
  tone: Tone;
  title?: string;
  children: ReactNode;
}) {
  return (
    <span
      title={title}
      className="inline-flex items-center gap-2 rounded-full border border-line bg-ink-900 px-3 py-1 font-mono text-[11px] whitespace-nowrap text-mute"
    >
      <Dot tone={tone} />
      {children}
    </span>
  );
}

export function ErrorText({ children }: { children: ReactNode }) {
  return <p className="text-sm text-alert">{children}</p>;
}

/** A labelled figure of a stats grid. */
export function Stat({
  label,
  value,
  unit,
  hint,
  large = false,
}: {
  label: string;
  value: ReactNode;
  unit?: string;
  hint?: string;
  large?: boolean;
}) {
  return (
    <div className="min-w-0">
      <p className="truncate text-xs text-dim">{label}</p>
      <p
        className={`mt-1 truncate font-mono text-bone ${large ? 'text-2xl' : 'text-lg'}`}
        title={hint}
      >
        {value}
        {unit && <span className="ml-1.5 text-xs text-dim">{unit}</span>}
      </p>
    </div>
  );
}

/** Text fields and selects. */
export const INPUT =
  'w-full rounded-xl border border-line bg-ink-950 px-3 py-2 font-mono text-sm text-bone outline-none transition-colors placeholder:text-ink-600 focus:border-signal/50 disabled:opacity-50';

/** Modal dialog, closed by Escape, a click outside or `onClose`. */
export function Modal({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(event) => {
        if (event.target === ref.current) onClose();
      }}
      className="m-auto w-[min(36rem,calc(100%-2rem))] rounded-card border border-line bg-ink-900 p-0 text-bone backdrop:bg-ink-950/70 backdrop:backdrop-blur-sm"
    >
      <div className="p-6">
        <div className="mb-5 flex items-center justify-between">
          <Eyebrow>{title}</Eyebrow>
          <Button variant="ghost" size="sm" onClick={onClose} aria-label="Fermer">
            ✕
          </Button>
        </div>
        {children}
      </div>
    </dialog>
  );
}

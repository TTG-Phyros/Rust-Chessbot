import type { ReviewClass } from '../../api/generated/ReviewClass';
import { CLASSES } from './review';

/** Round chip of a move class: its glyph on its color, its name as a tooltip. */
export function ClassBadge({ kind, large = false }: { kind: ReviewClass; large?: boolean }) {
  const style = CLASSES[kind];
  return (
    <span
      title={style.label}
      aria-label={style.label}
      className={`inline-flex shrink-0 items-center justify-center rounded-full font-sans font-bold text-ink-950 ${
        large ? 'size-7 text-xs' : 'size-4 text-[9px]'
      }`}
      style={{ background: style.color }}
    >
      {style.symbol}
    </span>
  );
}

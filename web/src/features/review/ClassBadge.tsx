import type { ReviewClass } from '../../api/generated/ReviewClass';
import { CLASSES } from './review';

const SIZES = {
  small: 'size-4 text-[9px]',
  large: 'size-7 text-xs',
  // As wide as its container (a corner of a square of the board).
  fill: 'aspect-square w-full text-[clamp(7px,1.4vw,14px)] shadow-[0_1px_3px_rgba(0,0,0,0.6)]',
};

/** Round chip of a move class: its glyph on its color, its name as a tooltip. */
export function ClassBadge({
  kind,
  large = false,
  fill = false,
}: {
  kind: ReviewClass;
  large?: boolean;
  fill?: boolean;
}) {
  const style = CLASSES[kind];
  return (
    <span
      title={style.label}
      aria-label={style.label}
      className={`inline-flex shrink-0 items-center justify-center rounded-full font-sans font-bold text-ink-950 ${
        SIZES[fill ? 'fill' : large ? 'large' : 'small']
      }`}
      style={{ background: style.color }}
    >
      {style.symbol}
    </span>
  );
}

import type { ScoreView } from '../api/generated/ScoreView';
import type { Side } from '../api/generated/Side';
import { formatScore, whiteShare } from '../lib/score';

/**
 * Evaluation bar; White's share grows from White's side of the board.
 * Vertical beside a board, horizontal under a title.
 */
export function EvalBar({
  score,
  orientation,
  horizontal = false,
}: {
  score: ScoreView | null;
  orientation: Side;
  horizontal?: boolean;
}) {
  const share = score ? whiteShare(score) : 0.5;
  const whiteFirst = orientation === 'white';
  const label = score ? formatScore(score) : '…';
  const whiteLeads = share >= 0.5;
  const meter = {
    title: `Évaluation : ${label}`,
    role: 'meter',
    'aria-label': 'Évaluation',
    'aria-valuemin': 0,
    'aria-valuemax': 100,
    'aria-valuenow': Math.round(share * 100),
  } as const;

  if (horizontal) {
    return (
      <div {...meter} className="relative h-1.5 w-full rounded-full bg-ink-700">
        <div
          className={`absolute inset-y-0 rounded-full bg-bone transition-[width] duration-300 ${
            whiteFirst ? 'left-0' : 'right-0'
          }`}
          style={{ width: `${share * 100}%` }}
        />
        <div
          className="absolute top-1/2 h-3 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full bg-signal shadow-[0_0_10px] shadow-signal/60 transition-[left] duration-300"
          style={{ left: `${(whiteFirst ? share : 1 - share) * 100}%` }}
        />
      </div>
    );
  }

  return (
    <div
      {...meter}
      className="relative flex w-7 shrink-0 overflow-hidden rounded-lg border border-line bg-ink-800"
    >
      <div
        className={`absolute inset-x-0 bg-bone transition-[height] duration-300 ${
          whiteFirst ? 'bottom-0' : 'top-0'
        }`}
        style={{ height: `${share * 100}%` }}
      />
      <div
        className="absolute inset-x-0 h-px bg-signal transition-[top] duration-300"
        style={{ top: `${(whiteFirst ? 1 - share : share) * 100}%` }}
      />
      <span
        className={`absolute inset-x-0 text-center font-mono text-[9px] ${
          whiteLeads === whiteFirst ? 'bottom-1' : 'top-1'
        } ${whiteLeads ? 'text-ink-950' : 'text-bone'}`}
      >
        {label.replace(/^([+-]?)0\./, '$1.')}
      </span>
    </div>
  );
}

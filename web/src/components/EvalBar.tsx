import type { ScoreView } from '../api/generated/ScoreView';
import type { Side } from '../api/generated/Side';
import { formatScore, whiteShare } from '../lib/score';

/** Vertical evaluation bar; White's share grows from White's side of the board. */
export function EvalBar({ score, orientation }: { score: ScoreView | null; orientation: Side }) {
  const share = score ? whiteShare(score) : 0.5;
  const whiteAtBottom = orientation === 'white';
  const label = score ? formatScore(score) : '…';
  const whiteLeads = share >= 0.5;

  return (
    <div
      className="relative flex w-6 shrink-0 overflow-hidden rounded bg-slate-700"
      title={`Évaluation : ${label}`}
      role="meter"
      aria-label="Évaluation"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(share * 100)}
    >
      <div
        className={`absolute inset-x-0 bg-slate-100 transition-[height] duration-300 ${
          whiteAtBottom ? 'bottom-0' : 'top-0'
        }`}
        style={{ height: `${share * 100}%` }}
      />
      <span
        className={`absolute inset-x-0 text-center text-[10px] font-semibold ${
          whiteLeads === whiteAtBottom ? 'bottom-1' : 'top-1'
        } ${whiteLeads ? 'text-slate-900' : 'text-slate-100'}`}
      >
        {label}
      </span>
    </div>
  );
}

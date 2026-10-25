import type { MoveCell } from '../lib/game';
import { moveRows } from '../lib/game';

type MoveListProps = {
  history: readonly string[];
  startFen?: string;
  /** Number of moves currently shown on the board (highlights the last one). */
  current?: number;
  /** Makes moves clickable: called with the number of moves to show. */
  onSelect?: (moves: number) => void;
};

/** Numbered SAN move list (`1. e4 e5`). */
export function MoveList({ history, startFen, current, onSelect }: MoveListProps) {
  const rows = moveRows(history, startFen);
  if (rows.length === 0) {
    return <p className="text-sm text-slate-500">Aucun coup joué.</p>;
  }

  const cell = (move: MoveCell | null) => {
    if (!move) return <span className="text-slate-600">…</span>;
    const active = current === move.ply + 1;
    const className = `rounded px-1.5 py-0.5 text-left ${
      active ? 'bg-slate-600 text-white' : 'text-slate-200'
    } ${onSelect ? 'hover:bg-slate-700' : ''}`;
    return onSelect ? (
      <button
        type="button"
        className={className}
        onClick={() => {
          onSelect(move.ply + 1);
        }}
      >
        {move.san}
      </button>
    ) : (
      <span className={className}>{move.san}</span>
    );
  };

  return (
    <ol className="grid max-h-72 grid-cols-[2.5rem_1fr_1fr] gap-y-0.5 overflow-y-auto font-mono text-sm">
      {rows.map((row) => (
        <li key={row.number} className="contents">
          <span className="py-0.5 text-slate-500">{row.number}.</span>
          {cell(row.white)}
          {cell(row.black)}
        </li>
      ))}
    </ol>
  );
}

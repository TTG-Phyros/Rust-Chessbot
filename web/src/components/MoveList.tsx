import { type MoveCell, moveRows } from '../lib/game';

type MoveListProps = {
  history: readonly string[];
  startFen?: string;
  /** Number of moves currently shown on the board (highlights the last one). */
  current?: number;
  /** Makes moves clickable: called with the number of moves to show. */
  onSelect?: (moves: number) => void;
};

/** Numbered SAN move list (`1. e4 e5`); the latest move is lit. */
export function MoveList({ history, startFen, current, onSelect }: MoveListProps) {
  const rows = moveRows(history, startFen);
  if (rows.length === 0) {
    return <p className="text-sm text-dim">Aucun coup joué.</p>;
  }
  const lit = current ?? history.length;

  const cell = (move: MoveCell | null) => {
    if (!move) return <span className="px-2 py-1 text-ink-600">…</span>;
    const active = lit === move.ply + 1;
    const className = `rounded-md px-2 py-1 text-left ${
      active ? 'font-medium text-signal' : 'text-bone'
    } ${onSelect ? 'hover:bg-ink-800' : ''}`;
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
    <ol className="max-h-80 overflow-y-auto font-mono text-sm">
      {rows.map((row) => {
        const active =
          (row.white && lit === row.white.ply + 1) || (row.black && lit === row.black.ply + 1);
        return (
          <li
            key={row.number}
            className={`grid grid-cols-[2.5rem_1fr_1fr] items-center rounded-lg ${
              active ? 'bg-signal/[0.07]' : ''
            }`}
          >
            <span className="py-1 pl-2 text-dim">{row.number}.</span>
            {cell(row.white)}
            {cell(row.black)}
          </li>
        );
      })}
    </ol>
  );
}

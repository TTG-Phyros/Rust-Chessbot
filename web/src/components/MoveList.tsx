import { type ReactNode, useEffect, useRef } from 'react';

import { type MoveCell, moveRows } from '../lib/game';

type MoveListProps = {
  history: readonly string[];
  startFen?: string;
  /** Number of moves currently shown on the board (highlights the last one). */
  current?: number;
  /** Makes moves clickable: called with the number of moves to show. */
  onSelect?: (moves: number) => void;
  /** Shown after a move (a badge), given its index in the history. */
  decorate?: (ply: number) => ReactNode;
  /** Height of the list before it scrolls (a Tailwind class). */
  maxHeight?: string;
};

/** Numbered SAN move list (`1. e4 e5`); the latest move is lit. */
export function MoveList({
  history,
  startFen,
  current,
  onSelect,
  decorate,
  maxHeight = 'max-h-80',
}: MoveListProps) {
  const rows = moveRows(history, startFen);
  const lit = current ?? history.length;
  const list = useRef<HTMLOListElement>(null);

  // Keeps the lit move in view, scrolling the list only (not the page).
  useEffect(() => {
    const element = list.current;
    const row = element?.querySelector<HTMLElement>('[data-active]');
    if (!element || !row) return;
    const top = row.offsetTop - element.offsetTop;
    if (top < element.scrollTop) element.scrollTop = top;
    else if (top + row.offsetHeight > element.scrollTop + element.clientHeight) {
      element.scrollTop = top + row.offsetHeight - element.clientHeight;
    }
  }, [lit, rows.length]);

  if (rows.length === 0) {
    return <p className="text-sm text-dim">Aucun coup joué.</p>;
  }

  const cell = (move: MoveCell | null) => {
    if (!move) return <span className="px-2 py-1 text-ink-600">…</span>;
    const active = lit === move.ply + 1;
    const className = `flex items-center justify-between gap-1 rounded-md px-2 py-1 text-left ${
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
        {decorate?.(move.ply)}
      </button>
    ) : (
      <span className={className}>
        {move.san}
        {decorate?.(move.ply)}
      </span>
    );
  };

  return (
    <ol ref={list} className={`${maxHeight} overflow-y-auto font-mono text-sm`}>
      {rows.map((row) => {
        const active =
          (row.white && lit === row.white.ply + 1) || (row.black && lit === row.black.ply + 1);
        return (
          <li
            key={row.number}
            data-active={active ? '' : undefined}
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

import { type CSSProperties, useState } from 'react';
import { type Arrow, Chessboard } from 'react-chessboard';

import type { MoveView } from '../api/generated/MoveView';
import type { PositionView } from '../api/generated/PositionView';
import type { Side } from '../api/generated/Side';
import { STARTPOS_FEN, destinations, movesBetween, pieceSide } from '../lib/game';

type BoardProps = {
  /** Server view of the position; its legal moves are the only ones allowed. */
  view: PositionView | null;
  orientation: Side;
  /** Whether the user may move the side to move right now. */
  interactive: boolean;
  onMove: (uci: string) => void;
  arrows?: Arrow[];
};

const LAST_MOVE: CSSProperties = { background: 'rgba(250, 204, 21, 0.35)' };
const SELECTED: CSSProperties = { background: 'rgba(59, 130, 246, 0.45)' };
const CHECK: CSSProperties = {
  background: 'radial-gradient(circle, rgba(239, 68, 68, 0.85) 0%, rgba(239, 68, 68, 0) 75%)',
};
const TARGET: CSSProperties = {
  background: 'radial-gradient(circle, rgba(15, 23, 42, 0.35) 22%, transparent 24%)',
};

const PROMOTION_NAMES: Record<string, string> = {
  q: 'Dame',
  r: 'Tour',
  b: 'Fou',
  n: 'Cavalier',
};
const PROMOTION_SYMBOLS: Record<Side, Record<string, string>> = {
  white: { q: '♕', r: '♖', b: '♗', n: '♘' },
  black: { q: '♛', r: '♜', b: '♝', n: '♞' },
};

/**
 * Interactive chessboard (drag and drop or click-click). Every move is
 * checked against the server's list of legal moves; promotions open a picker.
 */
export function Board({ view, orientation, interactive, onMove, arrows = [] }: BoardProps) {
  // react-chessboard v5 has no 'start' shorthand: always pass a real FEN.
  const fen = view?.fen ?? STARTPOS_FEN;
  const legal = view?.legalMoves ?? [];
  // Tied to the position they were made in, so they vanish when it changes.
  const [selection, setSelection] = useState<{ fen: string; square: string } | null>(null);
  const [promotion, setPromotion] = useState<{ fen: string; moves: MoveView[] } | null>(null);
  const selected = selection?.fen === fen ? selection.square : null;
  const choices = promotion?.fen === fen ? promotion.moves : null;

  const play = (from: string, to: string): boolean => {
    setSelection(null);
    const candidates = movesBetween(legal, from, to);
    const [first] = candidates;
    if (!first) return false;
    if (candidates.length > 1) {
      setPromotion({ fen, moves: candidates });
      return false;
    }
    onMove(first.uci);
    return true;
  };

  const squareStyles: Record<string, CSSProperties> = {};
  if (view?.lastMove) {
    squareStyles[view.lastMove.from] = LAST_MOVE;
    squareStyles[view.lastMove.to] = LAST_MOVE;
  }
  if (view?.checkSquare) squareStyles[view.checkSquare] = CHECK;
  if (selected) {
    squareStyles[selected] = SELECTED;
    for (const square of destinations(legal, selected)) squareStyles[square] = TARGET;
  }

  const ownPiece = (pieceType: string | undefined): boolean =>
    interactive && !!view && !!pieceType && pieceSide(pieceType) === view.sideToMove;

  return (
    <div className="relative aspect-square w-full">
      <Chessboard
        options={{
          id: 'board',
          position: fen,
          boardOrientation: orientation,
          allowDragging: interactive,
          arrows,
          squareStyles,
          animationDurationInMs: 150,
          darkSquareStyle: { backgroundColor: '#769656' },
          lightSquareStyle: { backgroundColor: '#eeeed2' },
          canDragPiece: ({ piece }) => ownPiece(piece.pieceType),
          onPieceDrop: ({ sourceSquare, targetSquare }) =>
            targetSquare ? play(sourceSquare, targetSquare) : false,
          onSquareClick: ({ square, piece }) => {
            if (selected && destinations(legal, selected).includes(square)) {
              play(selected, square);
            } else if (ownPiece(piece?.pieceType)) {
              setSelection({ fen, square });
            } else {
              setSelection(null);
            }
          },
        }}
      />
      {choices && view && (
        <div className="absolute inset-0 flex items-center justify-center bg-slate-950/60">
          <div className="rounded-xl bg-slate-900 p-4 shadow-xl">
            <p className="mb-3 text-center text-sm text-slate-300">Promotion</p>
            <div className="flex gap-2">
              {choices.map((move) => {
                const piece = move.promotion ?? 'q';
                return (
                  <button
                    key={move.uci}
                    type="button"
                    title={PROMOTION_NAMES[piece]}
                    className="flex size-14 items-center justify-center rounded-lg bg-slate-800 text-4xl hover:bg-slate-700"
                    onClick={() => {
                      setPromotion(null);
                      onMove(move.uci);
                    }}
                  >
                    {PROMOTION_SYMBOLS[view.sideToMove][piece]}
                  </button>
                );
              })}
            </div>
            <button
              type="button"
              className="mt-3 w-full text-xs text-slate-400 hover:text-white"
              onClick={() => {
                setPromotion(null);
              }}
            >
              Annuler
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

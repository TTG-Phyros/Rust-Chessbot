import { type CSSProperties, useState } from 'react';
import { type Arrow, Chessboard, type PieceRenderObject, defaultPieces } from 'react-chessboard';

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
  /**
   * Tilts the board in perspective, pieces standing up. Moves are then
   * played by clicks only: dragging does not follow a tilted board.
   */
  tilted?: boolean;
};

/** Board tilt, in degrees, and the stretch that stands the pieces back up. */
const TILT = 34;
const STAND_UP = 1 / Math.cos((TILT * Math.PI) / 180);

const DARK = '#232731';
const LIGHT = '#3b414d';
const LAST_MOVE: CSSProperties = { boxShadow: 'inset 0 0 0 100vmax rgba(200, 245, 96, 0.16)' };
const SELECTED: CSSProperties = {
  boxShadow: 'inset 0 0 0 2px rgba(200, 245, 96, 0.8), inset 0 0 0 100vmax rgba(200, 245, 96, 0.1)',
};
const CHECK: CSSProperties = {
  background: 'radial-gradient(circle, rgba(255, 138, 92, 0.9) 0%, rgba(255, 138, 92, 0) 72%)',
};
const TARGET: CSSProperties = {
  backgroundImage: 'radial-gradient(circle, rgba(236, 234, 228, 0.28) 18%, transparent 20%)',
};
const NOTATION: CSSProperties = {
  color: '#8a909c',
  fontFamily: 'JetBrains Mono, monospace',
  fontSize: '10px',
};

/**
 * The default pieces, lit for a dark board: black pieces get a bone rim so
 * they stay visible on dark squares; tilted, every piece stands up.
 */
function piecesFor(tilted: boolean): PieceRenderObject {
  return Object.fromEntries(
    Object.entries(defaultPieces).map(([code, render]) => {
      const style: CSSProperties = {
        filter: code.startsWith('b')
          ? 'drop-shadow(0 0 0.6px rgba(236, 234, 228, 0.9)) drop-shadow(0 2px 2px rgba(0, 0, 0, 0.5))'
          : 'drop-shadow(0 2px 2px rgba(0, 0, 0, 0.55))',
        ...(tilted && { transform: `scaleY(${STAND_UP})`, transformOrigin: '50% 92%' }),
      };
      return [
        code,
        (props?: Parameters<typeof render>[0]) => render({ ...props, svgStyle: style }),
      ];
    }),
  );
}

const FLAT_PIECES = piecesFor(false);
const TILTED_PIECES = piecesFor(true);

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
export function Board({
  view,
  orientation,
  interactive,
  onMove,
  arrows = [],
  tilted = false,
}: BoardProps) {
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
    for (const square of destinations(legal, selected)) {
      squareStyles[square] = { ...squareStyles[square], ...TARGET };
    }
  }

  const ownPiece = (pieceType: string | undefined): boolean =>
    interactive && !!view && !!pieceType && pieceSide(pieceType) === view.sideToMove;

  const board = (
    <Chessboard
      options={{
        id: 'board',
        position: fen,
        boardOrientation: orientation,
        allowDragging: interactive && !tilted,
        arrows,
        squareStyles,
        pieces: tilted ? TILTED_PIECES : FLAT_PIECES,
        animationDurationInMs: 150,
        // Tilted pieces rise above the top rank: let them overflow the board.
        boardStyle: tilted ? { overflow: 'visible' } : { borderRadius: 12 },
        darkSquareStyle: { backgroundColor: DARK },
        lightSquareStyle: { backgroundColor: LIGHT },
        darkSquareNotationStyle: NOTATION,
        lightSquareNotationStyle: NOTATION,
        dropSquareStyle: { boxShadow: 'inset 0 0 0 2px rgba(200, 245, 96, 0.6)' },
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
  );

  return (
    <div className="relative w-full">
      {tilted ? (
        // No overflow clipping on the way up: it would flatten the 3D. The
        // negative margin takes back the height the tilt frees above the board.
        <div className="-mt-[12%] px-[4%] pb-[2%] [perspective:1700px]">
          <div
            className="rounded-[14px] border border-ink-600/60 bg-ink-800 p-2 shadow-[0_40px_80px_-20px_rgba(0,0,0,0.9),0_10px_0_0_#0d0f14]"
            style={{ transform: `rotateX(${TILT}deg)`, transformOrigin: '50% 60%' }}
          >
            {board}
          </div>
        </div>
      ) : (
        <div className="rounded-2xl border border-line bg-ink-900 p-2">{board}</div>
      )}
      {choices && view && (
        <div className="absolute inset-0 z-10 flex items-center justify-center rounded-2xl bg-ink-950/70 backdrop-blur-sm">
          <div className="rounded-card border border-line bg-ink-900 p-5 shadow-xl">
            <p className="mb-3 text-center text-[11px] tracking-[0.14em] text-dim uppercase">
              Promotion
            </p>
            <div className="flex gap-2">
              {choices.map((move) => {
                const piece = move.promotion ?? 'q';
                return (
                  <button
                    key={move.uci}
                    type="button"
                    title={PROMOTION_NAMES[piece]}
                    className="flex size-14 items-center justify-center rounded-xl border border-line bg-ink-800 text-4xl hover:border-signal/50"
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
              className="mt-3 w-full text-xs text-dim hover:text-bone"
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

import { fenSquares } from '../lib/game';

/** Glyphs of the FEN piece letters (the solid ones, colored by CSS). */
const GLYPHS: Record<string, string> = {
  k: '♚',
  q: '♛',
  r: '♜',
  b: '♝',
  n: '♞',
  p: '♟',
};

/** Asks for the text presentation of a glyph (♟ would otherwise be an emoji). */
const TEXT = String.fromCodePoint(0xfe0e);

const FILES = 'abcdefgh';

/**
 * A small read-only board drawn from a FEN, White at the bottom; the
 * squares of `lastMove` (UCI) are lit.
 */
export function MiniBoard({ fen, lastMove }: { fen: string; lastMove?: string | null }) {
  const squares = fenSquares(fen);
  const lit = lastMove ? [lastMove.slice(0, 2), lastMove.slice(2, 4)] : [];
  return (
    <div
      className="grid aspect-square grid-cols-8 overflow-hidden rounded-lg border border-line"
      role="img"
      aria-label={`Position ${fen}`}
    >
      {squares.map((piece, index) => {
        const file = index % 8;
        const rank = 8 - Math.floor(index / 8);
        const name = `${FILES[file] ?? ''}${rank}`;
        const dark = (file + rank) % 2 === 1;
        const white = piece !== '' && piece === piece.toUpperCase();
        return (
          <span
            key={name}
            className={`flex items-center justify-center text-[clamp(8px,1.4vw,15px)] leading-none ${
              lit.includes(name) ? 'bg-signal/25' : dark ? 'bg-ink-700' : 'bg-ink-600'
            } ${white ? 'text-bone' : 'text-ink-950 [text-shadow:0_0_1px_#eceae4]'}`}
          >
            {piece ? `${GLYPHS[piece.toLowerCase()] ?? ''}${TEXT}` : ''}
          </span>
        );
      })}
    </div>
  );
}

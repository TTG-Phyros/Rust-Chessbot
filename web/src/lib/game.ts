/**
 * Game bookkeeping for the UI. There are no chess rules here: legality,
 * SAN and game status always come from the server.
 */
import type { GameRequest } from '../api/generated/GameRequest';
import type { MoveView } from '../api/generated/MoveView';
import type { Side } from '../api/generated/Side';

export const STARTPOS_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1';

export function newGame(fen?: string | null): GameRequest {
  return fen ? { fen, moves: [] } : { moves: [] };
}

export function withMove(game: GameRequest, uci: string): GameRequest {
  return { ...game, moves: [...game.moves, uci] };
}

/** The game with its last `count` moves taken back. */
export function withoutLast(game: GameRequest, count: number): GameRequest {
  return { ...game, moves: game.moves.slice(0, Math.max(0, game.moves.length - count)) };
}

/** The legal moves (from the server) going from `from` to `to`: several for a promotion. */
export function movesBetween(legal: readonly MoveView[], from: string, to: string): MoveView[] {
  return legal.filter((move) => move.from === from && move.to === to);
}

/** Destination squares of the legal moves starting on `from`. */
export function destinations(legal: readonly MoveView[], from: string): string[] {
  return [...new Set(legal.filter((move) => move.from === from).map((move) => move.to))];
}

/** Side to move and move number of a FEN (its 2nd and 6th fields). */
export function fenMoveNumber(fen: string): { side: Side; number: number } {
  const fields = fen.trim().split(/\s+/);
  const number = Number.parseInt(fields[5] ?? '1', 10);
  return { side: fields[1] === 'b' ? 'black' : 'white', number: Number.isNaN(number) ? 1 : number };
}

export type MoveCell = {
  san: string;
  /** Index of the move in the history. */
  ply: number;
};

export type MoveRow = { number: number; white: MoveCell | null; black: MoveCell | null };

/**
 * Groups a SAN history into numbered rows (`1. e4 e5`); a game starting with
 * Black to move gets an empty white cell (`12. … Kd7`).
 */
export function moveRows(history: readonly string[], startFen: string = STARTPOS_FEN): MoveRow[] {
  const start = fenMoveNumber(startFen);
  const cell = (ply: number): MoveCell | null => {
    const san = history[ply];
    return san === undefined ? null : { san, ply };
  };
  const rows: MoveRow[] = [];
  let ply = 0;
  let number = start.number;
  if (start.side === 'black' && history.length > 0) {
    rows.push({ number, white: null, black: cell(0) });
    ply = 1;
    number += 1;
  }
  for (; ply < history.length; ply += 2) {
    rows.push({ number, white: cell(ply), black: cell(ply + 1) });
    number += 1;
  }
  return rows;
}

/** A line of SAN moves with move numbers, from position `fen`: `12... Kd7 13. e4`. */
export function formatLine(moves: readonly string[], fen: string): string {
  return moveRows(moves, fen)
    .map((row) => {
      if (!row.white) return `${row.number}... ${row.black?.san ?? ''}`;
      const black = row.black ? ` ${row.black.san}` : '';
      return `${row.number}. ${row.white.san}${black}`;
    })
    .join(' ');
}

/** Color of a react-chessboard piece code (`wP`, `bK`). */
export function pieceSide(pieceType: string): Side {
  return pieceType.startsWith('w') ? 'white' : 'black';
}

import type { GameStatus } from '../api/generated/GameStatus';
import type { Side } from '../api/generated/Side';

export const SIDE_NAME: Record<Side, string> = { white: 'Blancs', black: 'Noirs' };

const DRAW_REASON: Record<Extract<GameStatus, { kind: 'draw' }>['reason'], string> = {
  fiftyMoveRule: 'règle des 50 coups',
  threefoldRepetition: 'triple répétition',
  insufficientMaterial: 'matériel insuffisant',
};

/** One-line French description of the state of the game. */
export function describeStatus(status: GameStatus, sideToMove: Side, inCheck: boolean): string {
  switch (status.kind) {
    case 'ongoing':
      return `Trait aux ${SIDE_NAME[sideToMove]}${inCheck ? ' — échec !' : ''}`;
    case 'checkmate':
      return `Échec et mat — les ${SIDE_NAME[status.winner]} gagnent`;
    case 'stalemate':
      return 'Pat — partie nulle';
    case 'draw':
      return `Partie nulle (${DRAW_REASON[status.reason]})`;
  }
}

/** PGN result of a finished game, `*` otherwise. */
export function resultOf(status: GameStatus): string {
  switch (status.kind) {
    case 'checkmate':
      return status.winner === 'white' ? '1-0' : '0-1';
    case 'stalemate':
    case 'draw':
      return '1/2-1/2';
    case 'ongoing':
      return '*';
  }
}

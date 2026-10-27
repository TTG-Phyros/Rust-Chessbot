import type { ReviewElo } from '../../api/generated/ReviewElo';
import type { ReviewGameRef } from '../../api/generated/ReviewGameRef';
import type { ReviewMotif } from '../../api/generated/ReviewMotif';
import type { ReviewPhase } from '../../api/generated/ReviewPhase';
import type { ReviewProfile } from '../../api/generated/ReviewProfile';
import type { ReviewRecord } from '../../api/generated/ReviewRecord';
import type { ReviewRequest } from '../../api/generated/ReviewRequest';
import type { ReviewWeakness } from '../../api/generated/ReviewWeakness';
import type { TimeClassView } from '../../api/generated/TimeClassView';
import { STARTPOS_FEN, formatLine } from '../../lib/game';
import { frenchSan } from '../../lib/san';

/** Which games of a player a profile covers. */
export type Filters = {
  timeClasses: TimeClassView[];
  maxGames: number;
  /** `YYYY-MM-DD`, or empty. */
  since: string;
  until: string;
  ratedOnly: boolean;
};

export const TIME_CLASSES: readonly TimeClassView[] = ['bullet', 'blitz', 'rapid', 'daily'];

export const DEFAULT_FILTERS: Filters = {
  timeClasses: [],
  maxGames: 50,
  since: '',
  until: '',
  ratedOnly: false,
};

const DAY = /^\d{4}-\d{2}-\d{2}$/;

/** The filters of the page's URL (`?tc=blitz,rapid&n=50&since=…&until=…&rated=1`). */
export function filtersFromSearch(params: URLSearchParams): Filters {
  const timeClasses = (params.get('tc') ?? '')
    .split(',')
    .filter((value): value is TimeClassView => (TIME_CLASSES as string[]).includes(value));
  const games = Number.parseInt(params.get('n') ?? '', 10);
  const day = (name: string) => {
    const value = params.get(name) ?? '';
    return DAY.test(value) ? value : '';
  };
  return {
    timeClasses,
    maxGames: Number.isNaN(games) || games < 1 ? DEFAULT_FILTERS.maxGames : games,
    since: day('since'),
    until: day('until'),
    ratedOnly: params.get('rated') === '1',
  };
}

/** The URL parameters of a player and filters (defaults left out). */
export function filtersToSearch(player: string, filters: Filters): Record<string, string> {
  const search: Record<string, string> = { player };
  if (filters.timeClasses.length > 0) search.tc = filters.timeClasses.join(',');
  if (filters.maxGames !== DEFAULT_FILTERS.maxGames) search.n = String(filters.maxGames);
  if (filters.since) search.since = filters.since;
  if (filters.until) search.until = filters.until;
  if (filters.ratedOnly) search.rated = '1';
  return search;
}

/** Query string of the `GET` endpoints of a player's games. */
export function filtersQuery(filters: Filters): string {
  const query = new URLSearchParams({ maxGames: String(filters.maxGames) });
  if (filters.timeClasses.length > 0) query.set('timeClasses', filters.timeClasses.join(','));
  if (filters.since) query.set('since', filters.since);
  if (filters.until) query.set('until', filters.until);
  if (filters.ratedOnly) query.set('ratedOnly', 'true');
  return query.toString();
}

/** Body of an estimate or of a review job. */
export function reviewRequest(player: string, filters: Filters, preset?: string): ReviewRequest {
  return {
    username: player,
    timeClasses: filters.timeClasses,
    maxGames: filters.maxGames,
    since: filters.since || null,
    until: filters.until || null,
    ratedOnly: filters.ratedOnly,
    preset: preset ?? null,
  };
}

/** `45 s`, `3 min 20 s`, `1 h 05 min`. */
export function formatDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  if (total < 60) return `${total} s`;
  if (total < 3600) return `${Math.floor(total / 60)} min ${String(total % 60).padStart(2, '0')} s`;
  return `${Math.floor(total / 3600)} h ${String(Math.floor((total % 3600) / 60)).padStart(2, '0')} min`;
}

/**
 * Where an estimate stands against the ratings of the calibration: at its
 * highest or lowest, the model can only say the rating is beyond.
 */
export function eloBound(elo: ReviewElo): 'high' | 'low' | null {
  if (elo.high - elo.rating < 1) return 'high';
  if (elo.rating - elo.low < 1) return 'low';
  return null;
}

/** `1850 (1612–2087)`: the estimate and its 95 % interval; `≥ 2569 (2094–2569)` at the top of the calibration. */
export function formatElo(elo: ReviewElo): string {
  const bound = eloBound(elo);
  const prefix = bound === 'high' ? '≥ ' : bound === 'low' ? '≤ ' : '';
  return `${prefix}${String(Math.round(elo.rating))} (${String(Math.round(elo.low))}–${String(Math.round(elo.high))})`;
}

/** Points scored, out of the games: `62 %`. */
export function scoreShare(record: ReviewRecord): number | null {
  const games = record.wins + record.draws + record.losses;
  return games === 0 ? null : ((record.wins + record.draws / 2) / games) * 100;
}

export const PHASE_NAME: Record<ReviewPhase, string> = {
  opening: 'Ouverture',
  middlegame: 'Milieu de partie',
  endgame: 'Finale',
};

export const MOTIF_NAME: Record<ReviewMotif, string> = {
  missedMate: 'Mats manqués',
  allowedMate: 'Mats concédés',
  hangingPiece: 'Pièces laissées en prise',
  missedMaterial: 'Gains de matériel manqués',
  missedFork: 'Fourchettes manquées',
  allowedFork: 'Fourchettes concédées',
  other: 'Autres erreurs (positionnelles)',
};

/** Title and explanation of a weakness. */
export function describeWeakness(
  weakness: ReviewWeakness,
  profile: ReviewProfile,
): { title: string; detail: string } {
  const cost = `${weakness.lostPerGame.toFixed(1)} points de chances de gain perdus par partie`;
  const kind = weakness.kind;
  switch (kind.kind) {
    case 'phase': {
      const stats = profile.phases.find((phase) => phase.phase === kind.value);
      return {
        title: `Erreurs en ${PHASE_NAME[kind.value].toLowerCase()}`,
        detail: `${String(weakness.count)} erreurs sur ${String(stats?.moves ?? 0)} coups, plus que dans les autres phases : ${cost}.`,
      };
    }
    case 'motif':
      return {
        title: MOTIF_NAME[kind.value],
        detail: `${String(weakness.count)} erreurs : ${cost}.`,
      };
    case 'timeTrouble': {
      const time = profile.time;
      const rate = (errors: number, moves: number) =>
        moves === 0 ? 0 : Math.round((errors / moves) * 100);
      return {
        title: 'Erreurs en manque de temps',
        detail: time
          ? `${String(rate(time.lowErrors, time.lowMoves))} % d'erreurs à court de temps, contre ${String(rate(time.normalErrors, time.normalMoves))} % sinon : ${cost}.`
          : cost,
      };
    }
    case 'conversion':
      return {
        title: 'Positions gagnantes non converties',
        detail: `${String(weakness.count)} parties gagnantes non gagnées sur ${String(profile.conversion.winning)} : ${cost}.`,
      };
    case 'line': {
      const line = profile.lines[kind.value];
      const moves = line ? formatLine(line.moves, STARTPOS_FEN) : '';
      const best = line?.best ? ` Le bon coup : ${frenchSan(line.best)}.` : '';
      return {
        title: 'Une position qui revient',
        detail: `${moves} : ${String(weakness.count)} erreurs.${best}`,
      };
    }
  }
}

/** Link to the game page, at the position after the move. */
export function gameLink(reference: ReviewGameRef, player: string): string {
  const id = encodeURIComponent(reference.gameId);
  return `/review/games/${id}?player=${encodeURIComponent(player)}&ply=${String(reference.ply + 1)}`;
}

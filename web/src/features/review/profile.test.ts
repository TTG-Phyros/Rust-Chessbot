import { describe, expect, it } from 'vitest';

import type { ReviewProfile } from '../../api/generated/ReviewProfile';
import {
  DEFAULT_FILTERS,
  describeWeakness,
  filtersFromSearch,
  filtersQuery,
  filtersToSearch,
  formatDuration,
  gameLink,
  reviewRequest,
  scoreShare,
} from './profile';

const COUNTS = {
  brilliant: 0,
  great: 0,
  best: 0,
  excellent: 0,
  good: 0,
  book: 0,
  inaccuracy: 0,
  mistake: 0,
  miss: 0,
  blunder: 0,
};

const PROFILE: ReviewProfile = {
  username: 'alice',
  games: 2,
  unanalysed: 0,
  skipped: 0,
  record: { wins: 0, draws: 0, losses: 2 },
  moves: 9,
  counts: COUNTS,
  repertoire: { white: [], black: [] },
  lines: [
    {
      color: 'white',
      moves: ['e4', 'e5', 'Nf3', 'Nc6', 'Bc4', 'Nd4'],
      fen: '',
      games: 2,
      errors: 2,
      best: 'Nxd4',
      played: [{ san: 'Nxe5', count: 2 }],
      lost: 55,
      examples: [],
    },
  ],
  phases: [
    { phase: 'opening', moves: 9, counts: COUNTS, lost: 55 },
    { phase: 'middlegame', moves: 0, counts: COUNTS, lost: 0 },
    { phase: 'endgame', moves: 0, counts: COUNTS, lost: 0 },
  ],
  time: {
    games: 1,
    secondsPerMove: 3,
    timeTroubleGames: 1,
    lowMoves: 10,
    lowErrors: 3,
    normalMoves: 40,
    normalErrors: 2,
    timeouts: 0,
  },
  conversion: { winning: 4, converted: 1, losing: 0, saved: 0 },
  motifs: [],
  weaknesses: [],
  perGame: [],
};

describe('filters', () => {
  it('round-trip through the URL, defaults left out', () => {
    const filters = {
      timeClasses: ['blitz', 'rapid'] as const,
      maxGames: 20,
      since: '2026-01-01',
      until: '',
      ratedOnly: true,
    };
    const search = filtersToSearch('alice', { ...filters, timeClasses: [...filters.timeClasses] });
    expect(search).toEqual({
      player: 'alice',
      tc: 'blitz,rapid',
      n: '20',
      since: '2026-01-01',
      rated: '1',
    });
    expect(filtersFromSearch(new URLSearchParams(search))).toEqual({
      ...filters,
      timeClasses: ['blitz', 'rapid'],
    });
    expect(filtersToSearch('bob', DEFAULT_FILTERS)).toEqual({ player: 'bob' });
  });

  it('ignore what they do not understand', () => {
    const params = new URLSearchParams({ tc: 'blitz,chess960', n: '-3', since: 'yesterday' });
    expect(filtersFromSearch(params)).toEqual({ ...DEFAULT_FILTERS, timeClasses: ['blitz'] });
  });

  it('make queries and requests', () => {
    const filters = { ...DEFAULT_FILTERS, timeClasses: ['bullet' as const], until: '2026-02-01' };
    expect(filtersQuery(filters)).toBe('maxGames=50&timeClasses=bullet&until=2026-02-01');
    expect(reviewRequest('alice', filters, 'fast')).toEqual({
      username: 'alice',
      timeClasses: ['bullet'],
      maxGames: 50,
      since: null,
      until: '2026-02-01',
      ratedOnly: false,
      preset: 'fast',
    });
  });
});

describe('formats', () => {
  it('format durations and scores', () => {
    expect(formatDuration(45.4)).toBe('45 s');
    expect(formatDuration(200)).toBe('3 min 20 s');
    expect(formatDuration(3900)).toBe('1 h 05 min');
    expect(scoreShare({ wins: 1, draws: 1, losses: 2 })).toBe(37.5);
    expect(scoreShare({ wins: 0, draws: 0, losses: 0 })).toBeNull();
  });

  it('link to the position after a move', () => {
    const link = gameLink(
      { gameId: 'a b', ply: 6, label: '4. Nxe5', class: 'blunder', loss: 30 },
      'Al',
    );
    expect(link).toBe('/review/games/a%20b?player=Al&ply=7');
  });
});

describe('describeWeakness', () => {
  const weakness = (kind: ReviewProfile['weaknesses'][number]['kind'], count = 2) => ({
    kind,
    lostPerGame: 27.5,
    count,
    examples: [],
  });

  it('names each kind', () => {
    expect(describeWeakness(weakness({ kind: 'phase', value: 'opening' }), PROFILE)).toEqual({
      title: 'Erreurs en ouverture',
      detail:
        '2 erreurs sur 9 coups, plus que dans les autres phases : 27.5 points de chances de gain perdus par partie.',
    });
    expect(describeWeakness(weakness({ kind: 'motif', value: 'allowedFork' }), PROFILE).title).toBe(
      'Fourchettes concédées',
    );
    expect(describeWeakness(weakness({ kind: 'timeTrouble' }), PROFILE).detail).toMatch(
      /^30 % d'erreurs à court de temps, contre 5 % sinon/,
    );
    expect(describeWeakness(weakness({ kind: 'conversion' }, 3), PROFILE).detail).toMatch(
      /^3 parties gagnantes non gagnées sur 4/,
    );
    expect(describeWeakness(weakness({ kind: 'line', value: 0 }), PROFILE).detail).toBe(
      '1. e4 e5 2. Cf3 Cc6 3. Fc4 Cd4 : 2 erreurs. Le bon coup : Cxd4.',
    );
  });
});

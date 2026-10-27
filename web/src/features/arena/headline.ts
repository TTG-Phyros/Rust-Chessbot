/** The one-sentence summary of a tournament shown at the top of the arena. */
import type { Tournament } from '../../api/generated/Tournament';
import { botLabel, stockfishLevel } from './stockfish';

export type Headline = {
  /** The bot the sentence is about (ours when one played). */
  bot: string;
  before: string;
  /** Emphasised part. */
  accent: string;
  after: string;
};

/** Points scored by `bot` against `other`, as a share of their games; `null` if unplayed. */
export function shareAgainst(tournament: Tournament, bot: string, other: string): number | null {
  const pairing = tournament.pairings.find(
    (p) => (p.a === bot && p.b === other) || (p.a === other && p.b === bot),
  );
  if (!pairing) return null;
  const games = pairing.wins + pairing.draws + pairing.losses;
  if (games === 0) return null;
  const won = pairing.a === bot ? pairing.wins : pairing.losses;
  return (won + pairing.draws / 2) / games;
}

/**
 * Our best-ranked bot, placed against the Stockfish levels it met: the
 * strongest it beats and the weakest it loses to. Without Stockfish, the
 * winner of the tournament.
 */
export function headline(tournament: Tournament): Headline | null {
  const ranked = tournament.standings.map((standing) => standing.bot);
  const ours = ranked.filter((bot) => stockfishLevel(bot) === null);
  // A saved bot rather than PeSTO, the reference.
  const focus = ours.find((bot) => bot !== 'classic') ?? ours[0];
  const running = tournament.state === 'running';

  if (focus) {
    let beaten: number | null = null;
    let lostTo: number | null = null;
    for (const bot of tournament.bots) {
      const level = stockfishLevel(bot);
      if (level === null) continue;
      const share = shareAgainst(tournament, focus, bot);
      if (share === null) continue;
      if (share > 0.5 && (beaten === null || level > beaten)) beaten = level;
      if (share < 0.5 && (lostTo === null || level < lostTo)) lostTo = level;
    }
    const name = botLabel(focus);
    if (beaten !== null && lostTo !== null) {
      return {
        bot: focus,
        before: `${name} bat `,
        accent: `Stockfish ${beaten}`,
        after: ` et ne cède qu’au niveau ${lostTo}.`,
      };
    }
    if (beaten !== null) {
      return {
        bot: focus,
        before: `${name} bat jusqu’à `,
        accent: `Stockfish ${beaten}`,
        after: '.',
      };
    }
    if (lostTo !== null) {
      return { bot: focus, before: `${name} cède dès `, accent: `Stockfish ${lostTo}`, after: '.' };
    }
  }

  const winner = ranked[0];
  if (!winner) return null;
  return {
    bot: winner,
    before: '',
    accent: botLabel(winner),
    after: running ? ' mène le tournoi.' : ' remporte le tournoi.',
  };
}

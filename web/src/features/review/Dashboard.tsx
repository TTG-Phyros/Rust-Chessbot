import { Link } from 'react-router';

import type { ReviewClassCounts } from '../../api/generated/ReviewClassCounts';
import type { ReviewGameRef } from '../../api/generated/ReviewGameRef';
import type { ReviewOpeningStats } from '../../api/generated/ReviewOpeningStats';
import type { ReviewProfile } from '../../api/generated/ReviewProfile';
import { Card, Stat } from '../../components/ui';
import { STARTPOS_FEN, formatLine } from '../../lib/game';
import { frenchSan } from '../../lib/san';
import { ClassBadge } from './ClassBadge';
import { MOTIF_NAME, PHASE_NAME, describeWeakness, gameLink, scoreShare } from './profile';
import { CLASSES, CLASS_ORDER } from './review';

const percent = (value: number | null | undefined, digits = 1) =>
  value === null || value === undefined ? '—' : value.toFixed(digits);

/** Errors (mistakes, misses, blunders) per 100 moves. */
function errorRate(counts: ReviewClassCounts, moves: number): string {
  if (moves === 0) return '—';
  return (((counts.mistake + counts.miss + counts.blunder) / moves) * 100).toFixed(1);
}

/** Links to the positions of some moves. */
function Examples({ refs, player }: { refs: readonly ReviewGameRef[]; player: string }) {
  if (refs.length === 0) return null;
  return (
    <ul className="mt-3 flex flex-wrap gap-2">
      {refs.map((reference) => (
        <li key={`${reference.gameId}-${String(reference.ply)}`}>
          <Link
            to={gameLink(reference, player)}
            title={`${CLASSES[reference.class].label} : −${reference.loss.toFixed(0)} points de chances`}
            className="inline-flex items-center gap-1.5 rounded-lg border border-line bg-ink-950 px-2 py-1 font-mono text-xs text-bone hover:border-ink-600"
          >
            <ClassBadge kind={reference.class} />
            {frenchSanLabel(reference.label)}
          </Link>
        </li>
      ))}
    </ul>
  );
}

/** `12... Nf3` in French notation. */
function frenchSanLabel(label: string): string {
  const space = label.lastIndexOf(' ');
  return space < 0
    ? frenchSan(label)
    : `${label.slice(0, space + 1)}${frenchSan(label.slice(space + 1))}`;
}

/** The moves of each class, as one bar. */
function ClassBar({ counts, moves }: { counts: ReviewClassCounts; moves: number }) {
  if (moves === 0) return null;
  return (
    <div>
      <div className="flex h-2.5 overflow-hidden rounded-full bg-ink-800">
        {CLASS_ORDER.map((kind) => (
          <span
            key={kind}
            title={`${CLASSES[kind].label} : ${String(counts[kind])}`}
            style={{ width: `${(counts[kind] / moves) * 100}%`, background: CLASSES[kind].color }}
          />
        ))}
      </div>
      <ul className="mt-3 flex flex-wrap gap-x-4 gap-y-1.5 text-xs text-mute">
        {CLASS_ORDER.filter((kind) => counts[kind] > 0).map((kind) => (
          <li key={kind} className="flex items-center gap-1.5">
            <ClassBadge kind={kind} />
            {CLASSES[kind].label} <span className="font-mono text-bone">{counts[kind]}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Summary({ profile }: { profile: ReviewProfile }) {
  const { record } = profile;
  const score = scoreShare(record);
  return (
    <Card
      title={`Profil de ${profile.username}`}
      aside={`${String(profile.games)} parties analysées${
        profile.unanalysed > 0 ? ` · ${String(profile.unanalysed)} à analyser` : ''
      }`}
    >
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
        <Stat
          label="Bilan"
          value={`${String(record.wins)} / ${String(record.draws)} / ${String(record.losses)}`}
          hint="Gains / nulles / pertes"
        />
        <Stat label="Score" value={percent(score, 0)} unit="%" />
        <Stat label="Précision" value={percent(profile.accuracy)} unit="%" large />
        <Stat label="ACPL" value={percent(profile.acpl, 0)} />
        <Stat label="Elo" value={profile.rating ?? '—'} />
        <Stat label="Adversaires" value={percent(profile.opponents, 0)} hint="Elo moyen" />
      </div>
      <div className="mt-5">
        <ClassBar counts={profile.counts} moves={profile.moves} />
      </div>
    </Card>
  );
}

function Weaknesses({ profile, player }: { profile: ReviewProfile; player: string }) {
  if (profile.weaknesses.length === 0) {
    return (
      <Card title="Points faibles">
        <p className="text-sm text-dim">Pas assez d'erreurs pour dégager une tendance.</p>
      </Card>
    );
  }
  return (
    <div className="grid gap-4 md:grid-cols-3">
      {profile.weaknesses.map((weakness, index) => {
        const { title, detail } = describeWeakness(weakness, profile);
        return (
          <Card key={index} title={`Point faible ${String(index + 1)}`} highlight={index === 0}>
            <p className="text-lg font-medium">{title}</p>
            <p className="mt-1 text-sm text-mute">{detail}</p>
            <Examples refs={weakness.examples} player={player} />
          </Card>
        );
      })}
    </div>
  );
}

function Phases({ profile }: { profile: ReviewProfile }) {
  return (
    <Card title="Par phase de jeu" aside="erreurs pour 100 coups">
      <table className="w-full text-sm">
        <thead className="text-left text-xs text-dim">
          <tr>
            <th className="pb-2 font-normal">Phase</th>
            <th className="pb-2 text-right font-normal">Coups</th>
            <th className="pb-2 text-right font-normal">Précision</th>
            <th className="pb-2 text-right font-normal">ACPL</th>
            <th className="pb-2 text-right font-normal">Erreurs</th>
            <th className="pb-2 text-right font-normal">Temps</th>
          </tr>
        </thead>
        <tbody className="font-mono">
          {profile.phases.map((phase) => (
            <tr key={phase.phase} className="border-t border-line">
              <td className="py-2 font-sans">{PHASE_NAME[phase.phase]}</td>
              <td className="py-2 text-right">{phase.moves}</td>
              <td className="py-2 text-right">{percent(phase.accuracy)}</td>
              <td className="py-2 text-right">{percent(phase.acpl, 0)}</td>
              <td className="py-2 text-right">{errorRate(phase.counts, phase.moves)}</td>
              <td className="py-2 text-right">
                {phase.secondsPerMove != null ? `${phase.secondsPerMove.toFixed(1)} s` : '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

function Clock({ profile }: { profile: ReviewProfile }) {
  const time = profile.time;
  if (!time) {
    return (
      <Card title="Gestion du temps">
        <p className="text-sm text-dim">Aucune partie avec pendule.</p>
      </Card>
    );
  }
  const rate = (errors: number, moves: number) =>
    moves === 0 ? '—' : ((errors / moves) * 100).toFixed(1);
  return (
    <Card title="Gestion du temps" aside={`${String(time.games)} parties avec pendule`}>
      <div className="grid grid-cols-2 gap-4">
        <Stat label="Temps moyen par coup" value={time.secondsPerMove.toFixed(1)} unit="s" />
        <Stat
          label="Parties en manque de temps"
          value={`${String(time.timeTroubleGames)} / ${String(time.games)}`}
        />
        <Stat
          label="Erreurs à court de temps"
          value={rate(time.lowErrors, time.lowMoves)}
          unit={`% · ${String(time.lowMoves)} coups`}
        />
        <Stat
          label="Erreurs sinon"
          value={rate(time.normalErrors, time.normalMoves)}
          unit={`% · ${String(time.normalMoves)} coups`}
        />
        <Stat label="Pertes au temps" value={time.timeouts} />
      </div>
    </Card>
  );
}

function Conversion({ profile }: { profile: ReviewProfile }) {
  const { winning, converted, losing, saved } = profile.conversion;
  const share = (part: number, whole: number) =>
    whole === 0 ? '—' : `${String(Math.round((part / whole) * 100))} %`;
  return (
    <Card title="Conversion et défense">
      <div className="grid grid-cols-2 gap-4">
        <div>
          <Stat label="Positions gagnantes converties" value={share(converted, winning)} large />
          <p className="mt-1 text-xs text-dim">
            {converted} gagnées sur {winning} parties où la victoire était là
          </p>
        </div>
        <div>
          <Stat label="Positions perdues sauvées" value={share(saved, losing)} large />
          <p className="mt-1 text-xs text-dim">
            {saved} nulles ou gains sur {losing} parties perdues à un moment
          </p>
        </div>
      </div>
    </Card>
  );
}

function Motifs({ profile, player }: { profile: ReviewProfile; player: string }) {
  const max = Math.max(1, ...profile.motifs.map((motif) => motif.count));
  return (
    <Card title="Motifs des erreurs" aside="erreurs, oublis et gaffes">
      {profile.motifs.length === 0 ? (
        <p className="text-sm text-dim">Aucune erreur.</p>
      ) : (
        <ul className="space-y-3">
          {profile.motifs.map((motif) => (
            <li key={motif.motif}>
              <div className="flex items-baseline justify-between gap-3 text-sm">
                <span>{MOTIF_NAME[motif.motif]}</span>
                <span className="font-mono text-mute">{motif.count}</span>
              </div>
              <div className="mt-1 h-1.5 rounded-full bg-ink-800">
                <div
                  className="h-full rounded-full bg-alert/80"
                  style={{ width: `${(motif.count / max) * 100}%` }}
                />
              </div>
              {motif.motif !== 'other' && (
                <Examples refs={motif.examples.slice(0, 2)} player={player} />
              )}
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function Repertoire({
  title,
  openings,
  player,
}: {
  title: string;
  openings: readonly ReviewOpeningStats[];
  player: string;
}) {
  return (
    <Card title={title} aside={`${String(openings.length)} ouvertures`}>
      {openings.length === 0 ? (
        <p className="text-sm text-dim">Aucune partie.</p>
      ) : (
        <table className="w-full text-sm">
          <thead className="text-left text-xs text-dim">
            <tr>
              <th className="pb-2 font-normal">Ouverture</th>
              <th className="pb-2 text-right font-normal">Parties</th>
              <th className="pb-2 text-right font-normal">G / N / P</th>
              <th className="pb-2 text-right font-normal">Score</th>
              <th className="pb-2 text-right font-normal">Précision</th>
            </tr>
          </thead>
          <tbody>
            {openings.slice(0, 10).map((opening) => {
              const { wins, draws, losses } = opening.record;
              return (
                <tr key={opening.family} className="border-t border-line">
                  <td className="max-w-0 py-2">
                    <Link
                      to={`/review/games/${encodeURIComponent(opening.lastGame)}?player=${encodeURIComponent(player)}`}
                      className="block truncate hover:text-signal"
                      title="Dernière partie"
                    >
                      <span className="font-mono text-xs text-dim">{opening.eco}</span>{' '}
                      {opening.family}
                    </Link>
                  </td>
                  <td className="py-2 text-right font-mono">{wins + draws + losses}</td>
                  <td className="py-2 text-right font-mono text-mute">
                    {wins} / {draws} / {losses}
                  </td>
                  <td className="py-2 text-right font-mono">
                    {percent(scoreShare(opening.record), 0)} %
                  </td>
                  <td className="py-2 text-right font-mono">{percent(opening.accuracy)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </Card>
  );
}

function Lines({ profile, player }: { profile: ReviewProfile; player: string }) {
  return (
    <Card title="Positions qui reviennent" aside="et le coup à retenir">
      {profile.lines.length === 0 ? (
        <p className="text-sm text-dim">
          Aucune position atteinte dans plusieurs parties avec une erreur à la clé.
        </p>
      ) : (
        <ul className="divide-y divide-line">
          {profile.lines.map((line, index) => (
            <li key={index} className="py-3 first:pt-0 last:pb-0">
              <p className="font-mono text-sm text-mute">
                {formatLine(line.moves, STARTPOS_FEN)}
                <span className="text-dim">
                  {' '}
                  · avec les {line.color === 'white' ? 'Blancs' : 'Noirs'}
                </span>
              </p>
              <p className="mt-1 text-sm">
                {line.games} parties, {line.errors} erreurs. Joué :{' '}
                {line.played.map((move, rank) => (
                  <span key={move.san} className="font-mono text-alert">
                    {rank > 0 && ', '}
                    {frenchSan(move.san)} ×{move.count}
                  </span>
                ))}
                {line.best && (
                  <>
                    {' '}
                    · à jouer :{' '}
                    <span className="font-mono text-signal">{frenchSan(line.best)}</span>
                  </>
                )}
              </p>
              <Examples refs={line.examples} player={player} />
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

/** Everything the profile says about a player. */
export function Dashboard({ profile, player }: { profile: ReviewProfile; player: string }) {
  return (
    <div className="space-y-4">
      <Summary profile={profile} />
      <Weaknesses profile={profile} player={player} />
      <div className="grid gap-4 lg:grid-cols-2">
        <Phases profile={profile} />
        <Clock profile={profile} />
        <Conversion profile={profile} />
        <Motifs profile={profile} player={player} />
        <Repertoire title="Avec les Blancs" openings={profile.repertoire.white} player={player} />
        <Repertoire title="Avec les Noirs" openings={profile.repertoire.black} player={player} />
      </div>
      <Lines profile={profile} player={player} />
    </div>
  );
}

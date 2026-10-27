import { useState } from 'react';
import { Link } from 'react-router';

import { api } from '../../api/client';
import type { ArenaView } from '../../api/generated/ArenaView';
import type { BotInfo } from '../../api/generated/BotInfo';
import type { MoveLimit } from '../../api/generated/MoveLimit';
import type { Tournament } from '../../api/generated/Tournament';
import type { TournamentState } from '../../api/generated/TournamentState';
import { useArena } from '../../api/hooks';
import {
  Button,
  Card,
  Dot,
  ErrorText,
  Eyebrow,
  INPUT,
  Segmented,
  Stat,
  type Tone,
} from '../../components/ui';
import { formatDate, formatElo, formatPoints } from '../training/format';
import { formatParam } from '../training/params';
import { estimateTournament, formatDuration } from './estimate';
import { headline, shareAgainst } from './headline';
import { PairingMatrix } from './PairingMatrix';
import { type Pair, allPairs, enabledPairs, idleBots } from './pairings';
import { LIMIT_OPTIONS, type LimitKind, describeLimit, limitError, limitOption } from './limit';
import {
  STOCKFISH_LEVELS,
  botLabel,
  describeLevel,
  stockfishBot,
  stockfishLevel,
} from './stockfish';

/** Name of the classical evaluation in tournaments. */
const CLASSIC = 'classic';

const STATE_LABEL: Record<TournamentState, string> = {
  running: 'En cours',
  finished: 'Terminé',
  stopped: 'Arrêté',
  failed: 'Échec',
};

const STATE_TONE: Record<TournamentState, Tone> = {
  running: 'signal',
  finished: 'think',
  stopped: 'warn',
  failed: 'alert',
};

/** Whether `bot` is one of ours (a saved network), not PeSTO or Stockfish. */
const isOurs = (bot: string) => bot !== CLASSIC && stockfishLevel(bot) === null;

const signed = (elo: number) => `${elo > 0 ? '+' : elo < 0 ? '−' : ''}${Math.abs(Math.round(elo))}`;

function gamesPlayed(tournament: Tournament): number {
  return tournament.pairings.reduce((sum, p) => sum + p.wins + p.draws + p.losses, 0);
}

function Hero({
  tournament,
  view,
  busy,
  onStop,
}: {
  tournament: Tournament | null;
  view: ArenaView;
  busy: boolean;
  onStop: () => void;
}) {
  if (!tournament) {
    return (
      <section className="rounded-card border border-line bg-ink-900 p-7">
        <Eyebrow>Arène</Eyebrow>
        <h1 className="mt-4 font-display text-5xl">Aucun tournoi joué.</h1>
        <p className="mt-4 max-w-xl text-sm text-mute">
          Choisissez des participants ci-dessous. Chaque paire de bots joue les mêmes ouvertures
          aléatoires avec les deux couleurs, nos bots avec la même réflexion (Stockfish à son
          niveau).
        </p>
      </section>
    );
  }
  const title = headline(tournament);
  const played = gamesPlayed(tournament);
  const total = tournament.pairings.length * tournament.gamesPerPair;
  const focus = title && isOurs(title.bot) ? title.bot : null;
  const standing = tournament.standings.find((s) => s.bot === focus);

  return (
    <section className="flex flex-col rounded-card border border-line bg-ink-900 p-7">
      <p className="flex items-center gap-2 text-[11px] tracking-[0.14em] text-dim uppercase">
        <Dot tone={STATE_TONE[tournament.state]} />
        Tournoi {STATE_LABEL[tournament.state].toLowerCase()} · {formatDate(tournament.updatedAt)}
      </p>
      {title && (
        <h1 className="mt-4 font-display text-[clamp(2.5rem,4.5vw,3.75rem)] leading-[0.95]">
          {title.before}
          <em className="text-signal">{title.accent}</em>
          {title.after}
        </h1>
      )}
      <p className="mt-4 max-w-xl text-sm text-mute">
        {describeLimit(tournament.limit).replace(/^./, (c) => c.toUpperCase())} ·{' '}
        {tournament.gamesPerPair} parties par paire · {played} parties jouées. Classement ajusté sur
        toutes les parties (modèle de Bradley-Terry).
      </p>
      {tournament.error && <ErrorText>{tournament.error}</ErrorText>}
      {tournament.state === 'running' && (
        <div className="mt-5">
          <div className="mb-1.5 flex items-center justify-between gap-3 text-xs text-dim">
            <span className="font-mono">
              {played} / {total} parties
            </span>
            <Button size="sm" onClick={onStop} disabled={busy}>
              Arrêter le tournoi
            </Button>
          </div>
          <div
            className="h-1.5 overflow-hidden rounded-full bg-ink-700"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={total}
            aria-valuenow={played}
          >
            <div
              className="h-full rounded-full bg-signal"
              style={{ width: `${total ? (100 * played) / total : 0}%` }}
            />
          </div>
        </div>
      )}
      <div className="mt-auto grid grid-cols-3 pt-8">
        <div className="pr-4">
          <Stat label="Bots" value={view.bots.length + 1} hint="PeSTO compris" large />
        </div>
        <div className="border-l border-line px-4">
          <Stat label="Tournois" value={view.tournaments.length} large />
        </div>
        <div className="border-l border-line px-4">
          {standing && focus ? (
            <Stat
              label={`Score ${botLabel(focus)}`}
              value={
                <span className="text-signal">
                  {standing.games > 0 ? Math.round((100 * standing.points) / standing.games) : 0} %
                </span>
              }
              large
            />
          ) : (
            <Stat label="En tête" value={botLabel(tournament.standings[0]?.bot ?? '—')} large />
          )}
        </div>
      </div>
    </section>
  );
}

/** The first three of the standings on blocks, ours lit. */
function Podium({ tournament }: { tournament: Tournament }) {
  const [first, second, third] = tournament.standings;
  const steps = [
    { standing: second, rank: 2, height: '60%' },
    { standing: first, rank: 1, height: '80%' },
    { standing: third, rank: 3, height: '44%' },
  ];
  return (
    <section className="relative flex min-h-72 items-end justify-center overflow-hidden rounded-card border border-line bg-ink-900 px-6 pt-10">
      <div
        aria-hidden
        className="pointer-events-none absolute bottom-0 left-1/2 h-24 w-3/4 -translate-x-1/2 rounded-full bg-signal/[0.06] blur-3xl"
      />
      <div className="relative flex h-60 w-full max-w-md items-end">
        {steps.map(({ standing, rank, height }) => {
          if (!standing) return <div key={rank} className="flex-1" />;
          const ours = isOurs(standing.bot);
          const face = ours
            ? 'bg-signal text-ink-950'
            : rank === 1
              ? 'bg-bone text-ink-950'
              : 'bg-ink-600 text-bone';
          const top = ours ? 'bg-[#e0ff98]' : rank === 1 ? 'bg-white' : 'bg-[#4a515f]';
          return (
            <div key={rank} className="relative flex-1" style={{ height }}>
              <div
                aria-hidden
                className={`absolute -top-3 right-0 left-0 h-3 origin-bottom-left [transform:skewX(-45deg)] ${top}`}
              />
              <div
                className={`flex h-full flex-col items-center px-2 pt-4 text-center shadow-[0_30px_60px_-20px_rgba(0,0,0,0.8)] ${face}`}
              >
                <span className="font-display text-5xl leading-none">{rank}</span>
                <span className="mt-2 max-w-full truncate text-xs font-semibold">
                  {botLabel(standing.bot)}
                </span>
                <span className="font-mono text-[10px] opacity-70">
                  {standing.games > 0 ? signed(standing.elo) : '—'}
                </span>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function Ranking({
  tournament,
  history,
  onShow,
}: {
  tournament: Tournament;
  history: readonly Tournament[];
  onShow: (id: string) => void;
}) {
  const relative = tournament.standings.some((s) => s.bot === CLASSIC)
    ? 'PeSTO = 0'
    : 'moyenne = 0';
  const reach = Math.max(1, ...tournament.standings.map((s) => Math.abs(s.elo)));
  return (
    <Card
      title={`Classement · Elo (${relative})`}
      aside={
        history.length > 1 && (
          <label className="flex items-center gap-2 font-sans">
            Historique
            <select
              value={tournament.id}
              onChange={(event) => {
                onShow(event.target.value);
              }}
              className={`${INPUT} w-auto py-1 text-xs`}
            >
              {history.map((t) => (
                <option key={t.id} value={t.id}>
                  {formatDate(t.createdAt)} — {t.bots.length} bots (
                  {STATE_LABEL[t.state].toLowerCase()})
                </option>
              ))}
            </select>
          </label>
        )
      }
    >
      <ol>
        {tournament.standings.map((standing, rank) => {
          const ours = isOurs(standing.bot);
          const width = `${(Math.abs(standing.elo) / reach) * 50}%`;
          return (
            <li
              key={standing.bot}
              className={`grid grid-cols-[2rem_minmax(0,1fr)_minmax(4rem,1fr)_4rem_3.5rem] items-center gap-3 rounded-xl px-3 py-3 ${
                ours ? 'bg-signal/[0.05]' : ''
              }`}
            >
              <span className="font-display text-2xl text-mute">{rank + 1}</span>
              <span className="truncate font-medium">{botLabel(standing.bot)}</span>
              <span className="relative h-2" aria-hidden>
                <span className="absolute inset-y-[-4px] left-1/2 w-px bg-line" />
                {standing.games > 0 && (
                  <span
                    className={`absolute inset-y-0 rounded-full ${
                      ours ? 'bg-signal' : standing.elo >= 0 ? 'bg-bone' : 'bg-ink-600'
                    }`}
                    style={standing.elo >= 0 ? { left: '50%', width } : { right: '50%', width }}
                  />
                )}
              </span>
              <span className="text-right font-mono text-sm">
                {standing.games > 0 ? signed(standing.elo) : '—'}
              </span>
              <span className="text-right font-mono text-sm text-dim">
                {standing.games > 0
                  ? `${Math.round((100 * standing.points) / standing.games)} %`
                  : '—'}
              </span>
            </li>
          );
        })}
      </ol>
      <p className="mt-2 flex justify-end gap-10 pr-3 text-[11px] text-dim">
        <span>Elo</span>
        <span>Score</span>
      </p>
    </Card>
  );
}

function CrossTable({ tournament }: { tournament: Tournament }) {
  const order = tournament.standings.map((standing) => standing.bot);
  const cell = 'flex h-11 items-center justify-center rounded-lg font-mono text-xs';
  return (
    <Card title="Table croisée · points de la ligne contre la colonne">
      <div className="-mx-1 overflow-x-auto px-1">
        <table className="w-full border-separate border-spacing-1.5 text-sm">
          <thead>
            <tr>
              <th />
              {order.map((bot) => (
                <th
                  key={bot}
                  scope="col"
                  className="px-1 text-center text-[11px] font-normal text-dim"
                >
                  {botLabel(bot)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {order.map((row) => (
              <tr key={row}>
                <th scope="row" className="pr-2 text-left text-xs font-semibold whitespace-nowrap">
                  {botLabel(row)}
                </th>
                {order.map((column) => {
                  if (row === column) {
                    return (
                      <td key={column}>
                        <span className={`${cell} bg-ink-950 text-ink-600`}>—</span>
                      </td>
                    );
                  }
                  const pairing = tournament.pairings.find(
                    (p) => (p.a === row && p.b === column) || (p.a === column && p.b === row),
                  );
                  if (!pairing) {
                    return (
                      <td key={column} title="Pas au programme">
                        <span className={`${cell} border border-line text-ink-600`}>×</span>
                      </td>
                    );
                  }
                  const games = pairing.wins + pairing.draws + pairing.losses;
                  const share = shareAgainst(tournament, row, column);
                  if (share === null) {
                    return (
                      <td key={column} title="Pas encore joué">
                        <span className={`${cell} border border-line text-dim`}>…</span>
                      </td>
                    );
                  }
                  const elo = pairing.a === row ? pairing.elo : -pairing.elo;
                  // The further from even, the stronger the color.
                  const strength = 0.25 + Math.min(1, Math.abs(share - 0.5) * 2) * 0.55;
                  const background =
                    share > 0.5
                      ? `rgba(200, 245, 96, ${strength})`
                      : share < 0.5
                        ? `rgba(255, 138, 92, ${strength * 0.45})`
                        : '#232731';
                  return (
                    <td
                      key={column}
                      title={`${formatElo(elo, pairing.eloError)} Elo sur ${games} parties`}
                    >
                      <span
                        className={`${cell} ${share > 0.5 && strength > 0.5 ? 'text-ink-950' : 'text-bone'}`}
                        style={{ background }}
                      >
                        {formatPoints(share * games)}/{games}
                      </span>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="mt-3 flex flex-wrap gap-x-5 gap-y-1 text-xs text-dim">
        <span className="flex items-center gap-1.5">
          <span className="size-2.5 rounded-sm bg-signal/70" /> gagne la paire
        </span>
        <span className="flex items-center gap-1.5">
          <span className="size-2.5 rounded-sm bg-alert/40" /> perd la paire
        </span>
        <span>× pas au programme · … pas encore joué · survolez une case pour l’écart Elo</span>
      </p>
    </Card>
  );
}

function Stable({
  bots,
  stockfish,
  canDelete,
  onDelete,
}: {
  bots: readonly BotInfo[];
  /** Version of Stockfish, if the training service has one. */
  stockfish: string | null;
  canDelete: boolean;
  onDelete: (name: string) => void;
}) {
  const chip = 'rounded-md bg-ink-950 px-2 py-0.5 font-mono text-[11px] text-mute';
  return (
    <div className="space-y-3">
      <Eyebrow className="px-1">Écurie · {bots.length + 1} bots</Eyebrow>
      <div className="rounded-2xl border border-line bg-ink-900 p-4">
        <div className="flex items-baseline justify-between gap-2">
          <span className="font-semibold">PeSTO</span>
          <span className="font-mono text-xs text-dim">0 · référence</span>
        </div>
        <p className="mt-1 text-sm text-mute">
          Évaluation classique (tables pièce-case), toujours disponible.
        </p>
        <Link
          to="/play?bot=classic"
          className="mt-3 inline-block text-sm font-medium text-signal underline underline-offset-4 hover:text-bone"
        >
          Jouer contre PeSTO
        </Link>
      </div>
      {bots.map((bot) => (
        <div key={bot.name} className="rounded-2xl border border-signal/25 bg-ink-900 p-4">
          <div className="flex items-baseline justify-between gap-2">
            <span className="truncate font-semibold">{bot.name}</span>
            <span className="font-mono text-xs whitespace-nowrap text-signal">
              {bot.elo != null ? formatElo(bot.elo, bot.eloError ?? 0) : '—'}
            </span>
          </div>
          <div className="mt-3 flex flex-wrap gap-1.5">
            <span className={chip}>
              {bot.network} · {bot.superbatch != null ? `sb ${bot.superbatch}` : 'dernière'}
            </span>
            {bot.params && (
              <>
                <span className={chip}>
                  {formatParam('selfplayNodes', bot.params.selfplayNodes)} nœuds
                </span>
                <span className={chip}>
                  lr {formatParam('learningRate', bot.params.learningRate)}
                </span>
              </>
            )}
            <span className={chip}>{formatDate(bot.createdAt)}</span>
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2">
            <Link
              to={`/play?bot=${encodeURIComponent(bot.name)}`}
              className="text-sm font-medium text-signal underline underline-offset-4 hover:text-bone"
            >
              Jouer contre ce bot
            </Link>
            <Link
              to={`/training?network=${encodeURIComponent(bot.network)}`}
              className="text-sm text-mute hover:text-bone"
            >
              Voir le réseau
            </Link>
            <span className="flex-1" />
            <Button
              variant="danger"
              size="sm"
              disabled={!canDelete}
              onClick={() => {
                if (window.confirm(`Supprimer le bot « ${bot.name} » ?`)) onDelete(bot.name);
              }}
            >
              Supprimer
            </Button>
          </div>
        </div>
      ))}
      <div
        className={`rounded-2xl border p-4 ${stockfish ? 'border-line bg-ink-900' : 'border-warn/30 bg-warn/[0.04]'}`}
      >
        <div className="flex items-baseline justify-between gap-2">
          <span className="font-semibold">Stockfish 1 à 10</span>
          {!stockfish && <span className="text-xs text-warn">indisponible</span>}
        </div>
        <p className="mt-1 text-sm text-mute">
          {stockfish ? (
            <>
              {stockfish}, bridé en 10 niveaux comme sur lichess ; il joue à son niveau, pas avec la
              réflexion choisie pour nos bots.
            </>
          ) : (
            <>
              Le service d’entraînement ne trouve pas Stockfish. Inclus dans les images Docker,
              sinon réglez <code className="font-mono text-warn">[arena] stockfish</code>.
            </>
          )}
        </p>
      </div>
      {bots.length === 0 && (
        <p className="px-1 text-sm text-dim">
          Aucun bot sauvegardé : dans{' '}
          <Link to="/training" className="underline hover:text-bone">
            Entraînement
          </Link>
          , choisissez un réseau et sauvegardez un de ses 5 meilleurs snapshots.
        </p>
      )}
    </div>
  );
}

/** Expected duration of the tournament being set up. */
function Estimate({
  bots,
  pairings,
  gamesPerPair,
  limit,
  threads,
}: {
  bots: string[];
  pairings: readonly Pair[];
  gamesPerPair: number;
  limit: MoveLimit;
  threads: number;
}) {
  const estimate = estimateTournament({ bots, pairings, gamesPerPair, limit, threads });
  const long = estimate.seconds > 3600;
  const slowest = estimate.slowest;
  const dominated =
    slowest !== null && pairings.length > 1 && slowest.seconds > estimate.seconds / 2;
  return (
    <p
      className={`text-xs ${long ? 'text-warn' : 'text-dim'}`}
      title="Ordre de grandeur mesuré sur la machine de référence : la longueur des parties varie, la profondeur coûte très différemment selon les positions, et un entraînement en parallèle ralentit tout."
    >
      Durée estimée <span className="font-mono">{formatDuration(estimate.seconds)}</span> sur{' '}
      {threads} thread
      {threads > 1 ? 's' : ''}
      {dominated &&
        ` — surtout ${botLabel(slowest.a)} contre ${botLabel(slowest.b)} (${formatDuration(slowest.seconds)})`}
    </p>
  );
}

function ParticipantChip({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label
      className={`flex cursor-pointer items-center gap-2 rounded-xl border px-3 py-2 text-sm transition-colors ${
        checked ? 'border-signal/60 bg-signal/[0.06]' : 'border-line hover:border-ink-600'
      }`}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
        className="size-4 accent-signal"
      />
      {label}
    </label>
  );
}

function TournamentForm({
  bots,
  stockfish,
  disabled,
  playing,
  onStart,
}: {
  bots: readonly BotInfo[];
  stockfish: string | null;
  disabled: boolean;
  /** A tournament is in progress (only one at a time). */
  playing: boolean;
  onStart: (request: {
    bots: string[];
    gamesPerPair: number;
    limit: MoveLimit;
    threads: number;
    pairings: { a: string; b: string }[] | null;
  }) => void;
}) {
  const [chosen, setChosen] = useState<string[]>([CLASSIC]);
  const [games, setGames] = useState('20');
  const [limitKind, setLimitKind] = useState<LimitKind>('nodes');
  // One draft per kind: switching back keeps what was typed.
  const [limitDrafts, setLimitDrafts] = useState<Record<LimitKind, string>>(
    () =>
      Object.fromEntries(
        LIMIT_OPTIONS.map((option) => [option.kind, String(option.initial)]),
      ) as Record<LimitKind, string>,
  );
  const limitValue = Number(limitDrafts[limitKind]);
  const limitProblem = limitError(limitKind, limitValue);
  const option = limitOption(limitKind);
  const [threads, setThreads] = useState('0');
  const names = [CLASSIC, ...bots.map((bot) => bot.name)];
  const levels = STOCKFISH_LEVELS.map((level) => stockfishBot(level.level));
  const participants = [...names, ...(stockfish ? levels : [])].filter((name) =>
    chosen.includes(name),
  );
  const toggle = (name: string, on: boolean) => {
    setChosen(on ? [...chosen, name] : chosen.filter((n) => n !== name));
  };
  // Pairings switched off in the cross table (the others are played).
  const [disabledPairs, setDisabledPairs] = useState<Set<string>>(() => new Set());
  const enabled = enabledPairs(participants, disabledPairs);
  const idle = idleBots(participants, enabled);
  const playingBots = participants.filter((bot) => !idle.includes(bot));
  const pairings = enabled.length;
  const gamesPerPair = Math.ceil(Number(games) / 2) * 2;
  // 0 = every core of the machine running the service: usually this one.
  const cores = navigator.hardwareConcurrency || 4;
  const workers = Number(threads) > 0 ? Number(threads) : cores;
  const valid =
    participants.length >= 2 &&
    participants.length <= 16 &&
    enabled.length > 0 &&
    Number.isInteger(Number(games)) &&
    Number(games) >= 2 &&
    Number(games) <= 2000 &&
    limitProblem === null &&
    Number.isInteger(Number(threads)) &&
    Number(threads) >= 0;

  return (
    // noValidate: the browser would only accept min + k × step; the form
    // checks the service's bounds itself.
    <form
      noValidate
      className="space-y-6"
      onSubmit={(event) => {
        event.preventDefault();
        if (valid) {
          onStart({
            // Bots without a pairing stay out of the tournament.
            bots: playingBots,
            gamesPerPair: Number(games),
            limit: { kind: limitKind, value: limitValue },
            threads: Number(threads),
            pairings:
              enabled.length === allPairs(participants).length
                ? null
                : enabled.map(([a, b]) => ({ a, b })),
          });
        }
      }}
    >
      <fieldset>
        <legend className="mb-2 text-sm text-mute">Participants</legend>
        <div className="flex flex-wrap gap-2">
          {names.map((name) => (
            <ParticipantChip
              key={name}
              label={botLabel(name)}
              checked={chosen.includes(name)}
              onChange={(on) => {
                toggle(name, on);
              }}
            />
          ))}
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-1.5">
          <span className="mr-2 text-sm text-dim">Stockfish</span>
          {STOCKFISH_LEVELS.map((level) => {
            const name = stockfishBot(level.level);
            const on = chosen.includes(name);
            return (
              <button
                key={name}
                type="button"
                aria-pressed={on}
                disabled={!stockfish}
                title={describeLevel(level)}
                onClick={() => {
                  toggle(name, !on);
                }}
                className={`size-9 rounded-lg border font-mono text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                  on
                    ? 'border-signal/60 bg-signal/10 text-signal'
                    : 'border-line text-mute hover:border-ink-600'
                }`}
              >
                {level.level}
              </button>
            );
          })}
        </div>
      </fieldset>

      <div>
        <p className="mb-2 text-sm text-mute">Réflexion de nos bots</p>
        <Segmented
          label="Réflexion de nos bots"
          value={limitKind}
          options={LIMIT_OPTIONS.map(({ kind, label }) => ({ value: kind, label }))}
          onChange={setLimitKind}
        />
      </div>

      <div className="grid gap-3 sm:grid-cols-3">
        <label className="block text-sm">
          <span className="text-mute">{option.field}</span>
          <input
            type="number"
            min={option.min}
            max={option.max}
            step={option.step}
            value={limitDrafts[limitKind]}
            onChange={(e) => {
              setLimitDrafts({ ...limitDrafts, [limitKind]: e.target.value });
            }}
            aria-invalid={limitProblem !== null}
            className={`mt-1.5 ${INPUT} ${limitProblem ? 'border-alert' : ''}`}
          />
        </label>
        <label
          className="block text-sm"
          title="Chaque paire joue ce nombre de parties, couleurs alternées, depuis les mêmes ouvertures."
        >
          <span className="text-mute">Parties par paire</span>
          <input
            type="number"
            min={2}
            max={2000}
            step={2}
            value={games}
            onChange={(e) => {
              setGames(e.target.value);
            }}
            className={`mt-1.5 ${INPUT}`}
          />
        </label>
        <label
          className="block text-sm"
          title="0 = tous les cœurs. Pendant un entraînement, en laisser au self-play."
        >
          <span className="text-mute">Threads (0 = tous)</span>
          <input
            type="number"
            min={0}
            max={256}
            step={1}
            value={threads}
            onChange={(e) => {
              setThreads(e.target.value);
            }}
            className={`mt-1.5 ${INPUT}`}
          />
        </label>
      </div>
      <p className={`-mt-3 text-xs ${limitProblem ? 'text-alert' : 'text-dim'}`}>
        {limitProblem ?? `${option.help} Stockfish joue toujours à son niveau.`}
      </p>

      {participants.length >= 2 && (
        <details className="rounded-xl border border-line px-4 py-3">
          <summary className="cursor-pointer text-sm text-mute hover:text-bone">
            Choisir les combats ({pairings} sur {allPairs(participants).length})
          </summary>
          <div className="mt-3">
            <PairingMatrix
              bots={participants}
              disabled={disabledPairs}
              onChange={setDisabledPairs}
              enabled={enabled}
              settings={{
                gamesPerPair,
                limit: { kind: limitKind, value: limitProblem ? 1 : limitValue },
                threads: workers,
              }}
            />
          </div>
        </details>
      )}

      <div className="flex flex-wrap items-center gap-4 rounded-2xl border border-line bg-ink-950 px-5 py-4">
        <div className="min-w-0 flex-1">
          <p className="font-mono text-base">
            {participants.length < 2
              ? 'Choisissez au moins 2 participants'
              : pairings === 0
                ? 'Activez au moins un combat'
                : `${pairings} paire${pairings > 1 ? 's' : ''} × ${gamesPerPair} = ${pairings * gamesPerPair} parties`}
          </p>
          {valid ? (
            <Estimate
              bots={playingBots}
              pairings={enabled}
              gamesPerPair={gamesPerPair}
              limit={{ kind: limitKind, value: limitValue }}
              threads={workers}
            />
          ) : (
            <p className="text-xs text-dim">joué par le service d’entraînement</p>
          )}
          {playing && (
            <p className="text-xs text-warn">Un tournoi est en cours : un seul à la fois.</p>
          )}
        </div>
        <Button type="submit" variant="primary" disabled={!valid || disabled}>
          🏆 Lancer le tournoi
        </Button>
      </div>
    </form>
  );
}

function ArenaContent({ view, refresh }: { view: ArenaView; refresh: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [shown, setShown] = useState<string | null>(null);
  const available = view.controller.state !== 'unavailable';
  const playing = view.controller.tournament ?? null;

  const act = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'erreur');
    } finally {
      setBusy(false);
      refresh();
    }
  };

  const tournament =
    view.tournaments.find((t) => t.id === shown) ??
    view.tournaments.find((t) => t.id === playing) ??
    view.tournaments[0] ??
    null;

  return (
    <div className="space-y-5">
      {error && <ErrorText>{error}</ErrorText>}

      <div
        className={`grid gap-5 ${tournament ? 'lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]' : ''}`}
      >
        <Hero
          tournament={tournament}
          view={view}
          busy={busy}
          onStop={() => void act(() => api.stopTournament())}
        />
        {tournament && <Podium tournament={tournament} />}
      </div>

      {tournament && (
        <div className="grid gap-5 lg:grid-cols-2">
          <Ranking tournament={tournament} history={view.tournaments} onShow={setShown} />
          <CrossTable tournament={tournament} />
        </div>
      )}

      <div className="grid gap-5 lg:grid-cols-[20rem_minmax(0,1fr)]">
        <Stable
          bots={view.bots}
          stockfish={view.controller.stockfish ?? null}
          canDelete={available && !playing && !busy}
          onDelete={(name) => void act(() => api.deleteBot(name))}
        />
        <Card
          title={
            <span className="text-xl font-semibold tracking-tight text-bone normal-case">
              Nouveau tournoi
            </span>
          }
          aside={<span className="font-sans">Toutes les paires s’affrontent</span>}
        >
          {!available && (
            <p className="mb-4 rounded-xl border border-alert/30 bg-alert/[0.06] px-3 py-2 text-sm text-mute">
              {view.controller.error ?? 'Service d’entraînement arrêté'} — les tournois sont joués
              par ce service ; bots et résultats restent consultables.
            </p>
          )}
          <TournamentForm
            bots={view.bots}
            stockfish={view.controller.stockfish ?? null}
            disabled={!available || playing !== null || busy}
            playing={playing !== null}
            onStart={(request) =>
              void act(async () => {
                const { id } = await api.startTournament(request);
                setShown(id);
              })
            }
          />
        </Card>
      </div>
    </div>
  );
}

export function ArenaPage() {
  const { view, error, refresh } = useArena();
  if (!view) {
    return error ? <ErrorText>{error}</ErrorText> : <p className="text-sm text-dim">Chargement…</p>;
  }
  return (
    <>
      {error && <ErrorText>{error}</ErrorText>}
      <ArenaContent view={view} refresh={refresh} />
    </>
  );
}

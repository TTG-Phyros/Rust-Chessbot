import { useState } from 'react';
import { Link } from 'react-router';

import { api } from '../../api/client';
import type { ArenaView } from '../../api/generated/ArenaView';
import type { BotInfo } from '../../api/generated/BotInfo';
import type { MoveLimit } from '../../api/generated/MoveLimit';
import type { Tournament } from '../../api/generated/Tournament';
import type { TournamentState } from '../../api/generated/TournamentState';
import { useArena } from '../../api/hooks';
import { Button, ErrorText, Panel, Segmented, Stat } from '../../components/ui';
import { formatDate, formatElo, formatPoints } from '../training/format';
import { formatParam } from '../training/params';
import { estimateTournament, formatDuration } from './estimate';
import { PairingMatrix } from './PairingMatrix';
import { type Pair, allPairs, enabledPairs, idleBots } from './pairings';
import { LIMIT_OPTIONS, type LimitKind, describeLimit, limitError, limitOption } from './limit';
import { STOCKFISH_LEVELS, botLabel, describeLevel, stockfishBot } from './stockfish';

/** Name of the classical evaluation in tournaments. */
const CLASSIC = 'classic';

const STATE_LABEL: Record<TournamentState, string> = {
  running: 'En cours',
  finished: 'Terminé',
  stopped: 'Arrêté',
  failed: 'Échec',
};

const STATE_DOT: Record<TournamentState, string> = {
  running: 'bg-emerald-400',
  finished: 'bg-sky-400',
  stopped: 'bg-amber-400',
  failed: 'bg-rose-500',
};

const INPUT =
  'mt-1 w-full rounded-md border border-slate-700 bg-slate-950 px-2 py-1.5 font-mono text-sm';

function BotList({
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
  return (
    <div className="-mx-4 overflow-x-auto px-4">
      <table className="w-full min-w-[40rem] text-sm">
        <thead>
          <tr className="text-left text-xs text-slate-500">
            <th className="py-1.5 font-normal">Bot</th>
            <th className="py-1.5 font-normal">Origine</th>
            <th className="py-1.5 text-right font-normal">Elo vs PeSTO</th>
            <th className="py-1.5 text-right font-normal">Nœuds self-play</th>
            <th className="py-1.5 text-right font-normal">Taux d’app.</th>
            <th className="py-1.5 text-right font-normal">Créé le</th>
            <th className="py-1.5" />
          </tr>
        </thead>
        <tbody>
          <tr className="border-t border-slate-800 text-slate-400">
            <td className="py-2 font-medium text-slate-200">PeSTO</td>
            <td className="py-2" colSpan={5}>
              Évaluation classique (tables pièce-case), toujours disponible : la référence 0 Elo.
            </td>
            <td />
          </tr>
          <tr className="border-t border-slate-800 text-slate-400">
            <td className="py-2 pr-3 font-medium whitespace-nowrap text-slate-200">
              Stockfish 1 à 10
            </td>
            <td className="py-2" colSpan={5}>
              {stockfish ? (
                <>
                  {stockfish}, bridé en 10 niveaux comme sur lichess (niveau de jeu, profondeur et
                  temps par coup croissants) ; il joue à son niveau, pas avec la réflexion choisie
                  pour nos bots.
                </>
              ) : (
                <span className="text-amber-300">
                  Indisponible : le service d’entraînement ne trouve pas Stockfish (inclus dans les
                  images Docker, sinon réglage <code>[arena] stockfish</code>).
                </span>
              )}
            </td>
            <td />
          </tr>
          {bots.map((bot) => (
            <tr key={bot.name} className="border-t border-slate-800">
              <td className="py-2 pr-3 font-medium whitespace-nowrap text-slate-100">{bot.name}</td>
              <td className="py-2 text-slate-400">
                <Link
                  to={`/training?network=${encodeURIComponent(bot.network)}`}
                  className="hover:text-slate-200 hover:underline"
                >
                  {bot.network}
                </Link>
                {bot.superbatch != null ? ` · sb ${bot.superbatch}` : ' · dernière version'}
              </td>
              <td className="py-2 text-right font-mono whitespace-nowrap">
                {bot.elo != null ? formatElo(bot.elo, bot.eloError ?? 0) : '—'}
              </td>
              <td className="py-2 text-right font-mono">
                {bot.params ? formatParam('selfplayNodes', bot.params.selfplayNodes) : '—'}
              </td>
              <td className="py-2 text-right font-mono">
                {bot.params ? formatParam('learningRate', bot.params.learningRate) : '—'}
              </td>
              <td className="py-2 text-right whitespace-nowrap text-slate-400">
                {formatDate(bot.createdAt)}
              </td>
              <td className="py-2 text-right">
                <Button
                  variant="ghost"
                  className="px-2 py-1 text-xs text-rose-400 hover:text-rose-300"
                  disabled={!canDelete}
                  onClick={() => {
                    if (window.confirm(`Supprimer le bot « ${bot.name} » ?`)) onDelete(bot.name);
                  }}
                >
                  Supprimer
                </Button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {bots.length === 0 && (
        <p className="mt-3 text-sm text-slate-400">
          Aucun bot sauvegardé : dans{' '}
          <Link to="/training" className="underline hover:text-slate-200">
            Entraînement
          </Link>
          , choisis un réseau et sauvegarde un de ses 5 meilleurs snapshots.
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
    <div
      className={`rounded-lg px-3 py-2 text-sm ${long ? 'bg-amber-950/40 text-amber-200' : 'bg-slate-950/60 text-slate-300'}`}
    >
      <p>
        Durée estimée :{' '}
        <span className="font-mono font-semibold">{formatDuration(estimate.seconds)}</span> sur{' '}
        {threads} thread{threads > 1 ? 's' : ''}
      </p>
      <p className="mt-1 text-xs text-slate-500">
        {dominated &&
          `Surtout ${botLabel(slowest.a)} contre ${botLabel(slowest.b)} (${formatDuration(slowest.seconds)}). `}
        Ordre de grandeur mesuré sur la machine de référence : la longueur des parties varie, la
        profondeur coûte très différemment selon les positions, et un entraînement en parallèle
        ralentit tout.
      </p>
    </div>
  );
}

function TournamentForm({
  bots,
  stockfish,
  disabled,
  onStart,
}: {
  bots: readonly BotInfo[];
  stockfish: string | null;
  disabled: boolean;
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
  const playing = participants.filter((bot) => !idle.includes(bot));
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
      className="space-y-4"
      onSubmit={(event) => {
        event.preventDefault();
        if (valid) {
          onStart({
            // Bots without a pairing stay out of the tournament.
            bots: playing,
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
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="space-y-4">
          <fieldset>
            <legend className="mb-2 text-sm text-slate-300">Participants</legend>
            <div className="flex flex-wrap gap-x-5 gap-y-2">
              {names.map((name) => (
                <label key={name} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={chosen.includes(name)}
                    onChange={(event) => {
                      toggle(name, event.target.checked);
                    }}
                    className="size-4 accent-emerald-500"
                  />
                  {botLabel(name)}
                </label>
              ))}
            </div>
            <div className="mt-3 flex flex-wrap items-center gap-1.5">
              <span className="mr-1 text-sm text-slate-400">Stockfish</span>
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
                    className={`min-w-8 rounded-md px-2 py-1 font-mono text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                      on
                        ? 'bg-emerald-600 text-white hover:bg-emerald-500'
                        : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    {level.level}
                  </button>
                );
              })}
            </div>
          </fieldset>
          <div>
            <div className="flex flex-wrap items-center gap-3">
              <span className="text-sm text-slate-300">Réflexion de nos bots</span>
              <Segmented
                value={limitKind}
                options={LIMIT_OPTIONS.map(({ kind, label }) => ({ value: kind, label }))}
                onChange={setLimitKind}
              />
            </div>
            <label className="mt-2 block text-sm">
              <span className="text-slate-400">{option.field}</span>
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
                className={`${INPUT} ${limitProblem ? 'border-rose-500' : ''}`}
              />
            </label>
            <p className={`mt-1 text-xs ${limitProblem ? 'text-rose-400' : 'text-slate-500'}`}>
              {limitProblem ?? `${option.help} Stockfish joue toujours à son niveau.`}
            </p>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <label
              className="block text-sm"
              title="Chaque paire joue ce nombre de parties, couleurs alternées, depuis les mêmes ouvertures."
            >
              <span className="text-slate-300">Parties par paire</span>
              <input
                type="number"
                min={2}
                max={2000}
                step={2}
                value={games}
                onChange={(e) => {
                  setGames(e.target.value);
                }}
                className={INPUT}
              />
            </label>
            <label
              className="block text-sm"
              title="0 = tous les cœurs. Pendant un entraînement, en laisser au self-play."
            >
              <span className="text-slate-300">Threads (0 = tous)</span>
              <input
                type="number"
                min={0}
                max={256}
                step={1}
                value={threads}
                onChange={(e) => {
                  setThreads(e.target.value);
                }}
                className={INPUT}
              />
            </label>
          </div>
        </div>
        <div className="space-y-4">
          {participants.length >= 2 ? (
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
          ) : (
            <p className="text-sm text-slate-400">
              Choisis au moins 2 participants : leur table croisée permettra de choisir les combats.
            </p>
          )}
          <div className="flex flex-wrap items-center gap-3">
            <Button type="submit" variant="primary" disabled={!valid || disabled}>
              Lancer le tournoi
            </Button>
            <span className="text-sm text-slate-400">
              {participants.length < 2
                ? 'Choisis au moins 2 participants.'
                : pairings === 0
                  ? 'Active au moins un combat.'
                  : `${pairings} combat${pairings > 1 ? 's' : ''} × ${gamesPerPair} parties = ${pairings * gamesPerPair} parties`}
            </span>
          </div>
          {valid && (
            <Estimate
              bots={playing}
              pairings={enabled}
              gamesPerPair={gamesPerPair}
              limit={{ kind: limitKind, value: limitValue }}
              threads={workers}
            />
          )}
        </div>
      </div>
    </form>
  );
}

function scoreOf(tournament: Tournament, row: string, column: string) {
  const pairing = tournament.pairings.find(
    (p) => (p.a === row && p.b === column) || (p.a === column && p.b === row),
  );
  if (!pairing) return null;
  const games = pairing.wins + pairing.draws + pairing.losses;
  if (games === 0) return { games, points: 0, elo: 0, error: 0 };
  const asA = pairing.a === row;
  const won = asA ? pairing.wins : pairing.losses;
  return {
    games,
    points: won + pairing.draws / 2,
    elo: asA ? pairing.elo : -pairing.elo,
    error: pairing.eloError,
  };
}

function TournamentView({
  tournament,
  onStop,
  busy,
}: {
  tournament: Tournament;
  onStop: () => void;
  busy: boolean;
}) {
  const played = tournament.pairings.reduce((sum, p) => sum + p.wins + p.draws + p.losses, 0);
  const total = tournament.pairings.length * tournament.gamesPerPair;
  const order = tournament.standings.map((standing) => standing.bot);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3 text-sm">
        <span className="flex items-center gap-2 font-medium">
          <span
            aria-hidden
            className={`inline-block size-2.5 rounded-full ${STATE_DOT[tournament.state]}`}
          />
          {STATE_LABEL[tournament.state]}
        </span>
        <span className="text-slate-400">
          {describeLimit(tournament.limit)} · {tournament.gamesPerPair} parties par paire · lancé le{' '}
          {formatDate(tournament.createdAt)}
        </span>
        <span className="flex-1" />
        {tournament.state === 'running' && (
          <Button onClick={onStop} disabled={busy}>
            Arrêter le tournoi
          </Button>
        )}
      </div>
      {tournament.error && <ErrorText>{tournament.error}</ErrorText>}
      <div>
        <div className="mb-1 flex justify-between text-xs text-slate-400">
          <span>Progression</span>
          <span>
            {played} / {total} parties
          </span>
        </div>
        <div
          className="h-2 overflow-hidden rounded-full bg-slate-800"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={total}
          aria-valuenow={played}
        >
          <div
            className="h-full rounded-full bg-emerald-500"
            style={{ width: `${total ? (100 * played) / total : 0}%` }}
          />
        </div>
      </div>

      <div className="-mx-4 overflow-x-auto px-4">
        <table className="w-full min-w-[28rem] text-sm">
          <thead>
            <tr className="text-left text-xs text-slate-500">
              <th className="py-1.5 pr-3 font-normal">#</th>
              <th className="py-1.5 font-normal">Bot</th>
              <th
                className="py-1.5 text-right font-normal"
                title={
                  order.includes(CLASSIC)
                    ? 'Relatif à PeSTO (0)'
                    : 'Relatif à la moyenne du tournoi'
                }
              >
                Elo {order.includes(CLASSIC) ? '(PeSTO = 0)' : '(moyenne = 0)'}
              </th>
              <th className="py-1.5 text-right font-normal">Points</th>
              <th className="py-1.5 text-right font-normal">Score</th>
            </tr>
          </thead>
          <tbody>
            {tournament.standings.map((standing, rank) => (
              <tr key={standing.bot} className="border-t border-slate-800">
                <td className="py-2 pr-3 font-mono text-slate-400">{rank + 1}</td>
                <td className="py-2 font-medium">{botLabel(standing.bot)}</td>
                <td className="py-2 text-right font-mono">
                  {standing.games > 0
                    ? `${standing.elo > 0 ? '+' : standing.elo < 0 ? '−' : ''}${Math.abs(Math.round(standing.elo))}`
                    : '—'}
                </td>
                <td className="py-2 text-right font-mono">
                  {formatPoints(standing.points)} / {standing.games}
                </td>
                <td className="py-2 text-right font-mono">
                  {standing.games > 0
                    ? `${Math.round((100 * standing.points) / standing.games)} %`
                    : '—'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div>
        <h3 className="mb-2 text-xs font-semibold tracking-wider text-slate-400 uppercase">
          Table croisée (points de la ligne contre la colonne)
        </h3>
        <div className="-mx-4 overflow-x-auto px-4">
          <table className="text-sm">
            <thead>
              <tr className="text-xs text-slate-500">
                <th />
                {order.map((bot) => (
                  <th key={bot} className="px-2 py-1.5 text-right font-normal">
                    {botLabel(bot)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {order.map((row) => (
                <tr key={row} className="border-t border-slate-800">
                  <th className="py-2 pr-3 text-left font-medium">{botLabel(row)}</th>
                  {order.map((column) => {
                    if (row === column) {
                      return (
                        <td key={column} className="px-2 text-right text-slate-600">
                          ·
                        </td>
                      );
                    }
                    const score = scoreOf(tournament, row, column);
                    if (!score) {
                      return (
                        <td
                          key={column}
                          className="px-2 text-right text-slate-700"
                          title="Pas au programme"
                        >
                          ×
                        </td>
                      );
                    }
                    if (score.games === 0) {
                      return (
                        <td
                          key={column}
                          className="px-2 text-right text-slate-600"
                          title="Pas encore joué"
                        >
                          …
                        </td>
                      );
                    }
                    const ratio = score.points / score.games;
                    return (
                      <td
                        key={column}
                        className={`px-2 text-right font-mono ${ratio > 0.5 ? 'text-emerald-300' : ratio < 0.5 ? 'text-rose-300' : 'text-slate-300'}`}
                        title={`${formatElo(score.elo, score.error)} Elo sur ${score.games} parties`}
                      >
                        {formatPoints(score.points)}/{score.games}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="mt-2 text-xs text-slate-500">
          Survole une case pour l’écart Elo de la paire et son intervalle à 95 % ; × : combat pas au
          programme, … : pas encore joué. Le classement, lui, ajuste un Elo sur toutes les parties
          (modèle de Bradley-Terry).
        </p>
      </div>
    </div>
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
    <div className="space-y-4">
      {!available && (
        <Panel>
          <p className="text-sm text-slate-400">
            {view.controller.error ?? 'Service d’entraînement arrêté'} — les tournois sont joués par
            ce service : bots et résultats restent consultables.
          </p>
        </Panel>
      )}
      {error && <ErrorText>{error}</ErrorText>}

      <div className="grid gap-4">
        <Panel title={`Bots (${view.bots.length + 1})`}>
          <BotList
            bots={view.bots}
            stockfish={view.controller.stockfish ?? null}
            canDelete={available && !playing && !busy}
            onDelete={(name) => void act(() => api.deleteBot(name))}
          />
        </Panel>
        <Panel title="Nouveau tournoi">
          <TournamentForm
            bots={view.bots}
            stockfish={view.controller.stockfish ?? null}
            disabled={!available || playing !== null || busy}
            onStart={(request) =>
              void act(async () => {
                const { id } = await api.startTournament(request);
                setShown(id);
              })
            }
          />
          {playing && (
            <p className="mt-3 text-sm text-slate-400">
              Un tournoi est en cours : un seul à la fois.
            </p>
          )}
        </Panel>
      </div>

      <Panel title="Tournoi">
        {tournament ? (
          <>
            {view.tournaments.length > 1 && (
              <label className="mb-4 flex items-center gap-2 text-sm text-slate-300">
                Historique
                <select
                  value={tournament.id}
                  onChange={(event) => {
                    setShown(event.target.value);
                  }}
                  className="rounded-md border border-slate-700 bg-slate-950 px-2 py-1 text-sm"
                >
                  {view.tournaments.map((t) => (
                    <option key={t.id} value={t.id}>
                      {formatDate(t.createdAt)} — {t.bots.map(botLabel).join(', ')} (
                      {STATE_LABEL[t.state].toLowerCase()})
                    </option>
                  ))}
                </select>
              </label>
            )}
            <TournamentView
              tournament={tournament}
              busy={busy}
              onStop={() => void act(() => api.stopTournament())}
            />
          </>
        ) : (
          <p className="text-sm text-slate-400">
            Aucun tournoi joué : choisis des participants ci-dessus. Chaque paire de bots joue les
            mêmes ouvertures aléatoires avec les deux couleurs, nos bots avec la même réflexion
            (Stockfish à son niveau).
          </p>
        )}
      </Panel>
    </div>
  );
}

export function ArenaPage() {
  const { view, error, refresh } = useArena();
  if (!view) {
    return error ? (
      <ErrorText>{error}</ErrorText>
    ) : (
      <p className="text-sm text-slate-400">Chargement…</p>
    );
  }
  return (
    <>
      {error && <ErrorText>{error}</ErrorText>}
      <div className="mb-4 grid grid-cols-2 gap-2 sm:grid-cols-4">
        <Stat label="Bots" value={view.bots.length + 1} hint="PeSTO compris" />
        <Stat label="Tournois" value={view.tournaments.length} />
        <Stat label="Tournoi en cours" value={view.controller.tournament ? 'oui' : 'non'} />
        <Stat
          label="Meilleur bot (dernier tournoi)"
          value={
            view.tournaments[0]?.standings[0] ? botLabel(view.tournaments[0].standings[0].bot) : '—'
          }
        />
      </div>
      <ArenaContent view={view} refresh={refresh} />
    </>
  );
}

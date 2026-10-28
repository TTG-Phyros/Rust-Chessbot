import { useCallback, useState } from 'react';
import { Link, useSearchParams } from 'react-router';

import type { ReviewGameSummary } from '../../api/generated/ReviewGameSummary';
import type { ReviewProfileGame } from '../../api/generated/ReviewProfileGame';
import { useReviewGames, useReviewOptions, useReviewProfile } from '../../api/hooks';
import { Button, Card, ErrorText, INPUT, Toggle } from '../../components/ui';
import { AnalysisPanel } from './AnalysisPanel';
import { Dashboard } from './Dashboard';
import {
  type Filters,
  TIME_CLASSES,
  filtersFromSearch,
  filtersQuery,
  filtersToSearch,
} from './profile';
import {
  type Outcome,
  TIME_CLASS_NAME,
  formatDate,
  formatResult,
  outcomeFor,
  sideOf,
} from './review';

const OUTCOME: Record<Outcome, { label: string; className: string }> = {
  win: { label: 'Gain', className: 'text-signal' },
  draw: { label: 'Nulle', className: 'text-mute' },
  loss: { label: 'Perte', className: 'text-alert' },
};

function GameRow({
  game,
  player,
  stats,
}: {
  game: ReviewGameSummary;
  player: string;
  stats: ReviewProfileGame | undefined;
}) {
  const side = sideOf(game, player) ?? 'white';
  const opponent = side === 'white' ? game.black : game.white;
  const outcome = OUTCOME[outcomeFor(game.result, side)];
  return (
    <li>
      <Link
        to={`/review/games/${encodeURIComponent(game.id)}?player=${encodeURIComponent(player)}`}
        className="grid grid-cols-[6.5rem_4.5rem_minmax(0,1fr)_3.5rem] items-center gap-3 rounded-xl px-3 py-2.5 text-sm hover:bg-ink-800 md:grid-cols-[6.5rem_4.5rem_minmax(0,14rem)_3.5rem_minmax(0,1fr)_5rem]"
      >
        <span className="font-mono text-xs text-dim">{formatDate(game.endTime)}</span>
        <span className="text-mute">{TIME_CLASS_NAME[game.timeClass]}</span>
        <span className="flex min-w-0 items-center gap-2">
          <span
            aria-label={side === 'white' ? 'avec les Blancs' : 'avec les Noirs'}
            className={`size-2.5 shrink-0 rounded-full border border-ink-600 ${
              side === 'white' ? 'bg-bone' : 'bg-ink-950'
            }`}
          />
          <span className="truncate">{opponent.username}</span>
          <span className="font-mono text-xs text-dim">{opponent.rating}</span>
        </span>
        <span className={`font-mono ${outcome.className}`} title={formatResult(game.result)}>
          {outcome.label}
        </span>
        <span className="hidden truncate text-mute md:block">
          {game.opening ? `${game.opening.eco} ${game.opening.name}` : '—'}
        </span>
        <span className="hidden text-right font-mono text-xs md:block">
          {stats?.accuracy != null ? (
            <span className="text-bone" title="Précision">
              {stats.accuracy.toFixed(1)} %
            </span>
          ) : (
            <span className="text-dim">non analysée</span>
          )}
        </span>
      </Link>
    </li>
  );
}

/**
 * Username and filters. The username is applied to the URL on submit; with a
 * player shown, the filters as soon as they change (the number of games once
 * typed: on leaving the field or Enter), so that the analysis below always
 * covers the games the form shows.
 */
function SearchForm({
  player,
  filters,
  maxGames,
  onSubmit,
}: {
  player: string | null;
  filters: Filters;
  maxGames: number;
  onSubmit: (player: string, filters: Filters) => void;
}) {
  const [name, setName] = useState(player ?? '');
  const [draft, setDraft] = useState(filters);
  const apply = (next: Filters) => {
    setDraft(next);
    const username = name.trim() || player;
    if (player && username) onSubmit(username, next);
  };
  const toggleClass = (timeClass: Filters['timeClasses'][number]) => {
    apply({
      ...draft,
      timeClasses: draft.timeClasses.includes(timeClass)
        ? draft.timeClasses.filter((value) => value !== timeClass)
        : [...draft.timeClasses, timeClass],
    });
  };
  return (
    <Card>
      <form
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          const username = name.trim();
          if (username) onSubmit(username, draft);
        }}
      >
        <div className="flex flex-wrap gap-3">
          <input
            className={`${INPUT} max-w-sm`}
            placeholder="Pseudo Chess.com"
            aria-label="Pseudo Chess.com"
            value={name}
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
          <Button type="submit" variant="primary">
            Voir le profil
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-x-6 gap-y-3 text-sm">
          <div className="flex flex-wrap gap-1.5" role="group" aria-label="Cadences">
            {TIME_CLASSES.map((timeClass) => {
              const active = draft.timeClasses.includes(timeClass);
              return (
                <button
                  key={timeClass}
                  type="button"
                  aria-pressed={active}
                  onClick={() => {
                    toggleClass(timeClass);
                  }}
                  className={`rounded-lg border px-2.5 py-1 text-xs transition-colors ${
                    active
                      ? 'border-signal/60 bg-signal/10 text-signal'
                      : 'border-line text-mute hover:text-bone'
                  }`}
                >
                  {TIME_CLASS_NAME[timeClass]}
                </button>
              );
            })}
            <span className="self-center pl-1 text-xs text-dim">
              {draft.timeClasses.length === 0 ? 'toutes cadences' : ''}
            </span>
          </div>
          <label className="flex items-center gap-2 text-mute">
            Parties
            <input
              type="number"
              min={1}
              max={maxGames}
              className={`${INPUT} w-20`}
              value={draft.maxGames}
              onChange={(event) => {
                const value = Number.parseInt(event.target.value, 10);
                if (!Number.isNaN(value)) {
                  setDraft({ ...draft, maxGames: Math.max(1, Math.min(maxGames, value)) });
                }
              }}
              onBlur={() => {
                if (draft.maxGames !== filters.maxGames) apply(draft);
              }}
            />
          </label>
          <label className="flex items-center gap-2 text-mute">
            Du
            <input
              type="date"
              className={`${INPUT} w-40`}
              value={draft.since}
              onChange={(event) => {
                apply({ ...draft, since: event.target.value });
              }}
            />
          </label>
          <label className="flex items-center gap-2 text-mute">
            au
            <input
              type="date"
              className={`${INPUT} w-40`}
              value={draft.until}
              onChange={(event) => {
                apply({ ...draft, until: event.target.value });
              }}
            />
          </label>
          <div className="w-44">
            <Toggle
              checked={draft.ratedOnly}
              onChange={(ratedOnly) => {
                apply({ ...draft, ratedOnly });
              }}
            >
              Classées seulement
            </Toggle>
          </div>
        </div>
      </form>
    </Card>
  );
}

export function ReviewPage() {
  const [search, setSearch] = useSearchParams();
  const player = search.get('player');
  const filters = filtersFromSearch(search);
  const query = filtersQuery(filters);
  // Bumped when a job ends: the profile and the games are read again.
  const [version, setVersion] = useState(0);
  const refresh = useCallback(() => {
    setVersion((value) => value + 1);
  }, []);
  const options = useReviewOptions();
  const profile = useReviewProfile(player, query, version);
  const games = useReviewGames(player, query, version);
  const stats = new Map((profile.view?.perGame ?? []).map((game) => [game.id, game]));

  return (
    <div className="space-y-6">
      <SearchForm
        // A new URL (back button) resets the form.
        key={search.toString()}
        player={player}
        filters={filters}
        maxGames={options.view?.maxGames ?? 200}
        onSubmit={(username, next) => {
          setSearch(filtersToSearch(username, next));
        }}
      />

      {player && options.view && (
        <AnalysisPanel
          key={`${player}?${query}`}
          player={player}
          filters={filters}
          options={options.view}
          version={version}
          onDone={refresh}
        />
      )}
      {options.error && <ErrorText>{options.error}</ErrorText>}

      {player && profile.error && <ErrorText>{profile.error}</ErrorText>}
      {player && profile.view && profile.view.games > 0 && (
        <Dashboard profile={profile.view} player={player} />
      )}
      {player && profile.view?.games === 0 && (
        <p className="text-sm text-dim">
          Aucune partie analysée pour ces filtres : lancez l'analyse ci-dessus.
        </p>
      )}

      {player && (
        <Card
          title={`Parties de ${player}`}
          aside={games.view ? `${String(games.view.length)} en cache` : undefined}
        >
          {games.error && <ErrorText>{games.error}</ErrorText>}
          {games.loading && <p className="text-sm text-dim">Chargement…</p>}
          {games.view?.length === 0 && (
            <p className="text-sm text-dim">Aucune partie en cache pour ces filtres.</p>
          )}
          {games.view && games.view.length > 0 && (
            <ol className="-mx-3">
              {games.view.map((game) => (
                <GameRow key={game.id} game={game} player={player} stats={stats.get(game.id)} />
              ))}
            </ol>
          )}
        </Card>
      )}
    </div>
  );
}

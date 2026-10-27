import { useState } from 'react';
import { Link, useSearchParams } from 'react-router';

import type { ReviewGameSummary } from '../../api/generated/ReviewGameSummary';
import { useReviewGames } from '../../api/hooks';
import { Button, Card, ErrorText, INPUT } from '../../components/ui';
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

function GameRow({ game, player }: { game: ReviewGameSummary; player: string }) {
  const side = sideOf(game, player) ?? 'white';
  const opponent = side === 'white' ? game.black : game.white;
  const outcome = OUTCOME[outcomeFor(game.result, side)];
  return (
    <li>
      <Link
        to={`/review/games/${encodeURIComponent(game.id)}?player=${encodeURIComponent(player)}`}
        className="grid grid-cols-[6.5rem_4.5rem_minmax(0,1fr)_3.5rem] items-center gap-3 rounded-xl px-3 py-2.5 text-sm hover:bg-ink-800 md:grid-cols-[6.5rem_4.5rem_minmax(0,14rem)_3.5rem_minmax(0,1fr)_6rem]"
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
        <span className="hidden text-right text-xs md:block">
          {game.analysed ? (
            <span className="text-signal">analysée</span>
          ) : (
            <span className="text-dim">non analysée</span>
          )}
        </span>
      </Link>
    </li>
  );
}

export function ReviewPage() {
  const [search, setSearch] = useSearchParams();
  const player = search.get('player');
  const [name, setName] = useState(player ?? '');
  const games = useReviewGames(player);

  return (
    <div className="space-y-6">
      <form
        className="flex max-w-xl gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          const username = name.trim();
          if (username) setSearch({ player: username });
        }}
      >
        <input
          className={INPUT}
          placeholder="Pseudo Chess.com"
          aria-label="Pseudo Chess.com"
          value={name}
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
        <Button type="submit" variant="primary">
          Voir les parties
        </Button>
      </form>

      {player && (
        <Card
          title={`Parties de ${player}`}
          aside={games.view ? `${games.view.length} en cache` : undefined}
        >
          {games.error && <ErrorText>{games.error}</ErrorText>}
          {games.loading && <p className="text-sm text-dim">Chargement…</p>}
          {games.view?.length === 0 && (
            <p className="text-sm text-dim">
              Aucune partie en cache pour ce joueur : téléchargez et analysez-les d'abord (
              <code className="font-mono">chess-reviewer analyse {player}</code>).
            </p>
          )}
          {games.view && games.view.length > 0 && (
            <ol className="-mx-3">
              {games.view.map((game) => (
                <GameRow key={game.id} game={game} player={player} />
              ))}
            </ol>
          )}
        </Card>
      )}
    </div>
  );
}

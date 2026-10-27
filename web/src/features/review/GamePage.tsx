import { type CSSProperties, useEffect, useMemo, useState } from 'react';
import { Link, useParams, useSearchParams } from 'react-router';

import type { ReviewGameView } from '../../api/generated/ReviewGameView';
import type { ReviewPlayer } from '../../api/generated/ReviewPlayer';
import type { ReviewPlayerStats } from '../../api/generated/ReviewPlayerStats';
import type { Side } from '../../api/generated/Side';
import { usePosition, useReviewGame } from '../../api/hooks';
import { Board } from '../../components/Board';
import { EvalBar } from '../../components/EvalBar';
import { EvalGraph } from '../../components/EvalGraph';
import { MoveList } from '../../components/MoveList';
import { Button, Card, ErrorText, Stat } from '../../components/ui';
import { formatLine, moveLabel, newGame } from '../../lib/game';
import { frenchSan } from '../../lib/san';
import { formatScore } from '../../lib/score';
import { ClassBadge } from './ClassBadge';
import {
  BEST_ARROW,
  CLASSES,
  CLASS_ORDER,
  MISSED_ARROW,
  TIME_CLASS_NAME,
  clockAt,
  formatClock,
  formatDate,
  formatResult,
  isNotable,
  reviewArrows,
  sideOf,
} from './review';

/** Moves after which the board shows nothing wrong. */
const SOUND = new Set(['brilliant', 'great', 'best', 'book']);

export function GamePage() {
  const { id = '' } = useParams();
  const [search] = useSearchParams();
  const player = search.get('player');
  const ply = Number.parseInt(search.get('ply') ?? '0', 10);
  const { view, error } = useReviewGame(id);

  if (error) {
    return (
      <Card title="Partie introuvable">
        <ErrorText>{error}</ErrorText>
        <BackLink player={player} />
      </Card>
    );
  }
  if (!view) return <p className="text-sm text-dim">Chargement…</p>;
  const start = Number.isNaN(ply) ? 0 : Math.max(0, Math.min(view.moves.length, ply));
  // Keyed by the position asked for: following another example resets the board.
  return (
    <GameReview
      key={`${view.game.id}-${String(start)}`}
      view={view}
      player={player}
      start={start}
    />
  );
}

function BackLink({ player }: { player: string | null }) {
  return (
    <Link
      to={player ? `/review?player=${encodeURIComponent(player)}` : '/review'}
      className="text-sm text-mute hover:text-bone"
    >
      ← {player ? `Parties de ${player}` : 'Parties Chess.com'}
    </Link>
  );
}

/** A player above or below the board: name, rating, accuracy, clock. */
function PlayerStrip({
  player,
  stats,
  clock,
  toMove,
}: {
  player: ReviewPlayer;
  stats: ReviewPlayerStats | null;
  clock: number | null;
  toMove: boolean;
}) {
  return (
    <div className="flex items-center gap-3 px-1">
      <span className="truncate font-medium">{player.username}</span>
      <span className="font-mono text-xs text-dim">{player.rating}</span>
      {stats?.accuracy != null && (
        <span className="font-mono text-xs text-mute" title="Précision">
          {stats.accuracy.toFixed(1)} %
        </span>
      )}
      <span className="flex-1" />
      {clock !== null && (
        <span
          className={`rounded-lg px-2 py-0.5 font-mono text-sm ${
            toMove ? 'bg-bone text-ink-950' : 'bg-ink-800 text-mute'
          }`}
        >
          {formatClock(clock)}
        </span>
      )}
    </div>
  );
}

/** Accuracy, ACPL and moves of each class, both players side by side. */
function Accuracies({ view }: { view: ReviewGameView }) {
  const analysis = view.analysis;
  if (!analysis) return null;
  const sides = [
    { name: view.game.white.username, stats: analysis.white },
    { name: view.game.black.username, stats: analysis.black },
  ];
  return (
    <Card title="Précision" aside={`${analysis.engine} · ${analysis.budget}`}>
      <div className="grid grid-cols-2 gap-4">
        {sides.map(({ name, stats }) => (
          <div key={name} className="min-w-0">
            <p className="truncate text-sm text-mute">{name}</p>
            <p className="font-display text-5xl">
              {stats.accuracy != null ? stats.accuracy.toFixed(1) : '—'}
            </p>
            <p className="font-mono text-xs text-dim">
              ACPL {stats.acpl != null ? Math.round(stats.acpl) : '—'}
            </p>
          </div>
        ))}
      </div>
      <table className="mt-5 w-full text-sm">
        <tbody>
          {CLASS_ORDER.map((kind) => (
            <tr key={kind} className="border-t border-line">
              <td className="py-1.5 font-mono text-bone">{analysis.white.counts[kind]}</td>
              <td className="py-1.5">
                <span className="flex items-center justify-center gap-2 text-mute">
                  <ClassBadge kind={kind} />
                  {CLASSES[kind].label}
                </span>
              </td>
              <td className="py-1.5 text-right font-mono text-bone">
                {analysis.black.counts[kind]}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

/** What the move just played was worth, and what was better. */
function Verdict({ view, shown }: { view: ReviewGameView; shown: number }) {
  const analysis = view.analysis;
  if (!analysis) {
    return (
      <Card title="Coup joué">
        <p className="text-sm text-dim">
          Partie pas encore analysée : lancez l'analyse des parties de ce joueur.
        </p>
      </Card>
    );
  }
  const history = view.moves.map((move) => move.san);
  const verdict = shown > 0 ? analysis.moves[shown - 1] : undefined;
  const before = analysis.positions[shown - 1];
  const best = before?.lines[0];
  if (!verdict) {
    return (
      <Card title="Coup joué">
        <p className="text-sm text-dim">Position de départ : avancez dans la partie.</p>
      </Card>
    );
  }
  const style = CLASSES[verdict.class];
  return (
    <Card title="Coup joué">
      <div className="flex items-center gap-3">
        <ClassBadge kind={verdict.class} large />
        <p className="text-lg">
          <span className="font-mono">{moveLabel(history, shown - 1, view.startFen)}</span>{' '}
          <span style={{ color: style.color }}>{style.label.toLowerCase()}</span>
        </p>
      </div>
      <div className="mt-4 grid grid-cols-3 gap-3">
        <Stat
          label="Chances"
          value={`${Math.round(verdict.winBefore)} → ${Math.round(verdict.winAfter)}`}
          unit="%"
          hint="Chances de gain du joueur avant (avec le meilleur coup) et après son coup"
        />
        <Stat label="Précision" value={verdict.accuracy.toFixed(0)} unit="%" />
        <Stat label="Perte" value={verdict.cpLoss} unit="cp" />
      </div>
      {best && !SOUND.has(verdict.class) && (
        <p className="mt-4 text-sm text-mute">
          <span className="mr-2 inline-block size-2 rounded-full bg-think" aria-hidden />
          Il fallait jouer{' '}
          <span className="font-mono text-bone">{frenchSan(best.san[0] ?? '')}</span>{' '}
          <span className="font-mono text-dim">({formatScore(best.score)})</span>
        </p>
      )}
    </Card>
  );
}

/** The engine's best lines in the position on the board. */
function BestLines({ view, shown }: { view: ReviewGameView; shown: number }) {
  const position = view.analysis?.positions[shown];
  if (!position) return null;
  return (
    <Card title="Meilleures lignes" aside={`profondeur ${position.lines[0]?.depth ?? '—'}`}>
      {position.lines.length === 0 ? (
        <p className="text-sm text-dim">
          {position.checkmated ? 'Échec et mat.' : 'Partie terminée.'}
        </p>
      ) : (
        <ol className="space-y-2">
          {position.lines.map((line, rank) => (
            <li key={rank} className="flex gap-3 text-sm">
              <span className="w-14 shrink-0 font-mono text-bone">{formatScore(line.score)}</span>
              <span className="min-w-0 truncate font-mono text-mute">
                {formatLine(line.san.slice(0, 8), position.fen)}
              </span>
            </li>
          ))}
        </ol>
      )}
    </Card>
  );
}

function GameReview({
  view,
  player,
  start,
}: {
  view: ReviewGameView;
  player: string | null;
  /** Moves shown at first. */
  start: number;
}) {
  const length = view.moves.length;
  const [shown, setShown] = useState(start);
  const [orientation, setOrientation] = useState<Side>(sideOf(view.game, player) ?? 'white');
  const request = useMemo(
    () => ({ ...newGame(view.startFen), moves: view.moves.slice(0, shown).map((m) => m.uci) }),
    [view, shown],
  );
  const position = usePosition(request);
  const analysis = view.analysis ?? null;

  // Arrow keys walk through the game.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const moves: Record<string, (current: number) => number> = {
        ArrowLeft: (current) => Math.max(0, current - 1),
        ArrowRight: (current) => Math.min(length, current + 1),
        Home: () => 0,
        End: () => length,
      };
      const move = moves[event.key];
      if (move) {
        event.preventDefault();
        setShown(move);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [length]);

  const history = view.moves.map((move) => move.san);
  const verdict = shown > 0 ? analysis?.moves[shown - 1] : undefined;
  const last = view.moves[shown - 1];
  // The squares of the move played take the color of its class.
  const squareTints: Record<string, CSSProperties> =
    verdict && last
      ? Object.fromEntries(
          [last.uci.slice(0, 2), last.uci.slice(2, 4)].map((square) => [
            square,
            { boxShadow: `inset 0 0 0 100vmax ${CLASSES[verdict.class].color}48` },
          ]),
        )
      : {};
  const boardView = position.view && verdict ? { ...position.view, lastMove: null } : position.view;
  const toMove = position.view?.sideToMove ?? 'white';
  const top: Side = orientation === 'white' ? 'black' : 'white';
  const strip = (side: Side) => (
    <PlayerStrip
      player={view.game[side]}
      stats={analysis?.[side] ?? null}
      clock={clockAt(view, shown, side)}
      toMove={toMove === side}
    />
  );
  const marks = (analysis?.moves ?? []).flatMap((move, ply) =>
    isNotable(move.class)
      ? [
          {
            index: ply + 1,
            color: CLASSES[move.class].color,
            title: `${moveLabel(history, ply, view.startFen) ?? ''} : ${CLASSES[move.class].label}`,
          },
        ]
      : [],
  );
  const opening = view.game.opening;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="min-w-0 space-y-1">
          <BackLink player={player} />
          <h1 className="text-2xl font-semibold tracking-tight">
            {view.game.white.username} – {view.game.black.username}{' '}
            <span className="font-mono text-mute">{formatResult(view.game.result)}</span>
          </h1>
          <p className="text-sm text-dim">
            {formatDate(view.game.endTime)} · {TIME_CLASS_NAME[view.game.timeClass]}{' '}
            {view.game.timeControl}
            {view.game.rated ? '' : ' · amicale'}
            {opening && (
              <>
                {' · '}
                <span
                  className="text-mute"
                  title={`Théorie jusqu'au demi-coup ${opening.bookPlies}`}
                >
                  {opening.eco} {opening.name}
                </span>
              </>
            )}
          </p>
        </div>
        <a
          href={view.game.url}
          target="_blank"
          rel="noreferrer"
          className="text-sm text-mute hover:text-bone"
        >
          Voir sur Chess.com ↗
        </a>
      </div>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_24rem]">
        <div className="min-w-0 space-y-4">
          <div className="mx-auto max-w-[46rem] space-y-2">
            {strip(top)}
            <div className="flex gap-3">
              <EvalBar
                score={analysis?.positions[shown]?.score ?? null}
                orientation={orientation}
              />
              <div className="min-w-0 flex-1">
                <Board
                  view={boardView}
                  orientation={orientation}
                  interactive={false}
                  onMove={() => undefined}
                  arrows={reviewArrows(view, shown)}
                  squareTints={squareTints}
                />
              </div>
            </div>
            {strip(orientation)}
          </div>

          <div className="mx-auto flex max-w-[46rem] flex-wrap items-center gap-3">
            <div className="flex rounded-xl border border-line bg-ink-900 p-1">
              {(
                [
                  ['Début', '⏮', () => 0, shown === 0],
                  ['Coup précédent', '‹', () => Math.max(0, shown - 1), shown === 0],
                  ['Coup suivant', '›', () => Math.min(length, shown + 1), shown >= length],
                  ['Fin', '⏭', () => length, shown >= length],
                ] as const
              ).map(([label, glyph, target, disabled]) => (
                <Button
                  key={label}
                  variant="ghost"
                  size="sm"
                  aria-label={label}
                  title={label}
                  className="min-w-8 text-base"
                  disabled={disabled}
                  onClick={() => {
                    setShown(target());
                  }}
                >
                  {glyph}
                </Button>
              ))}
            </div>
            <span className="font-mono text-xs text-dim">
              demi-coup {shown} / {length}
            </span>
            <span className="flex-1" />
            {analysis && (
              <span className="flex gap-3 text-xs text-dim">
                <span className="flex items-center gap-1.5">
                  <span className="size-2 rounded-full" style={{ background: BEST_ARROW }} />
                  meilleur coup ici
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="size-2 rounded-full" style={{ background: MISSED_ARROW }} />
                  il fallait jouer
                </span>
              </span>
            )}
            <Button
              onClick={() => {
                setOrientation(orientation === 'white' ? 'black' : 'white');
              }}
            >
              Retourner
            </Button>
          </div>

          {analysis && (
            <Card title="Chances de gain des Blancs" aside="cliquez pour y aller">
              <EvalGraph
                points={analysis.positions.map((p) => (p.win - 50) / 50)}
                cap={1}
                labels={false}
                current={shown}
                onSelect={setShown}
                marks={marks}
              />
            </Card>
          )}
        </div>

        <div className="space-y-4">
          <Verdict view={view} shown={shown} />
          <Card title="Coups">
            <MoveList
              history={history}
              startFen={view.startFen}
              current={shown}
              onSelect={setShown}
              maxHeight="max-h-96"
              decorate={(ply) => {
                const kind = analysis?.moves[ply]?.class;
                return kind && isNotable(kind) ? <ClassBadge kind={kind} /> : null;
              }}
            />
          </Card>
          <BestLines view={view} shown={shown} />
          <Accuracies view={view} />
        </div>
      </div>
    </div>
  );
}

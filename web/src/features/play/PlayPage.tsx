import { useEffect, useState } from 'react';

import { api } from '../../api/client';
import type { EngineLimits } from '../../api/generated/EngineLimits';
import type { EngineMoveResponse } from '../../api/generated/EngineMoveResponse';
import type { GameRequest } from '../../api/generated/GameRequest';
import type { Side } from '../../api/generated/Side';
import { usePosition } from '../../api/hooks';
import { Board } from '../../components/Board';
import { GameIO } from '../../components/GameIO';
import { MoveList } from '../../components/MoveList';
import { Button, ErrorText, Panel, Segmented } from '../../components/ui';
import { newGame, withMove, withoutLast } from '../../lib/game';
import { formatScore } from '../../lib/score';
import { SIDE_NAME, describeStatus, resultOf } from '../../lib/status';
import {
  DEFAULT_LEVEL,
  DEPTH_RANGE,
  type Level,
  TIME_RANGE,
  levelLabel,
  levelLimits,
} from './level';

type ColorChoice = Side | 'random';

const COLOR_OPTIONS = [
  { value: 'white', label: 'Blancs' },
  { value: 'black', label: 'Noirs' },
  { value: 'random', label: 'Aléatoire' },
] as const;

const MODE_OPTIONS = [
  { value: 'depth', label: 'Profondeur' },
  { value: 'time', label: 'Temps' },
] as const;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'erreur inconnue';
}

export function PlayPage() {
  const [colorChoice, setColorChoice] = useState<ColorChoice>('white');
  const [level, setLevel] = useState<Level>(DEFAULT_LEVEL);
  const [player, setPlayer] = useState<Side>('white');
  const [flipped, setFlipped] = useState(false);
  const [game, setGame] = useState<GameRequest>(newGame());
  const [engine, setEngine] = useState<EngineMoveResponse | null>(null);
  const [engineError, setEngineError] = useState<string | null>(null);

  const { view, error, loading } = usePosition(game);
  const gameKey = JSON.stringify(game);
  const limitsKey = JSON.stringify(levelLimits(level));
  const ongoing = view?.status.kind === 'ongoing';
  const engineTurn = !loading && !!view && ongoing && view.sideToMove !== player;

  // When it is the bot's turn, ask the server for its move. A move that
  // arrives after the game changed (undo, new game) is discarded.
  useEffect(() => {
    if (!engineTurn) return;
    const controller = new AbortController();
    const request = {
      game: JSON.parse(gameKey) as GameRequest,
      limits: JSON.parse(limitsKey) as EngineLimits,
    };
    api
      .engineMove(request, controller.signal)
      .then((response) => {
        setEngine(response);
        setEngineError(null);
        const best = response.bestMove;
        if (best) {
          setGame((current) =>
            JSON.stringify(current) === gameKey ? withMove(current, best.uci) : current,
          );
        }
      })
      .catch((err: unknown) => {
        if (!controller.signal.aborted) setEngineError(errorMessage(err));
      });
    return () => {
      controller.abort();
    };
  }, [engineTurn, gameKey, limitsKey]);

  const startNewGame = () => {
    const side = colorChoice === 'random' ? (Math.random() < 0.5 ? 'white' : 'black') : colorChoice;
    setPlayer(side);
    setFlipped(false);
    setEngine(null);
    setGame(newGame());
  };

  // On the player's turn take back the bot's reply too, so the player moves again.
  const undo = () => {
    setGame((current) => withoutLast(current, engineTurn ? 1 : 2));
  };

  const orientation: Side = flipped ? (player === 'white' ? 'black' : 'white') : player;
  const interactive = !!view && !loading && ongoing && view.sideToMove === player;

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <div className="mx-auto w-full max-w-160">
        <Board
          view={view}
          orientation={orientation}
          interactive={interactive}
          onMove={(uci) => {
            setGame((current) => withMove(current, uci));
          }}
        />
      </div>

      <div className="space-y-4">
        <Panel title="Partie">
          <p className="text-base font-medium">
            {view ? describeStatus(view.status, view.sideToMove, !!view.checkSquare) : '…'}
          </p>
          <p className="mt-1 text-sm text-slate-400">
            Vous jouez les {SIDE_NAME[player]} contre le bot ({levelLabel(level)}).
          </p>
          {engineTurn && <p className="mt-2 text-sm text-amber-300">Le bot réfléchit…</p>}
          {view && !ongoing && (
            <p className="mt-2 font-mono text-sm text-slate-300">
              Résultat : {resultOf(view.status)}
            </p>
          )}
          {error && <ErrorText>{error}</ErrorText>}
          {engineError && <ErrorText>Bot : {engineError}</ErrorText>}
          <div className="mt-4 flex flex-wrap gap-2">
            <Button variant="primary" onClick={startNewGame}>
              Nouvelle partie
            </Button>
            <Button onClick={undo} disabled={game.moves.length === 0}>
              Annuler le coup
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setFlipped((value) => !value);
              }}
            >
              Retourner
            </Button>
          </div>
        </Panel>

        <Panel title="Réglages">
          <div className="space-y-3 text-sm">
            <div className="flex items-center justify-between gap-2">
              <span className="text-slate-400">Couleur</span>
              <Segmented value={colorChoice} options={COLOR_OPTIONS} onChange={setColorChoice} />
            </div>
            <p className="text-xs text-slate-500">La couleur s’applique à la prochaine partie.</p>
            <div className="flex items-center justify-between gap-2">
              <span className="text-slate-400">Niveau</span>
              <Segmented
                value={level.mode}
                options={MODE_OPTIONS}
                onChange={(mode) => {
                  setLevel((current) => ({ ...current, mode }));
                }}
              />
            </div>
            {level.mode === 'depth' ? (
              <input
                type="range"
                aria-label="Profondeur"
                min={DEPTH_RANGE.min}
                max={DEPTH_RANGE.max}
                value={level.depth}
                onChange={(event) => {
                  setLevel((current) => ({ ...current, depth: Number(event.target.value) }));
                }}
                className="w-full accent-emerald-500"
              />
            ) : (
              <input
                type="range"
                aria-label="Temps par coup"
                min={TIME_RANGE.min}
                max={TIME_RANGE.max}
                step={TIME_RANGE.step}
                value={level.timeMs}
                onChange={(event) => {
                  setLevel((current) => ({ ...current, timeMs: Number(event.target.value) }));
                }}
                className="w-full accent-emerald-500"
              />
            )}
            <p className="text-right text-xs text-slate-400">{levelLabel(level)}</p>
          </div>
        </Panel>

        <Panel title="Coups">
          <MoveList history={view?.history ?? []} startFen={game.fen ?? undefined} />
        </Panel>

        {engine && (
          <Panel title="Dernier coup du bot">
            <dl className="grid grid-cols-2 gap-1 text-sm">
              <dt className="text-slate-400">Évaluation</dt>
              <dd className="font-mono">{formatScore(engine.score)}</dd>
              <dt className="text-slate-400">Profondeur</dt>
              <dd className="font-mono">{engine.depth}</dd>
              <dt className="text-slate-400">Nœuds</dt>
              <dd className="font-mono">{engine.nodes.toLocaleString('fr-FR')}</dd>
              <dt className="text-slate-400">Temps</dt>
              <dd className="font-mono">{engine.timeMs} ms</dd>
            </dl>
            {engine.pv.length > 0 && (
              <p className="mt-2 font-mono text-xs text-slate-300">{engine.pv.join(' ')}</p>
            )}
          </Panel>
        )}

        <Panel title="Import / export">
          <GameIO
            game={game}
            fen={view?.fen ?? null}
            tags={[
              { name: 'White', value: player === 'white' ? 'Joueur' : 'chess-bot' },
              { name: 'Black', value: player === 'black' ? 'Joueur' : 'chess-bot' },
            ]}
            onLoad={(loaded) => {
              setEngine(null);
              setGame(loaded);
            }}
          />
        </Panel>
      </div>
    </div>
  );
}

import { useEffect, useState } from 'react';
import type { Arrow } from 'react-chessboard';

import { api } from '../../api/client';
import type { EngineInfo } from '../../api/generated/EngineInfo';
import type { EngineLimits } from '../../api/generated/EngineLimits';
import type { EngineMoveResponse } from '../../api/generated/EngineMoveResponse';
import type { GameRequest } from '../../api/generated/GameRequest';
import type { MoveView } from '../../api/generated/MoveView';
import type { Side } from '../../api/generated/Side';
import { useEngineInfo, usePosition } from '../../api/hooks';
import { Board } from '../../components/Board';
import { EvalBar } from '../../components/EvalBar';
import { EvalGraph } from '../../components/EvalGraph';
import { GameIO } from '../../components/GameIO';
import { MoveList } from '../../components/MoveList';
import {
  Button,
  Card,
  Dot,
  ErrorText,
  Eyebrow,
  Modal,
  Segmented,
  Stat,
  Toggle,
} from '../../components/ui';
import { fenMoveNumber, formatLine, newGame, withMove, withoutLast } from '../../lib/game';
import { clampedPawns, formatScore } from '../../lib/score';
import { SIDE_NAME, describeStatus, resultOf } from '../../lib/status';
import { formatClock } from './clock';
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

const HINT_COLOR = 'rgba(200, 245, 96, 0.9)';
/** Bound of the evaluation sparkline, in pawns. */
const GRAPH_CAP = 3;

/** Thinking time of each side: the bot's from its answers, the player's measured here. */
type Clock = { bot: number; player: number; since: number | null };

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'erreur inconnue';
}

/** `main` for `/data/weights/main.nnue`. */
function networkName(info: EngineInfo | null): string {
  const file = info?.weightsFile;
  if (info?.evaluator !== 'nnue' || !file) return 'PeSTO';
  return (file.split(/[\\/]/).pop() ?? file).replace(/\.nnue$/, '');
}

function BotCard({
  info,
  side,
  thinking,
  clock,
  forecast,
}: {
  info: EngineInfo | null;
  side: Side;
  thinking: boolean;
  clock: number;
  forecast: string | null;
}) {
  const nnue = info?.evaluator === 'nnue';
  return (
    <Card highlight={thinking}>
      <div className="flex items-start gap-3">
        <span
          aria-hidden
          className="flex size-11 shrink-0 items-center justify-center rounded-xl border border-signal/30 bg-signal/10 text-xl text-signal"
        >
          ⌬
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate font-semibold">{networkName(info)}</p>
          <p className="truncate text-xs text-dim">
            {nnue ? 'NNUE · réseau entraîné' : 'Évaluation classique'}
          </p>
        </div>
        <span className="rounded-lg border border-line px-2 py-0.5 text-xs text-mute">
          {SIDE_NAME[side]}
        </span>
      </div>
      <div className="mt-5 flex items-baseline justify-between">
        <span className={`font-mono text-4xl ${thinking ? 'text-bone' : 'text-dim'}`}>
          {formatClock(clock, true)}
        </span>
        <span className="text-xs text-dim">réflexion</span>
      </div>
      <p
        className={`mt-4 flex items-center gap-2 rounded-xl border border-dashed px-3 py-2 text-xs ${
          thinking ? 'border-think/40 text-think' : 'border-line text-mute'
        }`}
      >
        <span aria-hidden className={thinking ? 'animate-pulse' : ''}>
          ▮▮▮
        </span>
        {thinking
          ? 'Réfléchit…'
          : forecast
            ? `Attend votre coup · prévoit ${forecast}`
            : 'Attend votre coup'}
      </p>
    </Card>
  );
}

function InferenceCard({
  engine,
  fen,
  playerToMove,
}: {
  engine: EngineMoveResponse | null;
  fen: string | null;
  playerToMove: boolean;
}) {
  const [showStats, setShowStats] = useState(true);
  const [showLine, setShowLine] = useState(true);
  const [hideUntilMove, setHideUntilMove] = useState(false);
  const hidden = hideUntilMove && playerToMove;
  const lastMove = engine?.bestMove?.san;
  const kns = engine && engine.timeMs > 0 ? engine.nodes / engine.timeMs : null;

  return (
    <Card
      title={<span className="text-think">Inférence du bot</span>}
      aside={lastMove ? `dernier : ${lastMove}` : undefined}
    >
      <div className="space-y-3">
        <Toggle checked={showStats} onChange={setShowStats}>
          Stats de recherche
        </Toggle>
        <Toggle checked={showLine} onChange={setShowLine}>
          Suite prévue (PV)
        </Toggle>
        <Toggle checked={hideUntilMove} onChange={setHideUntilMove}>
          Masquer jusqu’à mon coup
        </Toggle>
      </div>
      {hidden ? (
        <p className="mt-5 text-sm text-dim">Masqué jusqu’à votre coup.</p>
      ) : !engine ? (
        <p className="mt-5 text-sm text-dim">Le bot n’a pas encore joué.</p>
      ) : (
        <>
          {showStats && (
            <div className="mt-5 grid grid-cols-2 overflow-hidden rounded-xl border border-line">
              <div className="border-r border-b border-line p-3">
                <Stat label="Profondeur" value={engine.depth} />
              </div>
              <div className="border-b border-line p-3">
                <Stat label="Nœuds" value={engine.nodes.toLocaleString('fr-FR')} />
              </div>
              <div className="border-r border-line p-3">
                <Stat
                  label="Vitesse"
                  value={kns === null ? '—' : Math.round(kns).toLocaleString('fr-FR')}
                  unit="kN/s"
                />
              </div>
              <div className="p-3">
                <Stat label="Temps" value={engine.timeMs.toLocaleString('fr-FR')} unit="ms" />
              </div>
            </div>
          )}
          {showLine && fen && engine.pv.length > 1 && (
            <div className="mt-4">
              <p className="mb-2 text-xs text-dim">Suite prévue</p>
              <p className="rounded-xl border border-line bg-ink-950 px-3 py-2 font-mono text-xs leading-relaxed text-mute">
                {formatLine(engine.pv.slice(1), fen)}
              </p>
            </div>
          )}
        </>
      )}
    </Card>
  );
}

function PlayerCard({ side, toMove, clock }: { side: Side; toMove: boolean; clock: number }) {
  return (
    <Card highlight={toMove}>
      <div className="flex items-center gap-3">
        <span
          aria-hidden
          className="flex size-11 items-center justify-center rounded-xl bg-bone text-2xl text-ink-950"
        >
          {side === 'white' ? '♔' : '♚'}
        </span>
        <div className="flex-1">
          <p className="font-semibold">Vous</p>
          <p className="text-xs text-dim">{SIDE_NAME[side]}</p>
        </div>
        {toMove && (
          <span className="flex items-center gap-2 text-xs text-signal">
            <Dot tone="signal" />À vous
          </span>
        )}
      </div>
      <p className={`mt-5 font-mono text-4xl ${toMove ? 'text-signal' : 'text-dim'}`}>
        {formatClock(clock)}
      </p>
    </Card>
  );
}

function DepthPips({ value, onChange }: { value: number; onChange: (depth: number) => void }) {
  const depths = Array.from(
    { length: DEPTH_RANGE.max - DEPTH_RANGE.min + 1 },
    (_, i) => DEPTH_RANGE.min + i,
  );
  return (
    <div role="radiogroup" aria-label="Profondeur" className="flex gap-1">
      {depths.map((depth) => (
        <button
          key={depth}
          type="button"
          role="radio"
          aria-checked={depth === value}
          aria-label={`Profondeur ${depth}`}
          title={`Profondeur ${depth}`}
          onClick={() => {
            onChange(depth);
          }}
          className={`h-4 flex-1 rounded-[3px] transition-colors ${
            depth <= value ? 'bg-signal hover:bg-[#d6ff7a]' : 'bg-ink-700 hover:bg-ink-600'
          }`}
        />
      ))}
    </div>
  );
}

export function PlayPage() {
  const [colorChoice, setColorChoice] = useState<ColorChoice>('white');
  const [level, setLevel] = useState<Level>(DEFAULT_LEVEL);
  const [player, setPlayer] = useState<Side>('white');
  const [flipped, setFlipped] = useState(false);
  const [tilted, setTilted] = useState(true);
  const [game, setGame] = useState<GameRequest>(newGame());
  const [engine, setEngine] = useState<EngineMoveResponse | null>(null);
  const [engineError, setEngineError] = useState<string | null>(null);
  // The bot's evaluation after each of its moves, by number of moves played before it.
  const [evals, setEvals] = useState<Record<number, number>>({});
  const [hint, setHint] = useState<{ fen: string; move: MoveView } | null>(null);
  const [hintBusy, setHintBusy] = useState(false);
  const [io, setIo] = useState(false);
  const [clock, setClock] = useState<Clock>(() => ({ bot: 0, player: 0, since: Date.now() }));
  const [now, setNow] = useState(() => Date.now());
  const engineInfo = useEngineInfo();

  const { view, error, loading } = usePosition(game);
  const gameKey = JSON.stringify(game);
  const limitsKey = JSON.stringify(levelLimits(level));
  const ongoing = view?.status.kind === 'ongoing';
  const engineTurn = !loading && !!view && ongoing && view.sideToMove !== player;
  const interactive = !!view && !loading && ongoing && view.sideToMove === player;

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
        setEvals((current) => ({
          ...current,
          [request.game.moves.length]: clampedPawns(response.score, GRAPH_CAP),
        }));
        setClock((current) => ({
          ...current,
          bot: current.bot + response.timeMs,
          since: Date.now(),
        }));
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

  // The player's clock ticks while it is their turn.
  useEffect(() => {
    if (!interactive) return;
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 250);
    return () => {
      clearInterval(timer);
    };
  }, [interactive]);

  const restartClock = () => {
    const start = Date.now();
    setNow(start);
    setClock({ bot: 0, player: 0, since: start });
  };

  const startNewGame = () => {
    const side = colorChoice === 'random' ? (Math.random() < 0.5 ? 'white' : 'black') : colorChoice;
    setPlayer(side);
    setFlipped(false);
    setEngine(null);
    setEvals({});
    restartClock();
    setGame(newGame());
  };

  const playMove = (uci: string) => {
    const end = Date.now();
    setClock((current) => ({
      ...current,
      player: current.player + (current.since === null ? 0 : end - current.since),
      since: null,
    }));
    setGame((current) => withMove(current, uci));
  };

  // On the player's turn take back the bot's reply too, so the player moves again.
  const undo = () => {
    setGame((current) => withoutLast(current, engineTurn ? 1 : 2));
    setClock((current) => ({ ...current, since: current.since ?? Date.now() }));
  };

  const askHint = async () => {
    if (!view) return;
    setHintBusy(true);
    try {
      const response = await api.engineMove({ game, limits: levelLimits(level) });
      if (response.bestMove) setHint({ fen: view.fen, move: response.bestMove });
    } catch (err: unknown) {
      setEngineError(errorMessage(err));
    } finally {
      setHintBusy(false);
    }
  };

  const orientation: Side = flipped ? (player === 'white' ? 'black' : 'white') : player;
  const botSide: Side = player === 'white' ? 'black' : 'white';
  const shownHint = hint && view && hint.fen === view.fen ? hint.move : null;
  const arrows: Arrow[] = shownHint
    ? [{ startSquare: shownHint.from, endSquare: shownHint.to, color: HINT_COLOR }]
    : [];
  const playerClock =
    clock.player + (interactive && clock.since !== null ? Math.max(0, now - clock.since) : 0);
  const forecast =
    engine && view && interactive && engine.pv.length > 1
      ? formatLine(engine.pv.slice(1, 3), view.fen)
      : null;
  const moveNumber = view ? fenMoveNumber(view.fen).number : 1;
  const plies = view?.history.length ?? 0;
  const graph = Array.from({ length: plies + 1 }, (_, ply) => evals[ply] ?? null);

  return (
    <div className="grid gap-6 xl:grid-cols-[17.5rem_minmax(0,1fr)_20rem]">
      {/* Center first on small screens: the board is what matters. */}
      <div className="min-w-0 xl:order-2">
        <div className="mx-auto max-w-[44rem]">
          <div className="flex items-end justify-between gap-4">
            <div className="min-w-0">
              <Eyebrow>Partie · coup {moveNumber}</Eyebrow>
              <h1 className="mt-1.5 truncate text-2xl font-medium tracking-tight">
                {view ? describeStatus(view.status, view.sideToMove, !!view.checkSquare) : '…'}
              </h1>
            </div>
            <div className="flex items-baseline gap-2">
              <span className="text-xs text-signal">bot</span>
              <span className="font-display text-6xl">
                {engine ? formatScore(engine.score) : '—'}
              </span>
            </div>
          </div>
          <div className="mt-4">
            <EvalBar score={engine?.score ?? null} orientation={orientation} horizontal />
          </div>
          <div className="mt-2 min-h-5 text-sm">
            {view && !ongoing && (
              <p className="font-mono text-signal">Résultat : {resultOf(view.status)}</p>
            )}
            {error && <ErrorText>{error}</ErrorText>}
            {engineError && <ErrorText>Bot : {engineError}</ErrorText>}
          </div>

          <div className="mt-2">
            <Board
              view={view}
              orientation={orientation}
              interactive={interactive}
              onMove={playMove}
              arrows={arrows}
              tilted={tilted}
            />
          </div>

          <div className="mt-6 flex justify-center">
            <div className="flex flex-wrap items-center justify-center gap-1 rounded-2xl border border-line bg-ink-900 p-1.5">
              <Button variant="primary" onClick={startNewGame}>
                <span aria-hidden>＋</span> Nouvelle partie
              </Button>
              <Button variant="ghost" onClick={undo} disabled={game.moves.length === 0}>
                <span aria-hidden>↶</span> Annuler
              </Button>
              <Button
                variant="ghost"
                onClick={() => {
                  setFlipped((value) => !value);
                }}
              >
                <span aria-hidden>⇅</span> Retourner
              </Button>
              <Button
                variant="ghost"
                onClick={() => void askHint()}
                disabled={!interactive || hintBusy}
              >
                <span aria-hidden>✦</span> {hintBusy ? 'Recherche…' : 'Indice'}
              </Button>
              <Button
                variant="ghost"
                aria-pressed={tilted}
                onClick={() => {
                  setTilted((value) => !value);
                }}
                title={tilted ? 'Plateau à plat (glisser-déposer)' : 'Plateau en perspective'}
              >
                {tilted ? '2D' : '3D'}
              </Button>
              <span aria-hidden className="mx-1 h-6 w-px bg-line" />
              <Button
                className="font-mono text-xs"
                onClick={() => {
                  setIo(true);
                }}
              >
                FEN / PGN
              </Button>
            </div>
          </div>
          {tilted && (
            <p className="mt-3 text-center text-xs text-dim">
              En perspective, jouez en cliquant la pièce puis sa case.
            </p>
          )}
        </div>
      </div>

      <div className="space-y-4 xl:order-1">
        <BotCard
          info={engineInfo.view}
          side={botSide}
          thinking={engineTurn}
          clock={clock.bot}
          forecast={forecast}
        />
        <InferenceCard engine={engine} fen={view?.fen ?? null} playerToMove={interactive} />
        <PlayerCard side={player} toMove={interactive} clock={playerClock} />
      </div>

      <div className="space-y-4 xl:order-3">
        <Card title="Coups" aside="éval du bot">
          {plies > 0 && (
            <div className="mb-4 rounded-xl bg-ink-950 px-2 py-1">
              <EvalGraph points={graph} cap={GRAPH_CAP} height={48} labels={false} />
            </div>
          )}
          <MoveList history={view?.history ?? []} startFen={game.fen ?? undefined} />
        </Card>

        <Card title="Réglages">
          <div className="space-y-4 text-sm">
            <div>
              <Segmented
                label="Couleur"
                value={colorChoice}
                options={COLOR_OPTIONS}
                onChange={setColorChoice}
                stretch
              />
              <p className="mt-1.5 text-xs text-dim">S’applique à la prochaine partie.</p>
            </div>
            <Segmented
              label="Niveau"
              value={level.mode}
              options={MODE_OPTIONS}
              onChange={(mode) => {
                setLevel((current) => ({ ...current, mode }));
              }}
              stretch
            />
            {level.mode === 'depth' ? (
              <DepthPips
                value={level.depth}
                onChange={(depth) => {
                  setLevel((current) => ({ ...current, depth }));
                }}
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
                className="w-full accent-signal"
              />
            )}
            <p className="flex justify-between font-mono text-xs text-dim">
              <span>{level.mode === 'depth' ? DEPTH_RANGE.min : '0,1 s'}</span>
              <span className="text-bone">{levelLabel(level)}</span>
              <span>{level.mode === 'depth' ? DEPTH_RANGE.max : '10 s'}</span>
            </p>
          </div>
        </Card>
      </div>

      <Modal
        open={io}
        onClose={() => {
          setIo(false);
        }}
        title="Import / export"
      >
        <GameIO
          game={game}
          fen={view?.fen ?? null}
          tags={[
            { name: 'White', value: player === 'white' ? 'Joueur' : 'chess-bot' },
            { name: 'Black', value: player === 'black' ? 'Joueur' : 'chess-bot' },
          ]}
          onLoad={(loaded) => {
            setEngine(null);
            setEvals({});
            restartClock();
            setGame(loaded);
            setIo(false);
          }}
        />
      </Modal>
    </div>
  );
}

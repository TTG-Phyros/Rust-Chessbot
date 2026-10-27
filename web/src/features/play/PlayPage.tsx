import { useEffect, useRef, useState } from 'react';
import type { Arrow } from 'react-chessboard';
import { useSearchParams } from 'react-router';

import { api } from '../../api/client';
import type { BotInfo } from '../../api/generated/BotInfo';
import type { EngineInfo } from '../../api/generated/EngineInfo';
import type { EngineLimits } from '../../api/generated/EngineLimits';
import type { EngineMoveResponse } from '../../api/generated/EngineMoveResponse';
import type { EvalExplanation } from '../../api/generated/EvalExplanation';
import type { GameRequest } from '../../api/generated/GameRequest';
import type { PvLineView } from '../../api/generated/PvLineView';
import type { Side } from '../../api/generated/Side';
import type { StockfishResult } from '../../api/generated/StockfishResult';
import { useBots, useEngineInfo, usePosition } from '../../api/hooks';
import { Board } from '../../components/Board';
import { EvalBar } from '../../components/EvalBar';
import { EvalGraph } from '../../components/EvalGraph';
import { GameIO } from '../../components/GameIO';
import { MoveList } from '../../components/MoveList';
import { Button, Card, ErrorText, Eyebrow, INPUT, Modal, Segmented } from '../../components/ui';
import { fenMoveNumber, formatLine, newGame, withMove, withoutLast } from '../../lib/game';
import { clampedPawns, formatScore } from '../../lib/score';
import { SIDE_NAME, describeStatus, resultOf } from '../../lib/status';
import { formatElo } from '../training/format';
import {
  type Clocks,
  afterMove,
  formatClock,
  resumeClocks,
  startClocks,
  stopClocks,
  timeLeft,
  timeUsed,
} from './clock';
import { LEVEL_COLORS, compare } from './compare';
import {
  CADENCES,
  DEFAULT_LEVEL,
  DEPTH_RANGE,
  type Level,
  TIME_RANGE,
  cadenceLabel,
  levelLabel,
  levelLimits,
} from './level';
import {
  type Budget,
  BotCard,
  type InferenceSettings,
  InferenceCard,
  PlayerCard,
  StockfishCard,
} from './panels';

type ColorChoice = Side | 'random';

const COLOR_OPTIONS = [
  { value: 'white', label: 'Blancs' },
  { value: 'black', label: 'Noirs' },
  { value: 'random', label: 'Aléatoire' },
] as const;

const MODE_OPTIONS = [
  { value: 'depth', label: 'Profondeur' },
  { value: 'time', label: 'Temps' },
  { value: 'clock', label: 'Pendule' },
] as const;

const HINT_COLOR = 'rgba(200, 245, 96, 0.9)';
const FORECAST_COLOR = 'rgba(157, 140, 255, 0.75)';
/** Bound of the evaluation sparkline, in pawns. */
const GRAPH_CAP = 3;
/** Lines searched for the player's candidate moves. */
const CANDIDATES = 3;
/** A clock under this is shown in the alert color. */
const LOW_CLOCK_MS = 20_000;

const other = (side: Side): Side => (side === 'white' ? 'black' : 'white');

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'erreur inconnue';
}

/** Side to move after the moves of `game` (from its FEN's side). */
function sideToMoveOf(game: GameRequest): Side {
  const start = game.fen ? fenMoveNumber(game.fen).side : 'white';
  return game.moves.length % 2 === 0 ? start : other(start);
}

/** `main` for `/data/weights/main.nnue`. */
function networkName(info: EngineInfo | null): string {
  const file = info?.weightsFile;
  if (info?.evaluator !== 'nnue' || !file) return 'PeSTO';
  return (file.split(/[\\/]/).pop() ?? file).replace(/\.nnue$/, '');
}

/** Name, description and Elo of the bot playing. */
function describeBot(bot: string | null, saved: BotInfo | undefined, info: EngineInfo | null) {
  if (bot === 'classic') {
    return { name: 'PeSTO', subtitle: 'Évaluation classique · référence', elo: '0 Elo' };
  }
  if (saved) {
    const snapshot = saved.superbatch != null ? `sb ${saved.superbatch}` : 'dernière version';
    return {
      name: saved.name,
      subtitle: `NNUE · ${saved.network} · ${snapshot}`,
      elo: saved.elo != null ? formatElo(saved.elo, saved.eloError ?? 0) : null,
    };
  }
  return {
    name: networkName(info),
    subtitle: info?.evaluator === 'nnue' ? 'NNUE · réseau en service' : 'Évaluation classique',
    elo: null,
  };
}

/** Limits of the searches made for the player (candidates, comparison): quick. */
function helperLimits(level: Level): EngineLimits {
  switch (level.mode) {
    case 'depth':
      return levelLimits(level);
    case 'time':
      return { movetimeMs: Math.min(level.timeMs, 1000) };
    case 'clock':
      return { movetimeMs: 300 };
  }
}

/** A result tied to the request it answers (a game, as JSON, and options). */
type Keyed<T> = { key: string; value: T };

export function PlayPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const bot = searchParams.get('bot');
  const bots = useBots();
  const engineInfo = useEngineInfo();
  const [colorChoice, setColorChoice] = useState<ColorChoice>('white');
  const [level, setLevel] = useState<Level>(DEFAULT_LEVEL);
  const [player, setPlayer] = useState<Side>('white');
  const [flipped, setFlipped] = useState(false);
  const [tilted, setTilted] = useState(true);
  const [game, setGame] = useState<GameRequest>(newGame());
  const [engine, setEngine] = useState<EngineMoveResponse | null>(null);
  const [engineError, setEngineError] = useState<string | null>(null);
  // Evaluations for the sparkline, by number of moves played: the bot's,
  // and each Stockfish level's.
  const [evals, setEvals] = useState<Record<number, number>>({});
  const [sfEvals, setSfEvals] = useState<Record<number, Record<number, number>>>({});
  const [hintFor, setHintFor] = useState<string | null>(null);
  const [io, setIo] = useState(false);
  const [clocks, setClocks] = useState<Clocks>(() => startClocks(Date.now(), 'white'));
  const [now, setNow] = useState(() => Date.now());
  const [flagged, setFlagged] = useState<Side | null>(null);
  const [settings, setSettings] = useState<InferenceSettings>({
    stats: true,
    candidates: true,
    nnue: true,
    hideUntilMove: false,
  });
  const [candidates, setCandidates] = useState<Keyed<PvLineView[]> | null>(null);
  const [explanation, setExplanation] = useState<Keyed<EvalExplanation> | null>(null);
  const [sfEnabled, setSfEnabled] = useState(true);
  const [sfLevels, setSfLevels] = useState<number[]>([7, 10]);
  const [sfBudget, setSfBudget] = useState<Budget>('native');
  const [comparison, setComparison] = useState<Keyed<StockfishResult[]> | null>(null);
  const [sfError, setSfError] = useState<Keyed<string> | null>(null);

  const { view, error, loading } = usePosition(game);
  const gameKey = JSON.stringify(game);
  const clockMode = level.mode === 'clock';
  const baseMs = level.cadence.minutes * 60_000;
  const incrementMs = level.cadence.incrementS * 1000;
  const ongoing = view?.status.kind === 'ongoing' && flagged === null;
  const engineTurn = !loading && !!view && ongoing && view.sideToMove !== player;
  const interactive = !!view && !loading && ongoing && view.sideToMove === player;
  const botSide = other(player);

  // The bot reads the clocks when it is asked to move, without being asked
  // again each time they tick.
  const clocksRef = useRef(clocks);
  useEffect(() => {
    clocksRef.current = clocks;
  }, [clocks]);
  const limitsKey = JSON.stringify(clockMode ? { cadence: level.cadence } : levelLimits(level));

  // When it is the bot's turn, ask the server for its move. A move that
  // arrives after the game changed (undo, new game) is discarded.
  useEffect(() => {
    if (!engineTurn) return;
    const controller = new AbortController();
    const requested = JSON.parse(gameKey) as GameRequest;
    const asked = JSON.parse(limitsKey) as { cadence?: Level['cadence'] } & EngineLimits;
    let limits: EngineLimits = asked;
    const increment = asked.cadence ? asked.cadence.incrementS * 1000 : 0;
    if (asked.cadence) {
      const at = Date.now();
      const base = asked.cadence.minutes * 60_000;
      limits = {
        whiteTimeMs: Math.max(1, Math.round(timeLeft(clocksRef.current, 'white', at, base))),
        blackTimeMs: Math.max(1, Math.round(timeLeft(clocksRef.current, 'black', at, base))),
        whiteIncrementMs: increment,
        blackIncrementMs: increment,
      };
    }
    api
      .engineMove({ game: requested, limits, bot }, controller.signal)
      .then((response) => {
        setEngine(response);
        setEngineError(null);
        setEvals((current) => ({
          ...current,
          [requested.moves.length]: clampedPawns(response.score, GRAPH_CAP),
        }));
        const best = response.bestMove;
        if (best) {
          const side = sideToMoveOf(requested);
          setClocks((current) => afterMove(current, side, Date.now(), increment));
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
  }, [engineTurn, gameKey, limitsKey, bot]);

  // On the player's turn: their best moves (candidates, hint, the bot's
  // side of the comparison).
  const helperKey = JSON.stringify(helperLimits(level));
  useEffect(() => {
    if (!interactive) return;
    const controller = new AbortController();
    const requested = JSON.parse(gameKey) as GameRequest;
    const limits = { ...(JSON.parse(helperKey) as EngineLimits), multipv: CANDIDATES };
    api
      .engineMove({ game: requested, limits, bot }, controller.signal)
      .then((response) => {
        setCandidates({ key: gameKey, value: response.lines });
        setEvals((current) => ({
          ...current,
          [requested.moves.length]: clampedPawns(response.score, GRAPH_CAP),
        }));
      })
      .catch(() => {
        // Only a help: the game goes on without it.
      });
    return () => {
      controller.abort();
    };
  }, [interactive, gameKey, helperKey, bot]);

  // The raw evaluation of each position, taken apart.
  const positionShown = view !== null;
  useEffect(() => {
    if (!settings.nnue || !positionShown) return;
    const controller = new AbortController();
    api
      .explain({ game: JSON.parse(gameKey) as GameRequest, bot }, controller.signal)
      .then((value) => {
        setExplanation({ key: gameKey, value });
      })
      .catch(() => {
        // Only a help.
      });
    return () => {
      controller.abort();
    };
  }, [settings.nnue, positionShown, gameKey, bot]);

  // Stockfish's view of the player's position.
  const stockfish = engineInfo.view?.stockfish ?? null;
  const sfKey = JSON.stringify({ game: gameKey, levels: sfLevels, budget: sfBudget, helperKey });
  useEffect(() => {
    if (!sfEnabled || !stockfish || !interactive) return;
    const { levels, budget } = JSON.parse(sfKey) as { levels: number[]; budget: Budget };
    if (levels.length === 0) return;
    const controller = new AbortController();
    const requested = JSON.parse(gameKey) as GameRequest;
    api
      .stockfishCompare(
        {
          game: requested,
          levels,
          budget: budget === 'same' ? (JSON.parse(helperKey) as EngineLimits) : null,
        },
        controller.signal,
      )
      .then((response) => {
        setComparison({ key: sfKey, value: response.results });
        setSfError(null);
        setSfEvals((current) => {
          const next = { ...current };
          for (const result of response.results) {
            if (result.score) {
              next[result.level] = {
                ...next[result.level],
                [requested.moves.length]: clampedPawns(result.score, GRAPH_CAP),
              };
            }
          }
          return next;
        });
      })
      .catch((err: unknown) => {
        if (!controller.signal.aborted) setSfError({ key: sfKey, value: errorMessage(err) });
      });
    return () => {
      controller.abort();
    };
  }, [sfEnabled, stockfish, interactive, sfKey, gameKey, helperKey]);

  // The clock of the side to move ticks; with a time control, it can fall.
  const ticking = clocks.running !== null && ongoing;
  useEffect(() => {
    if (!ticking) return;
    const timer = setInterval(() => {
      const at = Date.now();
      setNow(at);
      if (!clockMode) return;
      const running = clocksRef.current.running;
      if (running && timeLeft(clocksRef.current, running, at, baseMs) <= 0) {
        setFlagged(running);
        setClocks((current) => stopClocks(current, at));
      }
    }, 200);
    return () => {
      clearInterval(timer);
    };
  }, [ticking, clockMode, baseMs]);

  const restart = (loaded: GameRequest) => {
    const start = Date.now();
    setNow(start);
    setClocks(startClocks(start, sideToMoveOf(loaded)));
    setFlagged(null);
    setEngine(null);
    setEvals({});
    setSfEvals({});
    setGame(loaded);
  };

  const startNewGame = () => {
    const side = colorChoice === 'random' ? (Math.random() < 0.5 ? 'white' : 'black') : colorChoice;
    setPlayer(side);
    setFlipped(false);
    restart(newGame());
  };

  const playMove = (uci: string) => {
    setClocks((current) => afterMove(current, player, Date.now(), clockMode ? incrementMs : 0));
    setGame((current) => withMove(current, uci));
  };

  // On the player's turn take back the bot's reply too, so the player moves again.
  const undo = () => {
    setGame((current) => withoutLast(current, engineTurn ? 1 : 2));
    setClocks((current) => resumeClocks(current, player, Date.now()));
  };

  const orientation: Side = flipped ? botSide : player;
  const lines = candidates?.key === gameKey ? candidates.value : null;
  const explained = explanation?.key === gameKey ? explanation.value : null;
  const results = comparison?.key === sfKey ? comparison.value : null;
  const hidden = settings.hideUntilMove && interactive;
  const best = lines?.[0] ?? null;
  const comparisonView =
    sfEnabled && results ? compare(best?.pv[0] ?? null, best?.score ?? null, results) : null;

  const arrows: Arrow[] = [];
  const anticipated = engine?.lines[0]?.pv[1];
  if (interactive && !hidden && anticipated) {
    arrows.push({
      startSquare: anticipated.slice(0, 2),
      endSquare: anticipated.slice(2, 4),
      color: FORECAST_COLOR,
    });
  }
  if (interactive && !hidden && comparisonView) {
    for (const row of comparisonView.rows) {
      const color = LEVEL_COLORS[sfLevels.indexOf(row.level)];
      if (row.uci && color) {
        arrows.push({ startSquare: row.uci.slice(0, 2), endSquare: row.uci.slice(2, 4), color });
      }
    }
  }
  if (hintFor === gameKey && best?.pv[0]) {
    arrows.push({
      startSquare: best.pv[0].slice(0, 2),
      endSquare: best.pv[0].slice(2, 4),
      color: HINT_COLOR,
    });
  }

  const clockText = (side: Side, precise: boolean) => {
    if (!clockMode) return formatClock(timeUsed(clocks, side, now), precise);
    const left = timeLeft(clocks, side, now, baseMs);
    return formatClock(left, left < 10_000);
  };
  const low = (side: Side) => clockMode && timeLeft(clocks, side, now, baseMs) < LOW_CLOCK_MS;
  const forecast =
    engine && view && interactive && engine.pv.length > 1
      ? formatLine(engine.pv.slice(1, 3), view.fen)
      : null;
  const moveNumber = view ? fenMoveNumber(view.fen).number : 1;
  const plies = view?.history.length ?? 0;
  const graph = Array.from({ length: plies + 1 }, (_, ply) => evals[ply] ?? null);
  const overlays = sfEnabled
    ? sfLevels.flatMap((sfLevel, i) => {
        const color = LEVEL_COLORS[i];
        const series = sfEvals[sfLevel];
        return color && series
          ? [{ color, points: Array.from({ length: plies + 1 }, (_, ply) => series[ply] ?? null) }]
          : [];
      })
    : [];
  const shownScore = best?.score ?? engine?.score ?? null;
  const saved = bots.view?.find((b) => b.name === bot);
  const described = describeBot(bot, saved, engineInfo.view);
  const result = flagged
    ? flagged === 'white'
      ? '0-1'
      : '1-0'
    : view && view.status.kind !== 'ongoing'
      ? resultOf(view.status)
      : null;

  return (
    <div className="grid gap-6 xl:grid-cols-[17.5rem_minmax(0,1fr)_20rem]">
      {/* Center first on small screens: the board is what matters. */}
      <div className="min-w-0 xl:order-2">
        <div className="mx-auto max-w-[44rem]">
          <div className="flex items-end justify-between gap-4">
            <div className="min-w-0">
              <Eyebrow>Partie · coup {moveNumber}</Eyebrow>
              <h1 className="mt-1.5 truncate text-2xl font-medium tracking-tight">
                {flagged
                  ? `Temps écoulé — les ${SIDE_NAME[other(flagged)]} gagnent`
                  : view
                    ? describeStatus(view.status, view.sideToMove, !!view.checkSquare)
                    : '…'}
              </h1>
            </div>
            <div className="text-right">
              <div className="flex items-baseline justify-end gap-2">
                <span className="text-xs text-signal">bot</span>
                <span className="font-display text-6xl">
                  {shownScore && !hidden ? formatScore(shownScore) : '—'}
                </span>
              </div>
              {comparisonView && !hidden && (
                <div className="mt-1 flex justify-end gap-1.5">
                  {comparisonView.rows.map((row) => (
                    <span
                      key={row.level}
                      className="flex items-center gap-1.5 rounded-md border border-line px-2 py-0.5 font-mono text-[11px]"
                    >
                      <span
                        className="size-2 rounded-sm"
                        style={{ background: LEVEL_COLORS[sfLevels.indexOf(row.level)] }}
                      />
                      SF {row.level} {row.score ? formatScore(row.score) : '—'}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div className="mt-4">
            <EvalBar score={hidden ? null : shownScore} orientation={orientation} horizontal />
          </div>
          <div className="mt-2 min-h-5 text-sm">
            {result && <p className="font-mono text-signal">Résultat : {result}</p>}
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
                aria-pressed={hintFor === gameKey}
                onClick={() => {
                  setHintFor(hintFor === gameKey ? null : gameKey);
                }}
                disabled={!interactive || !best}
              >
                <span aria-hidden>✦</span> {interactive && !best ? 'Recherche…' : 'Indice'}
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
          name={described.name}
          subtitle={described.subtitle}
          elo={described.elo}
          side={botSide}
          thinking={engineTurn}
          clock={clockText(botSide, true)}
          low={low(botSide)}
          forecast={hidden ? null : forecast}
        />
        <InferenceCard
          settings={settings}
          onSettings={setSettings}
          engine={engine}
          candidates={lines}
          explanation={explained}
          fen={view?.fen ?? null}
          player={player}
          playerToMove={interactive}
        />
        <PlayerCard
          side={player}
          toMove={interactive}
          clock={clockText(player, false)}
          low={low(player)}
        />
      </div>

      <div className="space-y-4 xl:order-3">
        <StockfishCard
          version={stockfish}
          enabled={sfEnabled}
          onEnabled={setSfEnabled}
          levels={sfLevels}
          onLevels={setSfLevels}
          budget={sfBudget}
          onBudget={setSfBudget}
          bot={best}
          comparison={hidden ? null : comparisonView}
          busy={interactive && sfEnabled && !results && sfError?.key !== sfKey}
          error={sfError?.key === sfKey ? sfError.value : null}
        />

        <Card title="Coups" aside={sfEnabled && stockfish ? 'bot vs Stockfish' : 'éval du bot'}>
          {plies > 0 && (
            <div className="mb-4 rounded-xl bg-ink-950 px-2 py-1">
              <EvalGraph
                points={graph}
                overlays={overlays}
                cap={GRAPH_CAP}
                height={48}
                labels={false}
              />
            </div>
          )}
          <MoveList history={view?.history ?? []} startFen={game.fen ?? undefined} />
        </Card>

        <Card title="Réglages">
          <div className="space-y-4 text-sm">
            <label className="block">
              <span className="text-xs text-dim">Adversaire</span>
              <select
                value={bot ?? ''}
                onChange={(event) => {
                  const name = event.target.value;
                  setSearchParams(name ? { bot: name } : {});
                }}
                className={`${INPUT} mt-1.5 font-sans`}
              >
                <option value="">Réseau en service ({networkName(engineInfo.view)})</option>
                <option value="classic">PeSTO (évaluation classique)</option>
                {bots.view?.map((savedBot) => (
                  <option key={savedBot.name} value={savedBot.name}>
                    {savedBot.name}
                    {savedBot.elo != null
                      ? ` · ${formatElo(savedBot.elo, savedBot.eloError ?? 0)}`
                      : ''}
                  </option>
                ))}
              </select>
            </label>
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
            {level.mode === 'depth' && (
              <div role="radiogroup" aria-label="Profondeur" className="flex gap-1">
                {Array.from(
                  { length: DEPTH_RANGE.max - DEPTH_RANGE.min + 1 },
                  (_, i) => DEPTH_RANGE.min + i,
                ).map((depth) => (
                  <button
                    key={depth}
                    type="button"
                    role="radio"
                    aria-checked={depth === level.depth}
                    aria-label={`Profondeur ${depth}`}
                    title={`Profondeur ${depth}`}
                    onClick={() => {
                      setLevel((current) => ({ ...current, depth }));
                    }}
                    className={`h-4 flex-1 rounded-[3px] transition-colors ${
                      depth <= level.depth
                        ? 'bg-signal hover:bg-[#d6ff7a]'
                        : 'bg-ink-700 hover:bg-ink-600'
                    }`}
                  />
                ))}
              </div>
            )}
            {level.mode === 'time' && (
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
            {level.mode === 'clock' && (
              <div className="grid grid-cols-3 gap-1.5">
                {CADENCES.map((cadence) => {
                  const on =
                    cadence.minutes === level.cadence.minutes &&
                    cadence.incrementS === level.cadence.incrementS;
                  return (
                    <button
                      key={cadenceLabel(cadence)}
                      type="button"
                      aria-pressed={on}
                      onClick={() => {
                        setLevel((current) => ({ ...current, cadence }));
                      }}
                      className={`h-9 rounded-lg border font-mono text-sm transition-colors ${
                        on
                          ? 'border-signal/60 bg-signal/10 text-signal'
                          : 'border-line text-mute hover:border-ink-600'
                      }`}
                    >
                      {cadenceLabel(cadence)}
                    </button>
                  );
                })}
              </div>
            )}
            {level.mode === 'clock' ? (
              <p className="text-xs text-dim">
                <span className="font-mono text-bone">{levelLabel(level)}</span> : le bot gère son
                temps comme en partie réelle. « Nouvelle partie » remet les pendules à zéro.
              </p>
            ) : (
              <p className="flex justify-between font-mono text-xs text-dim">
                <span>{level.mode === 'depth' ? DEPTH_RANGE.min : '0,1 s'}</span>
                <span className="text-bone">{levelLabel(level)}</span>
                <span>{level.mode === 'depth' ? DEPTH_RANGE.max : '10 s'}</span>
              </p>
            )}
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
            { name: 'White', value: player === 'white' ? 'Joueur' : described.name },
            { name: 'Black', value: player === 'black' ? 'Joueur' : described.name },
          ]}
          onLoad={(loaded) => {
            restart(loaded);
            setIo(false);
          }}
        />
      </Modal>
    </div>
  );
}

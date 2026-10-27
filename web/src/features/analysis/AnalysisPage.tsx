import { useEffect, useMemo, useState } from 'react';
import type { Arrow } from 'react-chessboard';

import type { AnalysisInfo } from '../../api/generated/AnalysisInfo';
import type { GameRequest } from '../../api/generated/GameRequest';
import type { Side } from '../../api/generated/Side';
import { useAnalysis, usePosition } from '../../api/hooks';
import { Board } from '../../components/Board';
import { EvalBar } from '../../components/EvalBar';
import { EvalGraph } from '../../components/EvalGraph';
import { GameIO } from '../../components/GameIO';
import { MoveList } from '../../components/MoveList';
import { Button, Card, Dot, ErrorText, Modal, Segmented, Stat } from '../../components/ui';
import { formatLine, moveLabel, newGame } from '../../lib/game';
import { clampedPawns, formatScore } from '../../lib/score';
import { SIDE_NAME, describeStatus } from '../../lib/status';

const BEST_MOVE_COLOR = 'rgba(200, 245, 96, 0.9)';
const REPLY_COLOR = 'rgba(157, 140, 255, 0.85)';
/** Bound of the evaluation graph, in pawns. */
const GRAPH_CAP = 3;

type ArrowMode = 'pv' | 'none';

const ARROW_OPTIONS = [
  { value: 'pv', label: 'Flèches PV' },
  { value: 'none', label: 'Sans flèches' },
] as const;

/** Bars of the nodes searched at each completed depth, on a log scale. */
function NodesPerDepth({ depths }: { depths: readonly AnalysisInfo[] }) {
  if (depths.length === 0) return <p className="text-sm text-dim">Pas encore de profondeur.</p>;
  const max = Math.log10(Math.max(...depths.map((info) => info.nodes), 10));
  const first = depths[0];
  const last = depths[depths.length - 1];
  return (
    <>
      <div className="flex h-24 items-end gap-1">
        {depths.map((info) => (
          <div
            key={info.depth}
            title={`Profondeur ${info.depth} : ${info.nodes.toLocaleString('fr-FR')} nœuds`}
            className={`flex-1 rounded-t-[3px] ${info === last ? 'bg-signal' : 'bg-ink-600'}`}
            style={{ height: `${Math.max(4, (Math.log10(Math.max(info.nodes, 1)) / max) * 100)}%` }}
          />
        ))}
      </div>
      <div className="mt-2 flex justify-between font-mono text-[10px] text-dim">
        <span>p{first?.depth}</span>
        <span>log</span>
        <span>p{last?.depth}</span>
      </div>
    </>
  );
}

export function AnalysisPage() {
  // The whole line, and how many of its moves are on the board.
  const [line, setLine] = useState<GameRequest>(newGame());
  const [shown, setShown] = useState(0);
  const [orientation, setOrientation] = useState<Side>('white');
  const [enabled, setEnabled] = useState(true);
  const [arrowMode, setArrowMode] = useState<ArrowMode>('pv');
  const [io, setIo] = useState(false);
  // Latest evaluation of each position analysed, by game (JSON).
  const [evals, setEvals] = useState<Record<string, number>>({});
  const [recorded, setRecorded] = useState<AnalysisInfo | null>(null);

  const game = useMemo(() => ({ ...line, moves: line.moves.slice(0, shown) }), [line, shown]);
  const { view, error, loading } = usePosition(game);
  const fullLine = usePosition(line);
  const analysis = useAnalysis();
  const { analyze, stop } = analysis;
  const gameKey = JSON.stringify(game);
  const status = view?.status.kind;

  // Re-analyse whenever the position on the board changes.
  useEffect(() => {
    if (!enabled || analysis.socket !== 'open' || loading || !status) return;
    if (status === 'ongoing') {
      analyze(JSON.parse(gameKey) as GameRequest);
    } else {
      stop();
    }
  }, [enabled, analysis.socket, loading, status, gameKey, analyze, stop]);

  // Keep each new depth's evaluation for the graph, under the game it belongs to.
  const info = analysis.info;
  if (info !== recorded) {
    setRecorded(info);
    if (info && analysis.game) {
      const key = JSON.stringify(analysis.game);
      setEvals((current) => ({ ...current, [key]: clampedPawns(info.score, GRAPH_CAP) }));
    }
  }

  // Arrow keys walk through the line.
  const length = line.moves.length;
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
        return;
      }
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

  // A move on the board continues the line, or replaces what followed.
  const playMove = (uci: string) => {
    setLine((current) => {
      if (current.moves[shown] === uci) return current;
      return { ...current, moves: [...current.moves.slice(0, shown), uci] };
    });
    setShown(shown + 1);
  };

  const load = (loaded: GameRequest) => {
    setLine(loaded);
    setShown(loaded.moves.length);
  };

  // The analysis on screen describes the position on the board (not a previous one).
  const current = info && status === 'ongoing' && JSON.stringify(analysis.game) === gameKey;
  const live = current ? info : null;
  const arrows: Arrow[] = [];
  if (enabled && live && arrowMode === 'pv') {
    const [best, reply] = live.pv;
    if (reply) {
      arrows.push({
        startSquare: reply.slice(0, 2),
        endSquare: reply.slice(2, 4),
        color: REPLY_COLOR,
      });
    }
    if (best) {
      arrows.push({
        startSquare: best.slice(0, 2),
        endSquare: best.slice(2, 4),
        color: BEST_MOVE_COLOR,
      });
    }
  }

  const history = fullLine.view?.history ?? [];
  const startFen = line.fen ?? undefined;
  const graph = Array.from(
    { length: length + 1 },
    (_, ply) => evals[JSON.stringify({ ...line, moves: line.moves.slice(0, ply) })] ?? null,
  );
  const tableShare = live ? live.hashfull / 10 : 0;

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_24rem]">
      <div className="min-w-0 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <Segmented
            label="Flèches"
            value={arrowMode}
            options={ARROW_OPTIONS}
            onChange={setArrowMode}
          />
          {view && (
            <button
              type="button"
              title="Copier la FEN"
              onClick={() => void navigator.clipboard.writeText(view.fen)}
              className="max-w-full truncate font-mono text-xs text-dim hover:text-bone"
            >
              {view.fen}
            </button>
          )}
        </div>

        <div className="mx-auto flex max-w-[46rem] gap-3">
          <EvalBar score={live?.score ?? null} orientation={orientation} />
          <div className="min-w-0 flex-1">
            <Board
              view={view}
              orientation={orientation}
              interactive={!loading}
              onMove={playMove}
              arrows={arrows}
            />
          </div>
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
            {shown > 0 && (
              <span className="text-bone">{moveLabel(history, shown - 1, startFen)} · </span>
            )}
            demi-coup {shown} / {length}
          </span>
          <span className="flex-1" />
          <Button
            onClick={() => {
              setOrientation(orientation === 'white' ? 'black' : 'white');
            }}
          >
            Retourner
          </Button>
          <Button
            onClick={() => {
              setIo(true);
            }}
          >
            Importer PGN / FEN
          </Button>
        </div>

        <Card title="Évaluation au fil de la partie" aside="cliquez pour y aller">
          {length === 0 ? (
            <p className="text-sm text-dim">
              Jouez des coups ou importez une partie : chaque position analysée ajoute son point.
            </p>
          ) : (
            <EvalGraph points={graph} cap={GRAPH_CAP} current={shown} onSelect={setShown} />
          )}
        </Card>
      </div>

      <div className="space-y-4">
        <Card>
          <div className="flex items-center justify-between gap-3">
            <p className="flex items-center gap-2 text-sm">
              <Dot tone={enabled && analysis.running ? 'signal' : 'idle'} />
              {view && status === 'ongoing'
                ? `Analyse en direct · trait aux ${SIDE_NAME[view.sideToMove]}`
                : view
                  ? describeStatus(view.status, view.sideToMove, !!view.checkSquare)
                  : '…'}
            </p>
            <Button
              size="sm"
              variant={enabled ? 'secondary' : 'primary'}
              onClick={() => {
                if (enabled) stop();
                setEnabled(!enabled);
              }}
            >
              {enabled ? '■ Arrêter' : '▶ Analyser'}
            </Button>
          </div>
          {analysis.socket !== 'open' && (
            <p className="mt-2 text-sm text-warn">Connexion au serveur…</p>
          )}
          <div className="mt-5 flex items-end justify-between gap-3">
            <span className="font-display text-7xl">{live ? formatScore(live.score) : '—'}</span>
            <div className="text-right">
              <p className="font-mono text-2xl">
                {live ? live.depth : '—'}
                <span className="text-base text-dim"> / {live ? live.seldepth : '—'}</span>
              </p>
              <p className="text-xs text-dim">profondeur / sélective</p>
            </div>
          </div>
          <div className="mt-4 flex h-3 gap-[3px]" aria-hidden>
            {Array.from({ length: Math.max(live?.seldepth ?? 0, 24) }, (_, i) => (
              <span
                key={i}
                className={`flex-1 rounded-[2px] ${
                  live && i < live.depth ? 'bg-signal' : 'bg-ink-700'
                }`}
              />
            ))}
          </div>
          <div className="mt-5 grid grid-cols-2 overflow-hidden rounded-xl border border-line">
            <div className="border-r border-b border-line p-3">
              <Stat label="Nœuds" value={live ? live.nodes.toLocaleString('fr-FR') : '—'} />
            </div>
            <div className="border-b border-line p-3">
              <Stat
                label="Vitesse"
                value={live ? Math.round(live.nps / 1000).toLocaleString('fr-FR') : '—'}
                unit="kN/s"
              />
            </div>
            <div className="border-r border-line p-3">
              <Stat label="Temps" value={live ? (live.timeMs / 1000).toFixed(1) : '—'} unit="s" />
            </div>
            <div className="p-3">
              <Stat
                label="Table de transposition"
                value={live ? tableShare.toFixed(1) : '—'}
                unit="%"
              />
              <div className="mt-2 h-1 rounded-full bg-ink-700">
                <div
                  className="h-full rounded-full bg-think"
                  style={{ width: `${Math.min(100, tableShare)}%` }}
                />
              </div>
            </div>
          </div>
          {analysis.error && <ErrorText>{analysis.error}</ErrorText>}
          {error && <ErrorText>{error}</ErrorText>}
        </Card>

        <Card title="Ligne principale">
          {live && view ? (
            <div className="flex items-start gap-3 rounded-xl border border-signal/25 bg-signal/[0.04] p-3">
              <span className="rounded-md bg-signal px-2 py-0.5 font-mono text-xs font-medium text-ink-950">
                {formatScore(live.score)}
              </span>
              <p className="font-mono text-xs leading-relaxed text-mute">
                {formatLine(live.pvSan, view.fen)}
              </p>
            </div>
          ) : (
            <p className="text-sm text-dim">
              {enabled && status === 'ongoing' ? 'Calcul en cours…' : 'Analyse arrêtée.'}
            </p>
          )}
        </Card>

        <Card title="Nœuds par profondeur">
          <NodesPerDepth depths={current ? analysis.depths : []} />
        </Card>

        <Card
          title="Coups"
          aside={
            <button
              type="button"
              className="hover:text-bone"
              onClick={() => {
                load(newGame());
              }}
            >
              position initiale
            </button>
          }
        >
          <MoveList history={history} startFen={startFen} current={shown} onSelect={setShown} />
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
          onLoad={(loaded) => {
            load(loaded);
            setIo(false);
          }}
        />
      </Modal>
    </div>
  );
}

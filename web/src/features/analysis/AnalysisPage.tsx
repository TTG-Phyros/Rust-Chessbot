import { useEffect, useMemo, useState } from 'react';

import type { GameRequest } from '../../api/generated/GameRequest';
import type { Side } from '../../api/generated/Side';
import { useAnalysis, usePosition } from '../../api/hooks';
import { Board } from '../../components/Board';
import { EvalBar } from '../../components/EvalBar';
import { GameIO } from '../../components/GameIO';
import { MoveList } from '../../components/MoveList';
import { Button, ErrorText, Panel } from '../../components/ui';
import { formatLine, newGame } from '../../lib/game';
import { formatScore } from '../../lib/score';
import { describeStatus } from '../../lib/status';

const BEST_MOVE_COLOR = 'rgba(16, 185, 129, 0.85)';

export function AnalysisPage() {
  // The whole line, and how many of its moves are on the board.
  const [line, setLine] = useState<GameRequest>(newGame());
  const [shown, setShown] = useState(0);
  const [orientation, setOrientation] = useState<Side>('white');
  const [enabled, setEnabled] = useState(true);

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

  const info = analysis.info;
  const best = info?.pv[0];
  const arrows =
    enabled && best && status === 'ongoing'
      ? [{ startSquare: best.slice(0, 2), endSquare: best.slice(2, 4), color: BEST_MOVE_COLOR }]
      : [];

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <div className="mx-auto w-full max-w-170">
        {/* The bar only spans the board, not the navigation buttons below. */}
        <div className="flex gap-3">
          <EvalBar
            score={status === 'ongoing' ? (info?.score ?? null) : null}
            orientation={orientation}
          />
          <div className="flex-1">
            <Board
              view={view}
              orientation={orientation}
              interactive={!loading}
              onMove={playMove}
              arrows={arrows}
            />
          </div>
        </div>
        <div>
          <div className="mt-3 flex flex-wrap justify-center gap-2">
            <Button
              aria-label="Début"
              onClick={() => {
                setShown(0);
              }}
              disabled={shown === 0}
            >
              ⏮
            </Button>
            <Button
              aria-label="Coup précédent"
              onClick={() => {
                setShown(Math.max(0, shown - 1));
              }}
              disabled={shown === 0}
            >
              ◀
            </Button>
            <Button
              aria-label="Coup suivant"
              onClick={() => {
                setShown(Math.min(length, shown + 1));
              }}
              disabled={shown >= length}
            >
              ▶
            </Button>
            <Button
              aria-label="Fin"
              onClick={() => {
                setShown(length);
              }}
              disabled={shown >= length}
            >
              ⏭
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setOrientation(orientation === 'white' ? 'black' : 'white');
              }}
            >
              Retourner
            </Button>
          </div>
        </div>
      </div>

      <div className="space-y-4">
        <Panel title="Analyse">
          <div className="flex items-center justify-between">
            <p className="text-sm">
              {view ? describeStatus(view.status, view.sideToMove, !!view.checkSquare) : '…'}
            </p>
            <Button
              variant={enabled ? 'secondary' : 'primary'}
              onClick={() => {
                if (enabled) stop();
                setEnabled(!enabled);
              }}
            >
              {enabled ? 'Arrêter' : 'Analyser'}
            </Button>
          </div>
          {analysis.socket !== 'open' && (
            <p className="mt-2 text-sm text-amber-300">Connexion au serveur…</p>
          )}
          {info && status === 'ongoing' ? (
            <>
              <p className="mt-3 font-mono text-2xl font-semibold">{formatScore(info.score)}</p>
              <dl className="mt-2 grid grid-cols-2 gap-1 text-sm">
                <dt className="text-slate-400">Profondeur</dt>
                <dd className="font-mono">
                  {info.depth}/{info.seldepth}
                </dd>
                <dt className="text-slate-400">Nœuds</dt>
                <dd className="font-mono">{info.nodes.toLocaleString('fr-FR')}</dd>
                <dt className="text-slate-400">Vitesse</dt>
                <dd className="font-mono">
                  {Math.round(info.nps / 1000).toLocaleString('fr-FR')} kN/s
                </dd>
                <dt className="text-slate-400">Temps</dt>
                <dd className="font-mono">{(info.timeMs / 1000).toFixed(1)} s</dd>
                <dt className="text-slate-400">Table</dt>
                <dd className="font-mono">{(info.hashfull / 10).toFixed(1)} %</dd>
              </dl>
              {view && (
                <p className="mt-3 font-mono text-xs leading-relaxed text-slate-300">
                  {formatLine(info.pvSan, view.fen)}
                </p>
              )}
            </>
          ) : (
            enabled &&
            status === 'ongoing' && <p className="mt-3 text-sm text-slate-400">Calcul en cours…</p>
          )}
          {analysis.error && <ErrorText>{analysis.error}</ErrorText>}
          {error && <ErrorText>{error}</ErrorText>}
        </Panel>

        <Panel title="Coups">
          <MoveList
            history={fullLine.view?.history ?? []}
            startFen={line.fen ?? undefined}
            current={shown}
            onSelect={setShown}
          />
          <div className="mt-3 flex gap-2">
            <Button
              variant="ghost"
              onClick={() => {
                load(newGame());
              }}
            >
              Position initiale
            </Button>
          </div>
        </Panel>

        <Panel title="Import / export">
          <GameIO game={game} fen={view?.fen ?? null} onLoad={load} />
        </Panel>
      </div>
    </div>
  );
}

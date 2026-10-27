/** The cards around the Play board. */
import type { EngineMoveResponse } from '../../api/generated/EngineMoveResponse';
import type { EvalExplanation } from '../../api/generated/EvalExplanation';
import type { PvLineView } from '../../api/generated/PvLineView';
import type { Side } from '../../api/generated/Side';
import { Card, Dot, Segmented, Stat, Toggle } from '../../components/ui';
import { formatLine } from '../../lib/game';
import { frenchSan } from '../../lib/san';
import { clampedPawns, formatScore } from '../../lib/score';
import { SIDE_NAME } from '../../lib/status';
import { type Comparison, LEVEL_COLORS, MAX_COMPARED, formatDelta } from './compare';

/** What the inference card shows. */
export type InferenceSettings = {
  stats: boolean;
  candidates: boolean;
  nnue: boolean;
  /** Hide what helps the player while it is their turn. */
  hideUntilMove: boolean;
};

export type Budget = 'native' | 'same';

const BUDGET_OPTIONS = [
  { value: 'native', label: 'Niveau natif' },
  { value: 'same', label: 'Même budget que le bot' },
] as const;

export function BotCard({
  name,
  subtitle,
  elo,
  side,
  thinking,
  clock,
  low,
  forecast,
}: {
  name: string;
  subtitle: string;
  /** `+303 ± 121`, when the bot was measured. */
  elo: string | null;
  side: Side;
  thinking: boolean;
  clock: string;
  /** The clock is nearly out. */
  low: boolean;
  forecast: string | null;
}) {
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
          <p className="truncate font-semibold">{name}</p>
          <p className="truncate text-xs text-dim">{subtitle}</p>
        </div>
        <span className="rounded-lg border border-line px-2 py-0.5 text-xs text-mute">
          {SIDE_NAME[side]}
        </span>
      </div>
      <div className="mt-5 flex items-baseline justify-between gap-2">
        <span
          className={`font-mono text-4xl ${low ? 'text-alert' : thinking ? 'text-bone' : 'text-dim'}`}
        >
          {clock}
        </span>
        {elo && <span className="font-mono text-xs text-dim">{elo}</span>}
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

/** Win / draw / loss bar, White's share first. */
function WdlBar({ explanation, searched }: { explanation: EvalExplanation; searched: string }) {
  const { win, draw, loss } = explanation.wdl;
  const parts = [
    { share: win, label: 'gain', className: 'bg-bone text-ink-950' },
    { share: draw, label: 'nulle', className: 'bg-ink-600 text-bone' },
    { share: loss, label: 'perte', className: 'bg-ink-950 text-bone' },
  ];
  return (
    <div className="mt-4">
      <p className="mb-2 text-xs text-dim">
        Sortie {explanation.evaluator === 'nnue' ? 'NNUE' : 'PeSTO'} brute · gain / nulle / perte
        (Blancs)
      </p>
      <div className="flex h-6 overflow-hidden rounded-lg border border-line font-mono text-[10px]">
        {parts.map((part) => (
          <span
            key={part.label}
            title={`${part.label} : ${(part.share / 10).toFixed(1)} %`}
            className={`flex items-center px-1.5 ${part.className}`}
            style={{ width: `${part.share / 10}%` }}
          >
            {part.share >= 80 ? Math.round(part.share / 10) : ''}
          </span>
        ))}
      </div>
      <p className="mt-2 font-mono text-xs text-dim">
        statique {explanation.staticCp > 0 ? '+' : ''}
        {explanation.staticCp} cp → recherche {searched}
      </p>
    </div>
  );
}

/** The best moves for the player, as bars from their point of view. */
function Candidates({
  lines,
  fen,
  player,
}: {
  lines: readonly PvLineView[];
  fen: string;
  player: Side;
}) {
  const sign = player === 'white' ? 1 : -1;
  const best = lines[0] ? clampedPawns(lines[0].score, 10) * sign : 0;
  return (
    <div className="mt-4">
      <p className="mb-2 text-xs text-dim">Coups candidats pour vous</p>
      <ul className="space-y-1.5">
        {lines.map((line, rank) => {
          const pawns = clampedPawns(line.score, 10) * sign;
          const width = Math.max(8, Math.min(100, 100 - (best - pawns) * 60));
          return (
            <li
              key={line.pv[0] ?? rank}
              className="grid grid-cols-[3.5rem_1fr_3.5rem] items-center gap-2"
            >
              <span className="font-mono text-sm">{frenchSan(line.pvSan[0] ?? '')}</span>
              <span className="h-1.5 rounded-full bg-ink-700">
                <span
                  className={`block h-full rounded-full ${rank === 0 ? 'bg-signal' : 'bg-ink-600'}`}
                  style={{ width: `${width}%` }}
                />
              </span>
              <span className="text-right font-mono text-xs text-mute">
                {formatScore(line.score)}
              </span>
            </li>
          );
        })}
      </ul>
      {lines[0] && lines[0].pvSan.length > 1 && (
        <p className="mt-3 rounded-xl border border-line bg-ink-950 px-3 py-2 font-mono text-xs leading-relaxed text-mute">
          PV : {formatLine(lines[0].pvSan, fen)}
        </p>
      )}
    </div>
  );
}

export function InferenceCard({
  settings,
  onSettings,
  engine,
  candidates,
  explanation,
  fen,
  player,
  playerToMove,
}: {
  settings: InferenceSettings;
  onSettings: (settings: InferenceSettings) => void;
  /** The bot's last move. */
  engine: EngineMoveResponse | null;
  /** The best lines for the player in the current position. */
  candidates: readonly PvLineView[] | null;
  explanation: EvalExplanation | null;
  fen: string | null;
  player: Side;
  playerToMove: boolean;
}) {
  const hidden = settings.hideUntilMove && playerToMove;
  const toggle = (key: keyof InferenceSettings) => (checked: boolean) => {
    onSettings({ ...settings, [key]: checked });
  };
  const kns = engine && engine.timeMs > 0 ? engine.nodes / engine.timeMs : null;
  const searched = candidates?.[0]?.score ?? engine?.score;

  return (
    <Card
      title={<span className="text-think">Inférence du bot</span>}
      aside={engine?.bestMove ? `dernier : ${frenchSan(engine.bestMove.san)}` : undefined}
    >
      <div className="space-y-3">
        <Toggle checked={settings.stats} onChange={toggle('stats')}>
          Stats de recherche
        </Toggle>
        <Toggle checked={settings.candidates} onChange={toggle('candidates')}>
          Coups candidats et PV
        </Toggle>
        <Toggle checked={settings.nnue} onChange={toggle('nnue')}>
          Sortie NNUE (G / N / P)
        </Toggle>
        <Toggle checked={settings.hideUntilMove} onChange={toggle('hideUntilMove')}>
          Masquer jusqu’à mon coup
        </Toggle>
      </div>
      {settings.stats && engine && (
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
      {hidden ? (
        <p className="mt-5 text-sm text-dim">
          Candidats et sortie NNUE masqués jusqu’à votre coup.
        </p>
      ) : (
        <>
          {settings.candidates && fen && candidates && candidates.length > 0 && (
            <Candidates lines={candidates} fen={fen} player={player} />
          )}
          {settings.nnue && explanation && (
            <WdlBar explanation={explanation} searched={searched ? formatScore(searched) : '—'} />
          )}
        </>
      )}
      {!engine && !candidates && (
        <p className="mt-5 text-sm text-dim">Le bot n’a pas encore joué.</p>
      )}
    </Card>
  );
}

export function PlayerCard({
  side,
  toMove,
  clock,
  low,
}: {
  side: Side;
  toMove: boolean;
  clock: string;
  low: boolean;
}) {
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
      <p
        className={`mt-5 font-mono text-4xl ${low ? 'text-alert' : toMove ? 'text-signal' : 'text-dim'}`}
      >
        {clock}
      </p>
    </Card>
  );
}

export function StockfishCard({
  version,
  enabled,
  onEnabled,
  levels,
  onLevels,
  budget,
  onBudget,
  bot,
  comparison,
  busy,
  error,
}: {
  /** Stockfish's `id name`, or `null` when the server has none. */
  version: string | null;
  enabled: boolean;
  onEnabled: (enabled: boolean) => void;
  levels: readonly number[];
  onLevels: (levels: number[]) => void;
  budget: Budget;
  onBudget: (budget: Budget) => void;
  /** The bot's best line in the compared position. */
  bot: PvLineView | null;
  comparison: Comparison | null;
  busy: boolean;
  error: string | null;
}) {
  const toggleLevel = (level: number) => {
    if (levels.includes(level)) onLevels(levels.filter((l) => l !== level));
    else if (levels.length < MAX_COMPARED) onLevels([...levels, level]);
  };
  return (
    <Card>
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-[11px] font-medium tracking-[0.14em] text-[#5ed6f5] uppercase">
            Comparer à Stockfish
          </p>
          <p className="mt-1 text-xs text-dim">Éval de référence, en parallèle du bot</p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          aria-label="Comparer à Stockfish"
          disabled={!version}
          onClick={() => {
            onEnabled(!enabled);
          }}
          className={`relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-40 ${
            enabled ? 'bg-[#5ed6f5]' : 'bg-ink-700'
          }`}
        >
          <span
            className={`absolute top-0.5 size-4 rounded-full bg-bone transition-[left] ${
              enabled ? 'left-[18px]' : 'left-0.5'
            }`}
          />
        </button>
      </div>

      {!version ? (
        <p className="mt-4 rounded-xl border border-warn/30 bg-warn/[0.04] px-3 py-2 text-sm text-mute">
          Stockfish introuvable sur le serveur : il est inclus dans l’image Docker, sinon réglez{' '}
          <code className="font-mono text-warn">CHESS_SERVER_STOCKFISH</code>.
        </p>
      ) : (
        enabled && (
          <>
            <p className="mt-4 mb-2 text-xs text-dim">Niveaux affichés (jusqu’à {MAX_COMPARED})</p>
            <div className="grid grid-cols-5 gap-1.5">
              {Array.from({ length: 10 }, (_, i) => i + 1).map((level) => {
                const index = levels.indexOf(level);
                const color = index >= 0 ? LEVEL_COLORS[index] : undefined;
                return (
                  <button
                    key={level}
                    type="button"
                    aria-pressed={index >= 0}
                    onClick={() => {
                      toggleLevel(level);
                    }}
                    className="h-9 rounded-lg border border-line font-mono text-sm text-mute transition-colors hover:border-ink-600"
                    style={
                      color ? { borderColor: color, color, background: `${color}14` } : undefined
                    }
                  >
                    {level}
                  </button>
                );
              })}
            </div>
            <div className="mt-3">
              <Segmented
                label="Budget de Stockfish"
                value={budget}
                options={BUDGET_OPTIONS}
                onChange={onBudget}
                stretch
              />
            </div>

            <table className="mt-4 w-full text-sm">
              <thead>
                <tr className="text-left text-[10px] tracking-wider text-dim uppercase">
                  <th className="py-1.5 font-normal">Moteur</th>
                  <th className="py-1.5 text-right font-normal">Éval</th>
                  <th className="py-1.5 text-right font-normal">Δ bot</th>
                  <th className="py-1.5 text-right font-normal">Coup</th>
                </tr>
              </thead>
              <tbody className="font-mono">
                <tr className="bg-signal/[0.06]">
                  <td className="rounded-l-lg py-2 pl-2 font-sans font-semibold text-signal">
                    Bot
                  </td>
                  <td className="text-right">{bot ? formatScore(bot.score) : '—'}</td>
                  <td className="text-right text-dim">—</td>
                  <td className="rounded-r-lg pr-2 text-right">
                    {bot?.pvSan[0] ? frenchSan(bot.pvSan[0]) : '—'}
                  </td>
                </tr>
                {comparison?.rows.map((row) => {
                  const color = LEVEL_COLORS[levels.indexOf(row.level)] ?? '#a3a9b5';
                  return (
                    <tr key={row.level}>
                      <td className="py-2 pl-2 font-sans">
                        <span className="flex items-center gap-2">
                          <span className="size-2 rounded-sm" style={{ background: color }} />
                          SF {row.level}
                        </span>
                      </td>
                      <td className="text-right">{row.score ? formatScore(row.score) : '—'}</td>
                      <td className="text-right text-dim">
                        {row.delta === null ? '—' : formatDelta(row.delta)}
                      </td>
                      <td className={`pr-2 text-right ${row.agrees ? 'text-signal' : 'text-warn'}`}>
                        {row.san ? `${row.agrees ? '✓' : '≠'} ${frenchSan(row.san)}` : '—'}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {busy && <p className="mt-2 text-xs text-dim">Stockfish réfléchit…</p>}
            {error && <p className="mt-2 text-xs text-alert">{error}</p>}
            {comparison && comparison.rows.length > 0 && (
              <div className="mt-4 flex items-start gap-3 rounded-xl border border-line bg-ink-950 p-3">
                <span className="font-display text-3xl">
                  {comparison.agreeing}/{comparison.rows.length}
                </span>
                <p className="text-xs text-mute">
                  niveau(x) Stockfish jouent le même coup que le bot.
                  {comparison.meanDelta !== null && (
                    <>
                      {' '}
                      Écart moyen d’éval :{' '}
                      <span className="font-mono text-bone">
                        {formatDelta(comparison.meanDelta)}
                      </span>
                      .
                    </>
                  )}
                </p>
              </div>
            )}
            <p className="mt-3 text-xs text-dim" title={version}>
              {version} · position où c’est à vous de jouer
            </p>
          </>
        )
      )}
    </Card>
  );
}

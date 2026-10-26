import type { MoveLimit } from '../../api/generated/MoveLimit';
import { Button } from '../../components/ui';
import { formatDuration, pairingSeconds } from './estimate';
import { type Pair, allPairs, groups, idleBots, pairKey, toggleRow } from './pairings';
import { botLabel } from './stockfish';

/**
 * Cross table of the participants: each cell switches one pairing on or
 * off, each row header all the pairings of its bot.
 */
export function PairingMatrix({
  bots,
  disabled,
  onChange,
  enabled,
  settings,
}: {
  bots: readonly string[];
  disabled: ReadonlySet<string>;
  onChange: (disabled: Set<string>) => void;
  /** The pairings switched on, in playing order. */
  enabled: readonly Pair[];
  settings: { gamesPerPair: number; limit: MoveLimit; threads: number };
}) {
  const total = allPairs(bots).length;
  const idle = idleBots(bots, enabled);
  const playing = bots.filter((bot) => !idle.includes(bot));
  const split = enabled.length > 0 ? groups(playing, enabled) : [];

  const toggle = (a: string, b: string) => {
    const key = pairKey(a, b);
    const next = new Set(disabled);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    onChange(next);
  };

  return (
    <fieldset>
      <legend className="mb-2 text-sm text-slate-300">Combats</legend>
      <div className="-mx-1 overflow-x-auto px-1">
        <table className="border-separate border-spacing-1 text-xs">
          <thead>
            <tr>
              <th />
              {bots.map((bot) => (
                <th
                  key={bot}
                  scope="col"
                  className="max-w-20 truncate px-1 text-center font-normal text-slate-400"
                  title={botLabel(bot)}
                >
                  {botLabel(bot)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {bots.map((row) => (
              <tr key={row}>
                <th scope="row" className="pr-1 text-left font-normal">
                  <button
                    type="button"
                    title={`Activer ou désactiver tous les combats de ${botLabel(row)}`}
                    onClick={() => {
                      onChange(toggleRow(row, bots, disabled));
                    }}
                    className={`max-w-32 truncate rounded px-1.5 py-0.5 text-left hover:bg-slate-800 ${idle.includes(row) ? 'text-slate-500 line-through' : 'text-slate-200'}`}
                  >
                    {botLabel(row)}
                  </button>
                </th>
                {bots.map((column) => {
                  if (row === column) {
                    return (
                      <td key={column} className="p-0 text-center">
                        <span className="mx-auto block size-7 rounded bg-slate-950" />
                      </td>
                    );
                  }
                  const on = !disabled.has(pairKey(row, column));
                  const duration = formatDuration(pairingSeconds(row, column, settings));
                  return (
                    <td key={column} className="p-0 text-center">
                      <button
                        type="button"
                        aria-pressed={on}
                        aria-label={`${botLabel(row)} contre ${botLabel(column)}`}
                        title={`${botLabel(row)} contre ${botLabel(column)} : ${on ? `au programme (${duration})` : 'pas joué'}`}
                        onClick={() => {
                          toggle(row, column);
                        }}
                        className={`size-7 rounded text-sm transition-colors ${
                          on
                            ? 'bg-emerald-700/80 text-white hover:bg-emerald-600'
                            : 'bg-slate-800 text-slate-500 hover:bg-slate-700'
                        }`}
                      >
                        {on ? '✓' : '×'}
                      </button>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
        <span className="text-slate-400">
          {enabled.length} combat{enabled.length > 1 ? 's' : ''} sur {total}
        </span>
        <Button
          variant="ghost"
          className="px-2 py-0.5 text-xs"
          disabled={disabled.size === 0}
          onClick={() => {
            onChange(new Set());
          }}
        >
          Tout activer
        </Button>
        <Button
          variant="ghost"
          className="px-2 py-0.5 text-xs"
          disabled={enabled.length === 0}
          onClick={() => {
            onChange(new Set(allPairs(bots).map(([a, b]) => pairKey(a, b))));
          }}
        >
          Tout désactiver
        </Button>
      </div>
      <p className="mt-1 text-xs text-slate-500">
        Clique une case pour activer ou désactiver un combat, ou un nom pour tous ses combats.
      </p>
      {idle.length > 0 && enabled.length > 0 && (
        <p className="mt-1 text-xs text-amber-300">
          Sans combat, donc absent{idle.length > 1 ? 's' : ''} du tournoi :{' '}
          {idle.map(botLabel).join(', ')}.
        </p>
      )}
      {split.length > 1 && (
        <p className="mt-1 text-xs text-amber-300">
          Ces groupes ne se rencontrent jamais, leurs Elo ne sont pas comparables entre eux :{' '}
          {split.map((group) => group.map(botLabel).join(', ')).join(' | ')}.
        </p>
      )}
    </fieldset>
  );
}

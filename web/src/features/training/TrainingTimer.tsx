import { useEffect, useState } from 'react';

import { Button } from '../../components/ui';
import { DURATION_PRESETS, formatClock, formatRemaining, minutesError } from './timer';

const CUSTOM = 'custom';

/**
 * Picks a time limit: a preset, or a custom number of minutes. Reports
 * `null` for no limit and `undefined` while the custom value is invalid.
 */
export function DurationPicker({
  value,
  onChange,
  disabled = false,
}: {
  value: number | null | undefined;
  onChange: (minutes: number | null | undefined) => void;
  disabled?: boolean;
}) {
  const preset = DURATION_PRESETS.find((p) => p.minutes === value);
  const [custom, setCustom] = useState(value !== null && !preset);
  const [text, setText] = useState(value != null && !preset ? String(value) : '90');
  const error = custom ? minutesError(Number(text)) : null;

  return (
    <span className="inline-flex flex-wrap items-center gap-2 text-sm">
      <select
        aria-label="Durée de l’entraînement"
        disabled={disabled}
        value={custom ? CUSTOM : String(value ?? '')}
        onChange={(event) => {
          const choice = event.target.value;
          if (choice === CUSTOM) {
            setCustom(true);
            onChange(minutesError(Number(text)) === null ? Number(text) : undefined);
          } else {
            setCustom(false);
            onChange(choice === '' ? null : Number(choice));
          }
        }}
        className="rounded-xl border border-line bg-ink-950 px-2 py-1.5 text-sm disabled:opacity-40"
      >
        {DURATION_PRESETS.map((p) => (
          <option key={p.label} value={p.minutes ?? ''}>
            {p.label}
          </option>
        ))}
        <option value={CUSTOM}>Personnalisée…</option>
      </select>
      {custom && (
        <label className="inline-flex items-center gap-1">
          <input
            type="number"
            min={1}
            step={5}
            value={text}
            disabled={disabled}
            onChange={(event) => {
              setText(event.target.value);
              const minutes = Number(event.target.value);
              onChange(minutesError(minutes) === null ? minutes : undefined);
            }}
            aria-invalid={error !== null}
            className={`w-20 rounded-xl border bg-ink-950 px-2 py-1 font-mono text-sm ${error ? 'border-alert' : 'border-line'}`}
          />
          <span className="text-dim">min</span>
          {error && <span className="text-xs text-alert">{error}</span>}
        </label>
      )}
    </span>
  );
}

/** Time limit of the running training: when it stops, and how to change it. */
export function TrainingTimer({
  deadline,
  busy,
  onSet,
}: {
  /** Unix seconds; absent: no limit. */
  deadline: number | null;
  busy: boolean;
  onSet: (minutes: number | null) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [minutes, setMinutes] = useState<number | null | undefined>(60);
  // A clock ticking every second: a live countdown between two polls.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      clearInterval(timer);
    };
  }, []);
  const remaining = deadline === null ? null : deadline - now / 1000;

  return (
    <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-lg bg-ink-950 px-3 py-2 text-sm">
      <span aria-hidden>⏱</span>
      {deadline === null ? (
        <span className="text-dim">Sans limite de temps</span>
      ) : (
        <span>
          Arrêt automatique à <span className="font-mono">{formatClock(deadline)}</span>
          <span className="text-dim">
            {' '}
            (dans {formatRemaining(remaining ?? 0)}) — fin de l’étape, checkpoint, export
          </span>
        </span>
      )}
      <span className="flex-1" />
      {editing ? (
        <>
          <span className="text-dim">S’arrêter dans</span>
          <DurationPicker value={minutes} onChange={setMinutes} disabled={busy} />
          <Button
            variant="primary"
            className="px-2 py-1 text-xs"
            disabled={busy || minutes === undefined}
            onClick={() => {
              if (minutes !== undefined) onSet(minutes);
              setEditing(false);
            }}
          >
            Appliquer
          </Button>
          <Button
            variant="ghost"
            className="px-2 py-1 text-xs"
            onClick={() => {
              setEditing(false);
            }}
          >
            Annuler
          </Button>
        </>
      ) : (
        <>
          <Button
            variant="ghost"
            className="px-2 py-1 text-xs"
            disabled={busy}
            onClick={() => {
              setEditing(true);
            }}
          >
            {deadline === null ? 'Ajouter un minuteur' : 'Modifier'}
          </Button>
          {deadline !== null && (
            <Button
              variant="ghost"
              className="px-2 py-1 text-xs"
              disabled={busy}
              onClick={() => {
                onSet(null);
              }}
            >
              Retirer
            </Button>
          )}
        </>
      )}
    </div>
  );
}

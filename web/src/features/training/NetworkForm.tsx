import { useEffect, useState } from 'react';

import { ApiError, api } from '../../api/client';
import type { NetworkParams } from '../../api/generated/NetworkParams';
import type { NetworkSummary } from '../../api/generated/NetworkSummary';
import { Button, ErrorText } from '../../components/ui';
import {
  type NumericParam,
  PARAM_FIELDS,
  PARAM_GROUPS,
  formatParam,
  nameError,
  paramError,
  randomName,
  randomParams,
} from './params';

type Drafts = Record<NumericParam, string>;

function toDrafts(params: NetworkParams): Drafts {
  return Object.fromEntries(
    PARAM_FIELDS.map((field) => [field.key, String(params[field.key])]),
  ) as Drafts;
}

const INPUT =
  'w-full rounded-xl border border-line bg-ink-950 px-2 py-1.5 font-mono text-sm disabled:opacity-50';

/** Creates a network: a name and its hyperparameters, from the defaults or another network. */
export function NetworkForm({
  networks,
  onCreated,
  onCancel,
}: {
  networks: readonly NetworkSummary[];
  onCreated: (name: string) => void;
  onCancel: () => void;
}) {
  const [defaults, setDefaults] = useState<NetworkParams | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [base, setBase] = useState('');
  const [drafts, setDrafts] = useState<Drafts | null>(null);
  const [datasetsFrom, setDatasetsFrom] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .networkDefaults()
      .then((params) => {
        if (cancelled) return;
        setDefaults(params);
        setDrafts(toDrafts(params));
      })
      .catch((err: unknown) => {
        if (!cancelled) setLoadError(err instanceof Error ? err.message : 'erreur');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const startFrom = (source: string) => {
    setBase(source);
    const params = source ? networks.find((n) => n.name === source)?.params : defaults;
    if (params) {
      setDrafts(toDrafts(params));
      setDatasetsFrom([...params.datasetsFrom]);
    }
  };

  if (loadError) {
    return (
      <div className="space-y-3">
        <ErrorText>
          Impossible de lire les paramètres par défaut : {loadError}. Le service d’entraînement doit
          tourner pour créer un réseau.
        </ErrorText>
        <Button onClick={onCancel}>Fermer</Button>
      </div>
    );
  }
  if (!drafts) return <p className="text-sm text-dim">Chargement des paramètres…</p>;

  const values = Object.fromEntries(
    PARAM_FIELDS.map((field) => [field.key, Number(drafts[field.key])]),
  ) as Record<NumericParam, number>;
  const fieldErrors = Object.fromEntries(
    PARAM_FIELDS.map((field) => [field.key, paramError(field, values[field.key])]),
  ) as Record<NumericParam, string | null>;
  const nameProblem = name ? nameError(name) : null;
  const taken = networks.some((network) => network.name === name);
  const invalid =
    !name || nameProblem !== null || taken || Object.values(fieldErrors).some((e) => e !== null);
  const sources = networks.filter((network) => network.name !== name);

  // Draws from the current values (valid or not, the drawn ones replace them).
  const randomize = () => {
    setDrafts(toDrafts(randomParams({ ...values, datasetsFrom })));
    if (!name) setName(randomName());
  };

  const submit = async (start: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const params: NetworkParams = {
        ...values,
        datasetsFrom: datasetsFrom.filter((source) => source !== name),
      };
      await api.createNetwork({ name, params });
      if (start) {
        try {
          await api.startTraining({ network: name, fresh: false });
        } catch (err: unknown) {
          if (!(err instanceof ApiError && err.status === 409)) throw err;
          // Another network is training: the new one is created, not started.
        }
      }
      onCreated(name);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'erreur');
    } finally {
      setBusy(false);
    }
  };

  return (
    // noValidate: the fields are checked against the service's bounds below.
    <form
      noValidate
      className="space-y-5"
      onSubmit={(event) => {
        event.preventDefault();
        if (!invalid) void submit(false);
      }}
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <label className="block text-sm">
          <span className="text-mute">Nom du réseau</span>
          <input
            value={name}
            onChange={(event) => {
              setName(event.target.value.trim());
            }}
            placeholder="ex. lr-3e4"
            autoFocus
            className={`mt-1 ${INPUT}`}
          />
          {(nameProblem ?? (taken ? 'ce nom est déjà pris' : null)) && (
            <span className="mt-1 block text-xs text-alert">
              {nameProblem ?? 'ce nom est déjà pris'}
            </span>
          )}
        </label>
        <label className="block text-sm">
          <span className="text-mute">Partir des paramètres de</span>
          <select
            value={base}
            onChange={(event) => {
              startFrom(event.target.value);
            }}
            className={`mt-1 ${INPUT} font-sans`}
          >
            <option value="">Valeurs par défaut</option>
            {networks
              .filter((network) => network.params)
              .map((network) => (
                <option key={network.name} value={network.name}>
                  {network.name}
                </option>
              ))}
          </select>
        </label>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <Button onClick={randomize}>Hyperparamètres aléatoires</Button>
        <span className="text-xs text-dim">
          Tire au hasard les nœuds de self-play, le taux d’apprentissage et sa décroissance, le
          poids du résultat, la taille de batch et la réutilisation, dans des plages raisonnables ;
          la durée et la mesure ne changent pas.
        </span>
      </div>

      {PARAM_GROUPS.map((group) => (
        <fieldset key={group.title}>
          <legend className="mb-2 text-xs font-semibold tracking-wider text-dim uppercase">
            {group.title}
          </legend>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {group.fields.map((field) => {
              const fieldError = fieldErrors[field.key];
              const fallback = defaults?.[field.key];
              return (
                <label key={field.key} className="block text-sm" title={field.help}>
                  <span className="text-mute">{field.label}</span>
                  <input
                    type="number"
                    inputMode="decimal"
                    min={field.min}
                    max={field.max}
                    step="any"
                    value={drafts[field.key]}
                    onChange={(event) => {
                      setDrafts({ ...drafts, [field.key]: event.target.value });
                    }}
                    aria-invalid={fieldError !== null}
                    className={`mt-1 ${INPUT} ${fieldError ? 'border-alert' : ''}`}
                  />
                  <span className={`mt-1 block text-xs ${fieldError ? 'text-alert' : 'text-dim'}`}>
                    {fieldError ??
                      (fallback !== undefined && values[field.key] !== fallback
                        ? `défaut : ${formatParam(field.key, fallback)}`
                        : field.help)}
                  </span>
                </label>
              );
            })}
          </div>
        </fieldset>
      ))}

      <fieldset>
        <legend className="mb-2 text-xs font-semibold tracking-wider text-dim uppercase">
          Réutiliser les parties de
        </legend>
        {sources.length === 0 ? (
          <p className="text-sm text-dim">Aucun autre réseau.</p>
        ) : (
          <div className="flex flex-wrap gap-x-5 gap-y-2">
            {sources.map((network) => (
              <label key={network.name} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={datasetsFrom.includes(network.name)}
                  onChange={(event) => {
                    setDatasetsFrom(
                      event.target.checked
                        ? [...datasetsFrom, network.name]
                        : datasetsFrom.filter((source) => source !== network.name),
                    );
                  }}
                  className="size-4 accent-signal"
                />
                {network.name}
              </label>
            ))}
          </div>
        )}
        <p className="mt-2 text-xs text-dim">
          Le réseau s’entraîne aussi sur les parties déjà jouées par ces réseaux : il démarre sans
          attendre son propre self-play.
        </p>
      </fieldset>

      {error && <ErrorText>{error}</ErrorText>}
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" disabled={invalid || busy} onClick={() => void submit(true)}>
          Créer et entraîner
        </Button>
        <Button type="submit" disabled={invalid || busy}>
          Créer
        </Button>
        <Button variant="ghost" onClick={onCancel} disabled={busy}>
          Annuler
        </Button>
      </div>
    </form>
  );
}

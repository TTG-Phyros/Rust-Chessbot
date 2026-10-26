import type { NetworkParams } from '../../api/generated/NetworkParams';

export type NumericParam = Exclude<keyof NetworkParams, 'datasetsFrom'>;

export type ParamField = {
  key: NumericParam;
  label: string;
  help: string;
  /** Bounds checked by the training service. */
  min: number;
  max: number;
  step: number;
};

export type ParamGroup = { title: string; fields: ParamField[] };

/** Every hyperparameter a network can change, grouped as in the form. */
export const PARAM_GROUPS: ParamGroup[] = [
  {
    title: 'Self-play',
    fields: [
      {
        key: 'selfplayNodes',
        label: 'Nœuds par coup',
        help: 'Recherche de chaque coup des parties d’entraînement : plus de nœuds, des parties de meilleure qualité mais plus lentes à générer.',
        min: 100,
        max: 1_000_000,
        step: 500,
      },
    ],
  },
  {
    title: 'Optimisation',
    fields: [
      {
        key: 'learningRate',
        label: 'Taux d’apprentissage',
        help: 'Pas initial d’AdamW. Trop haut : la perte oscille ; trop bas : l’apprentissage traîne.',
        min: 0.000_001,
        max: 1,
        step: 0.0001,
      },
      {
        key: 'lrDropEvery',
        label: 'Baisse du taux tous les',
        help: 'Nombre de superbatchs entre deux baisses du taux d’apprentissage.',
        min: 1,
        max: 100_000,
        step: 1,
      },
      {
        key: 'lrDropFactor',
        label: 'Facteur de baisse',
        help: 'Le taux d’apprentissage est multiplié par ce facteur à chaque baisse (1 = jamais).',
        min: 0.01,
        max: 1,
        step: 0.05,
      },
      {
        key: 'wdl',
        label: 'Poids du résultat (WDL)',
        help: 'Part du résultat de la partie dans la cible, le reste étant le score de la recherche (0 = score seul, 1 = résultat seul).',
        min: 0,
        max: 1,
        step: 0.05,
      },
      {
        key: 'batchSize',
        label: 'Taille de batch',
        help: 'Positions par pas d’optimisation (16 384 occupe environ 100 Mo de VRAM).',
        min: 64,
        max: 65_536,
        step: 1024,
      },
      {
        key: 'batchesPerSuperbatch',
        label: 'Batchs par superbatch',
        help: 'Pas d’optimisation entre deux checkpoints (un point des courbes chacun).',
        min: 1,
        max: 100_000,
        step: 50,
      },
      {
        key: 'superbatches',
        label: 'Superbatchs (0 = illimité)',
        help: 'L’entraînement s’arrête seul après ce nombre de superbatchs.',
        min: 0,
        max: 1_000_000,
        step: 10,
      },
    ],
  },
  {
    title: 'Données',
    fields: [
      {
        key: 'minPositions',
        label: 'Positions avant de commencer',
        help: 'Le self-play génère d’abord ce nombre de positions (ou les trouve chez les réseaux dont on réutilise les parties).',
        min: 0,
        max: 1_000_000_000,
        step: 100_000,
      },
      {
        key: 'maxReuse',
        label: 'Réutilisation max (0 = illimitée)',
        help: 'Nombre moyen de passages sur chaque position avant d’attendre de nouvelles parties : évite le surapprentissage.',
        min: 0,
        max: 100_000,
        step: 5,
      },
    ],
  },
  {
    title: 'Mesure et top 5',
    fields: [
      {
        key: 'exportEvery',
        label: 'Export tous les',
        help: 'Superbatchs entre deux exports. Chaque export joue un match contre PeSTO et peut entrer dans le top 5.',
        min: 1,
        max: 100_000,
        step: 1,
      },
      {
        key: 'evalGames',
        label: 'Parties par mesure',
        help: 'Parties de chaque match contre PeSTO : plus de parties, un Elo plus précis (et un top 5 plus fiable), mais plus de CPU pris au self-play.',
        min: 2,
        max: 10_000,
        step: 16,
      },
    ],
  },
];

export const PARAM_FIELDS: ParamField[] = PARAM_GROUPS.flatMap((group) => group.fields);

/** Why a value is out of the accepted range, if it is. */
export function paramError(field: ParamField, value: number): string | null {
  if (!Number.isFinite(value)) return 'nombre attendu';
  if (value < field.min || value > field.max) {
    return `entre ${field.min.toLocaleString('fr-FR')} et ${field.max.toLocaleString('fr-FR')}`;
  }
  if (field.key === 'learningRate' && value <= 0) return 'strictement positif';
  return null;
}

/** Same rule as the server: 1 to 32 letters, digits, `-` or `_`. */
export function nameError(name: string): string | null {
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,31}$/.test(name)) {
    return '1 à 32 lettres, chiffres, « - » ou « _ », sans espace';
  }
  if (name.toLowerCase() === 'classic') return '« classic » est réservé à PeSTO';
  return null;
}

/** `1.0e-3`, `16 384`: compact display of a parameter value. */
export function formatParam(key: NumericParam, value: number): string {
  return key === 'learningRate' ? value.toExponential(1) : value.toLocaleString('fr-FR');
}

/** Uniform draw in `[0, 1)`; injectable for tests. */
export type Random = () => number;

const pick = <T>(values: readonly T[], random: Random): T =>
  values[Math.min(values.length - 1, Math.floor(random() * values.length))] as T;

/** Log-uniform draw in `[min, max]`, rounded to `significant` digits. */
function logUniform(min: number, max: number, significant: number, random: Random): number {
  const value = Math.exp(Math.log(min) + random() * (Math.log(max) - Math.log(min)));
  return Number(value.toPrecision(significant));
}

/** Uniform draw in `[min, max]` on a grid of `step`. */
function stepped(min: number, max: number, step: number, random: Random): number {
  const steps = Math.round((max - min) / step);
  return Number((min + Math.floor(random() * (steps + 1)) * step).toFixed(6));
}

/**
 * Hyperparameters drawn at random in ranges that train reasonably, for
 * exploring: what shapes the network (self-play strength, optimisation,
 * target, data reuse) changes; duration, start threshold and measurement
 * settings are kept from `base`.
 */
export function randomParams(base: NetworkParams, random: Random = Math.random): NetworkParams {
  return {
    ...base,
    selfplayNodes: Math.round(logUniform(2_000, 20_000, 3, random) / 500) * 500,
    learningRate: logUniform(1e-4, 3e-3, 2, random),
    lrDropEvery: pick([30, 50, 75, 100, 150, 200], random),
    lrDropFactor: stepped(0.2, 0.6, 0.05, random),
    wdl: stepped(0, 0.6, 0.05, random),
    batchSize: pick([4_096, 8_192, 16_384, 32_768], random),
    maxReuse: pick([10, 20, 30, 50], random),
  };
}

/** A name for a network with random settings: `rand-` and 4 letters or digits. */
export function randomName(random: Random = Math.random): string {
  const alphabet = 'abcdefghijkmnpqrstuvwxyz23456789';
  return `rand-${Array.from({ length: 4 }, () => pick(alphabet.split(''), random)).join('')}`;
}

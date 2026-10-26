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

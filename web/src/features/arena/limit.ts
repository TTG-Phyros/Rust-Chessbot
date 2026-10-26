import type { MoveLimit } from '../../api/generated/MoveLimit';

export type LimitKind = MoveLimit['kind'];

export type LimitOption = {
  kind: LimitKind;
  /** Segmented control label. */
  label: string;
  /** Input label. */
  field: string;
  /** Bounds checked by the training service. */
  min: number;
  max: number;
  step: number;
  initial: number;
  help: string;
};

export const LIMIT_OPTIONS: readonly LimitOption[] = [
  {
    kind: 'nodes',
    label: 'Nœuds',
    field: 'Nœuds par coup',
    min: 100,
    max: 10_000_000,
    step: 500,
    initial: 5000,
    help: 'Chaque bot examine ce nombre de positions avant de jouer : même effort pour tous, résultats reproductibles quelle que soit la charge de la machine. Environ 4 M nœuds/s par thread.',
  },
  {
    kind: 'depth',
    label: 'Profondeur',
    field: 'Profondeur (demi-coups)',
    min: 1,
    max: 30,
    step: 1,
    initial: 8,
    help: 'Chaque bot cherche jusqu’à cette profondeur, quel que soit le nombre de positions : le temps par coup varie beaucoup selon la position.',
  },
  {
    kind: 'time',
    label: 'Temps',
    field: 'Temps par coup (ms)',
    min: 10,
    max: 60_000,
    step: 50,
    initial: 100,
    help: 'Chaque bot réfléchit ce temps : une évaluation plus rapide cherche plus loin. Dépend de la charge de la machine (un entraînement en parallèle ralentit tout le monde).',
  },
];

export function limitOption(kind: LimitKind): LimitOption {
  return LIMIT_OPTIONS.find((option) => option.kind === kind) ?? (LIMIT_OPTIONS[0] as LimitOption);
}

/** Why `value` is not accepted for `kind`, if it is not. */
export function limitError(kind: LimitKind, value: number): string | null {
  const { min, max } = limitOption(kind);
  if (!Number.isInteger(value)) return 'nombre entier attendu';
  if (value < min || value > max) {
    return `entre ${min.toLocaleString('fr-FR')} et ${max.toLocaleString('fr-FR')}`;
  }
  return null;
}

/** `5 000 nœuds par coup`, `profondeur 8`, `100 ms par coup`. */
export function describeLimit(limit: MoveLimit): string {
  switch (limit.kind) {
    case 'nodes':
      return `${limit.value.toLocaleString('fr-FR')} nœuds par coup`;
    case 'depth':
      return `profondeur ${String(limit.value)}`;
    case 'time':
      return `${limit.value.toLocaleString('fr-FR')} ms par coup`;
  }
}

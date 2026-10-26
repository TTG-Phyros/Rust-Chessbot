import type { ControllerState } from '../../api/generated/ControllerState';
import type { EloPoint } from '../../api/generated/EloPoint';

/** `12 345`, `1,2 M`. */
export function formatCount(value: number): string {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} M`;
  }
  return Math.round(value).toLocaleString('fr-FR');
}

/** Elapsed time between two Unix timestamps: `42 s`, `5 min`, `2 h 07`. */
export function formatElapsed(from: number, to: number): string {
  const seconds = Math.max(0, to - from);
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  return `${hours} h ${String(minutes % 60).padStart(2, '0')}`;
}

/** `+12 ± 30` (Elo with its 95 % interval). */
export function formatElo(elo: number, error: number): string {
  const sign = elo > 0 ? '+' : elo < 0 ? '−' : '';
  return `${sign}${Math.abs(Math.round(elo))} ± ${Math.round(error)}`;
}

/** `26/10 14:03` (local time of a Unix timestamp). */
export function formatDate(unix: number): string {
  return new Date(unix * 1000).toLocaleString('fr-FR', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** `12,5` points: halves for draws. */
export function formatPoints(points: number): string {
  return points.toLocaleString('fr-FR', { maximumFractionDigits: 1 });
}

export const CONTROLLER_LABEL: Record<ControllerState, string> = {
  unavailable: 'Service arrêté',
  idle: 'En attente',
  running: 'En cours',
  stopping: 'Arrêt en cours…',
};

export const PHASE_LABEL: Record<string, string> = {
  pipeline: 'Self-play + entraînement',
  training: 'Entraînement',
  selfplay: 'Self-play',
  finished: 'Terminé',
};

/** Elo points with the interval as a `[low, high]` band, for an area chart. */
export function eloBand(points: readonly EloPoint[]) {
  return points.map((point) => ({
    superbatch: point.superbatch,
    elo: point.elo,
    band: [point.elo - point.error, point.elo + point.error] as [number, number],
    error: point.error,
    games: point.games,
  }));
}

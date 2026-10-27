/** Time limit of a training run (the service accepts 1 minute to a week). */

const MAX_MINUTES = 7 * 24 * 60;

/** Choices of the duration picker; `null`: no time limit. */
export const DURATION_PRESETS: readonly { minutes: number | null; label: string }[] = [
  { minutes: null, label: 'Sans limite' },
  { minutes: 15, label: '15 min' },
  { minutes: 30, label: '30 min' },
  { minutes: 60, label: '1 h' },
  { minutes: 120, label: '2 h' },
  { minutes: 240, label: '4 h' },
  { minutes: 480, label: '8 h' },
  { minutes: 720, label: '12 h' },
  { minutes: 1440, label: '24 h' },
];

/** Why `minutes` is not an accepted time limit, if it is not. */
export function minutesError(minutes: number): string | null {
  if (!Number.isInteger(minutes)) return 'nombre entier de minutes';
  if (minutes < 1 || minutes > MAX_MINUTES)
    return `entre 1 et ${MAX_MINUTES.toLocaleString('fr-FR')} minutes`;
  return null;
}

/** Time left: `45 s`, `8 min`, `1 h 05`, `2 j 3 h`. */
export function formatRemaining(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${String(s)} s`;
  const minutes = Math.floor(s / 60);
  if (minutes < 60) return `${String(minutes)} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${String(hours)} h ${String(minutes % 60).padStart(2, '0')}`;
  return `${String(Math.floor(hours / 24))} j ${String(hours % 24)} h`;
}

/** `18:30`, or `mar. 27/10 18:30` when it is not today. */
export function formatClock(unix: number, now: Date = new Date()): string {
  const date = new Date(unix * 1000);
  const time = date.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
  if (date.toDateString() === now.toDateString()) return time;
  const day = date.toLocaleDateString('fr-FR', {
    weekday: 'short',
    day: '2-digit',
    month: '2-digit',
  });
  return `${day} ${time}`;
}

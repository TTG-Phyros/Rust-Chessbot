/**
 * `04:47` (minutes and seconds), `1:02:05` past an hour; with `hundredths`,
 * `00:00.04` under an hour (a bot thinks in milliseconds).
 */
export function formatClock(ms: number, hundredths = false): string {
  const clamped = Math.max(0, ms);
  const total = Math.floor(clamped / 1000);
  const seconds = String(total % 60).padStart(2, '0');
  const minutes = Math.floor(total / 60);
  if (minutes >= 60) {
    return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, '0')}:${seconds}`;
  }
  const clock = `${String(minutes).padStart(2, '0')}:${seconds}`;
  return hundredths
    ? `${clock}.${String(Math.floor((clamped % 1000) / 10)).padStart(2, '0')}`
    : clock;
}

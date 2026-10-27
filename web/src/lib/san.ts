/**
 * French algebraic notation. The server speaks English SAN (as PGN
 * requires); the UI shows the French piece letters: R (roi), D (dame),
 * T (tour), F (fou), C (cavalier).
 */
const FRENCH: Record<string, string> = { K: 'R', Q: 'D', R: 'T', B: 'F', N: 'C' };

/** `Nf3` → `Cf3`, `exd8=Q+` → `exd8=D+`, `O-O` unchanged. */
export function frenchSan(san: string): string {
  return san.replace(/^[KQRBN]|=[QRBN]/g, (match) =>
    match.startsWith('=') ? `=${FRENCH[match.slice(1)] ?? ''}` : (FRENCH[match] ?? match),
  );
}

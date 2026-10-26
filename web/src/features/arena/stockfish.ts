/**
 * The Stockfish bots of the arena: `stockfish-1` to `stockfish-10`, played
 * by the training service. Mirrors the level table of
 * `crates/chess-train/src/stockfish.rs` (for display only).
 */

export type StockfishLevel = {
  level: number;
  /** UCI `Skill Level`, 0 to 20. */
  skill: number;
  depth: number;
  movetimeMs: number;
};

export const STOCKFISH_LEVELS: readonly StockfishLevel[] = [
  [0, 1, 50],
  [2, 1, 100],
  [4, 2, 150],
  [6, 3, 200],
  [8, 5, 300],
  [10, 8, 400],
  [13, 13, 500],
  [16, 22, 1000],
  [18, 26, 1500],
  [20, 30, 2000],
].map(([skill, depth, movetimeMs], index) => ({
  level: index + 1,
  skill: skill ?? 0,
  depth: depth ?? 1,
  movetimeMs: movetimeMs ?? 0,
}));

export const stockfishBot = (level: number) => `stockfish-${String(level)}`;

/** The level of a `stockfish-N` bot name, or `null`. */
export function stockfishLevel(name: string): number | null {
  const match = /^stockfish-([1-9]|10)$/.exec(name);
  return match ? Number(match[1]) : null;
}

/** `niveau 5 : skill 8, profondeur 5, 300 ms par coup`. */
export function describeLevel({ level, skill, depth, movetimeMs }: StockfishLevel): string {
  return `niveau ${String(level)} : skill ${String(skill)}/20, profondeur ${String(depth)}, ${String(movetimeMs)} ms par coup`;
}

/** Display name of any bot: `PeSTO`, `Stockfish 5`, or the saved name. */
export function botLabel(name: string): string {
  if (name === 'classic') return 'PeSTO';
  const level = stockfishLevel(name);
  return level === null ? name : `Stockfish ${String(level)}`;
}

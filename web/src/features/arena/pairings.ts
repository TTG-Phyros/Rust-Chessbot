/** Which pairings of a tournament are played, chosen in its cross table. */

export type Pair = readonly [string, string];

/** Same key whichever way round the pair is given. */
export function pairKey(a: string, b: string): string {
  return a < b ? `${a}\u0000${b}` : `${b}\u0000${a}`;
}

/** Every pair of `bots`, in playing order (that of the bots). */
export function allPairs(bots: readonly string[]): Pair[] {
  return bots.flatMap((a, i) => bots.slice(i + 1).map((b) => [a, b] as const));
}

/** The pairs of `bots` not switched off. */
export function enabledPairs(bots: readonly string[], disabled: ReadonlySet<string>): Pair[] {
  return allPairs(bots).filter(([a, b]) => !disabled.has(pairKey(a, b)));
}

/** Bots of `bots` that play none of `pairs`. */
export function idleBots(bots: readonly string[], pairs: readonly Pair[]): string[] {
  return bots.filter((bot) => !pairs.some(([a, b]) => a === bot || b === bot));
}

/**
 * Groups of bots linked, directly or not, by `pairs`. Elo ratings compare
 * within a group only: two groups that never meet have no common scale.
 */
export function groups(bots: readonly string[], pairs: readonly Pair[]): string[][] {
  const parent = new Map(bots.map((bot) => [bot, bot]));
  const root = (bot: string): string => {
    let current = bot;
    while (parent.get(current) !== current) current = parent.get(current) ?? current;
    return current;
  };
  for (const [a, b] of pairs) parent.set(root(a), root(b));
  const byRoot = new Map<string, string[]>();
  for (const bot of bots) byRoot.set(root(bot), [...(byRoot.get(root(bot)) ?? []), bot]);
  return [...byRoot.values()];
}

/**
 * Toggles every pairing of `bot` among `bots`: all off if any is on,
 * all on otherwise.
 */
export function toggleRow(
  bot: string,
  bots: readonly string[],
  disabled: ReadonlySet<string>,
): Set<string> {
  const keys = bots.filter((other) => other !== bot).map((other) => pairKey(bot, other));
  const anyOn = keys.some((key) => !disabled.has(key));
  const next = new Set(disabled);
  for (const key of keys) {
    if (anyOn) next.add(key);
    else next.delete(key);
  }
  return next;
}

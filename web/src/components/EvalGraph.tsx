/**
 * Evaluation along a game: one point per position (in pawns for White,
 * already clamped to `[-cap, cap]`), `null` where it is unknown.
 */
export function EvalGraph({
  points,
  cap,
  current,
  onSelect,
  height = 160,
  labels = true,
  overlays = [],
  marks = [],
}: {
  points: readonly (number | null)[];
  /** Other series drawn as plain lines over the main one (same scale). */
  overlays?: readonly { points: readonly (number | null)[]; color: string }[];
  cap: number;
  /** Index of the position on the board: a cursor and a lit dot. */
  current?: number;
  /** Makes the graph clickable: called with the nearest position. */
  onSelect?: (index: number) => void;
  height?: number;
  labels?: boolean;
  /** Colored dots on some positions (the notable moves), over the line. */
  marks?: readonly { index: number; color: string; title: string }[];
}) {
  const span = Math.max(1, points.length - 1);
  const x = (index: number) => (index / span) * 100;
  // Top 4 %, bottom 96 %: dots at the bounds stay whole.
  const y = (pawns: number) => 50 - (pawns / cap) * 46;
  const known = points.flatMap((pawns, index) => (pawns === null ? [] : [{ index, pawns }]));
  const polyline = (series: readonly (number | null)[]) =>
    series.flatMap((pawns, index) => (pawns === null ? [] : [`${x(index)},${y(pawns)}`])).join(' ');
  const line = polyline(points);
  const first = known[0];
  const last = known[known.length - 1];
  const area = first && last ? `${x(first.index)},50 ${line} ${x(last.index)},50` : '';
  const dots = known.length <= 80;

  return (
    // Keyboard users walk through the game with the arrow keys instead.
    <div
      className={`relative w-full ${onSelect ? 'cursor-pointer' : ''}`}
      style={{ height }}
      onClick={(event) => {
        if (!onSelect) return;
        const rect = event.currentTarget.getBoundingClientRect();
        const ratio = (event.clientX - rect.left) / rect.width;
        onSelect(Math.max(0, Math.min(span, Math.round(ratio * span))));
      }}
    >
      <svg
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        className="absolute inset-0 size-full overflow-visible"
        aria-hidden
      >
        <line x1="0" x2="100" y1="50" y2="50" stroke="#232731" vectorEffect="non-scaling-stroke" />
        {labels && (
          <>
            <line
              x1="0"
              x2="100"
              y1="4"
              y2="4"
              stroke="#14171e"
              vectorEffect="non-scaling-stroke"
            />
            <line
              x1="0"
              x2="100"
              y1="96"
              y2="96"
              stroke="#14171e"
              vectorEffect="non-scaling-stroke"
            />
          </>
        )}
        {area && <polygon points={area} fill="#eceae4" fillOpacity="0.12" />}
        {line && (
          <polyline
            points={line}
            fill="none"
            stroke="#eceae4"
            strokeWidth="1.5"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
        )}
        {overlays.map((overlay, i) => (
          <polyline
            key={i}
            points={polyline(overlay.points)}
            fill="none"
            stroke={overlay.color}
            strokeWidth="1.5"
            strokeDasharray="3 3"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {current !== undefined && (
          <line
            x1={x(current)}
            x2={x(current)}
            y1="0"
            y2="100"
            stroke="#c8f560"
            strokeOpacity="0.7"
            vectorEffect="non-scaling-stroke"
          />
        )}
      </svg>
      {known.map(({ index, pawns }) => {
        const lit = index === current;
        if (!dots && !lit) return null;
        const style = { left: `${x(index)}%`, top: `${y(pawns)}%` };
        const className = `pointer-events-none absolute -translate-x-1/2 -translate-y-1/2 rounded-full border-2 ${
          lit ? 'size-3 border-ink-950 bg-signal' : 'size-2 border-bone bg-ink-950'
        }`;
        return <span key={index} style={style} className={className} />;
      })}
      {marks.map(({ index, color, title }) => {
        const pawns = points[index];
        if (pawns === null || pawns === undefined) return null;
        return (
          <span
            key={`mark-${index}`}
            title={title}
            style={{ left: `${x(index)}%`, top: `${y(pawns)}%`, background: color }}
            className="absolute size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border border-ink-950"
          />
        );
      })}
      {labels && (
        <>
          <span className="absolute top-0 left-1 font-mono text-[10px] text-dim">
            +{cap.toFixed(2)}
          </span>
          <span className="absolute bottom-0 left-1 font-mono text-[10px] text-dim">
            −{cap.toFixed(2)}
          </span>
        </>
      )}
    </div>
  );
}

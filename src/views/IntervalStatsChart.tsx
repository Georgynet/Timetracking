import { format } from "date-fns";
import type { Granularity, IntervalBucket } from "../api/types";
import { formatDuration } from "../lib/format";

const MAX_DISTINCT_TICKETS = 8;
const SERIES_VARS = [
  "--series-1",
  "--series-2",
  "--series-3",
  "--series-4",
  "--series-5",
  "--series-6",
  "--series-7",
  "--series-8",
];

/** Minimum column slot width per granularity — widened further below if a bucket's
 * own label (e.g. a cross-month week range) would be wider than this. */
const MIN_COLUMN_SLOT_WIDTH: Record<Granularity, number> = { day: 40, week: 56, month: 56 };
const BAR_WIDTH = 24;
const PLOT_HEIGHT = 200;
const PLOT_TOP = 14; // headroom so the topmost gridline label isn't clipped by the SVG edge
const GAP = 2; // surface-color gap between stacked segments
const CAP_RADIUS = 4; // rounded top corners on the outermost segment only

interface Segment {
  key: string;
  label: string;
  seconds: number;
  color: string; // CSS custom property reference, e.g. "var(--series-1)"
}

/** Parsed as local midnight, not UTC, so a `YYYY-MM-DD` bucket boundary never shifts
 * to the previous/next day under a non-UTC system timezone. */
function parseLocalDate(dateOnly: string): Date {
  return new Date(`${dateOnly}T00:00:00`);
}

function bucketLabel(bucket: IntervalBucket, granularity: Granularity): string {
  const start = parseLocalDate(bucket.periodStart);
  if (granularity === "day") return format(start, "MMM d");
  if (granularity === "month") return format(start, "MMM yyyy");
  const end = parseLocalDate(bucket.periodEnd);
  if (format(start, "MMM d") === format(end, "MMM d")) return format(start, "MMM d");
  // Drop the repeated month name for a same-month week ("Aug 10–16") — only spell it
  // out twice when the week actually crosses a month boundary ("Aug 30–Sep 5").
  return format(start, "MMM") === format(end, "MMM")
    ? `${format(start, "MMM d")}–${format(end, "d")}`
    : `${format(start, "MMM d")}–${format(end, "MMM d")}`;
}

/** A "nice" round number (1/2/5 × a power of ten) at or above `value`, so axis ticks
 * land on clean numbers instead of whatever the data happens to total. */
function niceCeil(value: number): number {
  if (value <= 0) return 1;
  const exponent = Math.floor(Math.log10(value));
  const magnitude = 10 ** exponent;
  const fraction = value / magnitude;
  const niceFraction = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
  return niceFraction * magnitude;
}

function yAxisTicks(maxHours: number): { max: number; ticks: number[] } {
  if (maxHours <= 0) return { max: 1, ticks: [0, 1] };
  const step = niceCeil(maxHours / 4);
  const max = step * Math.ceil(maxHours / step);
  const ticks: number[] = [];
  for (let v = 0; v <= max + 1e-9; v += step) ticks.push(Math.round(v * 100) / 100);
  return { max, ticks };
}

/** Square baseline, rounded top corners — the "free end" of the stack, per the
 * house mark spec (4px rounded data-end, square at the baseline). */
function roundedTopRectPath(x: number, y: number, w: number, h: number): string {
  const r = Math.min(CAP_RADIUS, w / 2, Math.max(h, 0));
  if (h <= 0) return "";
  return `M${x},${y + h} V${y + r} Q${x},${y} ${x + r},${y} H${x + w - r} Q${x + w},${y} ${x + w},${y + r} V${y + h} Z`;
}

interface IntervalStatsChartProps {
  buckets: IntervalBucket[];
  granularity: Granularity;
}

export function IntervalStatsChart({ buckets, granularity }: IntervalStatsChartProps) {
  if (buckets.length === 0 || buckets.every((b) => b.tickets.length === 0 && b.breakSeconds === 0)) {
    return <p className="empty-hint">No time logged in this range.</p>;
  }

  // Rank tickets by total seconds across the whole chart, not per bucket — color and
  // stack position follow the ticket (a fixed identity), never its per-column rank,
  // so the same ticket lands in the same slot and color in every column.
  const totalsByTicket = new Map<string, { taskKey: string; seconds: number }>();
  for (const bucket of buckets) {
    for (const t of bucket.tickets) {
      const existing = totalsByTicket.get(t.taskKey);
      totalsByTicket.set(t.taskKey, { taskKey: t.taskKey, seconds: (existing?.seconds ?? 0) + t.seconds });
    }
  }
  const ranked = [...totalsByTicket.values()].sort((a, b) => b.seconds - a.seconds);
  const topKeys = ranked.slice(0, MAX_DISTINCT_TICKETS).map((t) => t.taskKey);
  const hasOther = ranked.length > MAX_DISTINCT_TICKETS;
  const colorByKey = new Map(topKeys.map((key, i) => [key, `var(${SERIES_VARS[i]})`]));

  function segmentsFor(bucket: IntervalBucket): Segment[] {
    const segments: Segment[] = [];
    if (bucket.breakSeconds > 0) {
      segments.push({ key: "__break", label: "Break", seconds: bucket.breakSeconds, color: "var(--stats-break)" });
    }
    let other = 0;
    for (const key of topKeys) {
      const seconds = bucket.tickets.find((t) => t.taskKey === key)?.seconds ?? 0;
      if (seconds > 0) segments.push({ key, label: key, seconds, color: colorByKey.get(key)! });
    }
    if (hasOther) {
      for (const t of bucket.tickets) {
        if (!topKeys.includes(t.taskKey)) other += t.seconds;
      }
      if (other > 0) segments.push({ key: "__other", label: "Other", seconds: other, color: "var(--stats-other)" });
    }
    return segments;
  }

  const maxSeconds = Math.max(
    ...buckets.map((b) => b.breakSeconds + b.tickets.reduce((sum, t) => sum + t.seconds, 0)),
    0,
  );
  const { max: maxHours, ticks } = yAxisTicks(maxSeconds / 3600);
  const pxPerSecond = PLOT_HEIGHT / (maxHours * 3600);
  const plotBottom = PLOT_TOP + PLOT_HEIGHT;

  // Widen the column slot beyond its granularity minimum if a bucket's own label
  // (e.g. a week range crossing a month boundary) wouldn't otherwise fit without
  // overlapping its neighbors — labels are measured, not assumed to fit.
  const labels = buckets.map((b) => bucketLabel(b, granularity));
  const estimatedLabelWidth = Math.max(...labels.map((l) => l.length * 6.2 + 12));
  const columnSlotWidth = Math.max(MIN_COLUMN_SLOT_WIDTH[granularity], estimatedLabelWidth);

  const chartWidth = Math.max(buckets.length * columnSlotWidth, columnSlotWidth);
  const axisGutter = 44;
  const svgWidth = chartWidth + axisGutter;
  const svgHeight = plotBottom + 28; // + room for x-axis labels

  return (
    <div className="stats-chart">
      <div className="stats-chart-scroll">
        <svg width={svgWidth} height={svgHeight} role="img" aria-label={`Time by ${granularity}`}>
          {ticks.map((hours) => {
            const y = plotBottom - hours * 3600 * pxPerSecond;
            return (
              <g key={hours}>
                <line x1={axisGutter} y1={y} x2={svgWidth} y2={y} className="stats-chart-gridline" />
                <text x={axisGutter - 6} y={y + 3} textAnchor="end" className="stats-chart-axis-label">
                  {hours}h
                </text>
              </g>
            );
          })}
          {buckets.map((bucket, i) => {
            const segments = segmentsFor(bucket);
            const x = axisGutter + i * columnSlotWidth + (columnSlotWidth - BAR_WIDTH) / 2;
            let cursor = plotBottom;
            return (
              <g key={bucket.periodStart}>
                {segments.map((seg, segIndex) => {
                  const rawHeight = seg.seconds * pxPerSecond;
                  const top = cursor - rawHeight;
                  const isOutermost = segIndex === segments.length - 1;
                  cursor = top;
                  const visibleHeight = isOutermost ? rawHeight : Math.max(rawHeight - GAP, 0);
                  return isOutermost ? (
                    <path key={seg.key} d={roundedTopRectPath(x, top, BAR_WIDTH, visibleHeight)} style={{ fill: seg.color }}>
                      <title>
                        {seg.label}: {formatDuration(seg.seconds)}
                      </title>
                    </path>
                  ) : (
                    <rect
                      key={seg.key}
                      x={x}
                      y={top + (rawHeight - visibleHeight)}
                      width={BAR_WIDTH}
                      height={visibleHeight}
                      style={{ fill: seg.color }}
                    >
                      <title>
                        {seg.label}: {formatDuration(seg.seconds)}
                      </title>
                    </rect>
                  );
                })}
                <text x={x + BAR_WIDTH / 2} y={plotBottom + 18} textAnchor="middle" className="stats-chart-axis-label">
                  {labels[i]}
                </text>
              </g>
            );
          })}
        </svg>
      </div>
      <div className="stats-legend">
        {topKeys.map((key, i) => (
          <span key={key} className="stats-legend-item">
            <span className="stats-legend-swatch" style={{ background: `var(${SERIES_VARS[i]})` }} />
            {key}
          </span>
        ))}
        {hasOther && (
          <span className="stats-legend-item">
            <span className="stats-legend-swatch" style={{ background: "var(--stats-other)" }} />
            Other
          </span>
        )}
        <span className="stats-legend-item">
          <span className="stats-legend-swatch" style={{ background: "var(--stats-break)" }} />
          Break
        </span>
      </div>
    </div>
  );
}

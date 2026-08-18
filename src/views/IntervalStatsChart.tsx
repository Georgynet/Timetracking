import { format } from "date-fns";
import { useState } from "react";
import type { Granularity, IntervalBucket } from "../api/types";
import { formatDuration } from "../lib/format";

const BREAK_KEY = "__break";
const OTHER_KEY = "__other";

// Only the first 8 tickets by total time get their own validated, distinct hue —
// beyond that, ticket identity in the *bar* is carried by the neutral "Other" color
// (the legend still names every ticket individually and each remains toggleable).
const MAX_DISTINCT_COLORS = 8;
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
  // Which tickets/"Break" the user has clicked off in the legend — hidden from both
  // the stack and the axis scale, but still listed (dimmed) so they can be toggled
  // back on.
  const [hidden, setHidden] = useState<Set<string>>(new Set());

  function toggle(key: string) {
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  /** Bulk-toggles every tail ticket (rank ≥ MAX_DISTINCT_COLORS, sharing the "Other"
   * bar color) at once: hides all of them if any is currently visible, otherwise
   * shows them all again. Each still remains individually toggleable afterward. */
  function toggleOther(tailKeys: string[]) {
    setHidden((prev) => {
      const next = new Set(prev);
      const shouldHide = tailKeys.some((k) => !next.has(k));
      for (const k of tailKeys) {
        if (shouldHide) next.add(k);
        else next.delete(k);
      }
      return next;
    });
  }

  if (buckets.length === 0 || buckets.every((b) => b.tickets.length === 0 && b.breakSeconds === 0)) {
    return <p className="empty-hint">No time logged in this range.</p>;
  }

  // Rank every ticket by total seconds across the whole chart, not per bucket —
  // color and stack position follow the ticket's identity, never its per-column
  // rank, so the same ticket lands in the same slot and color in every column.
  const totalsByTicket = new Map<string, number>();
  for (const bucket of buckets) {
    for (const t of bucket.tickets) {
      totalsByTicket.set(t.taskKey, (totalsByTicket.get(t.taskKey) ?? 0) + t.seconds);
    }
  }
  const ranked = [...totalsByTicket.entries()]
    .map(([taskKey, seconds]) => ({ taskKey, seconds }))
    .sort((a, b) => b.seconds - a.seconds);
  const rankByKey = new Map(ranked.map((t, i) => [t.taskKey, i]));
  const colorForRank = (rank: number) =>
    rank < MAX_DISTINCT_COLORS ? `var(${SERIES_VARS[rank]})` : "var(--stats-other)";
  const tailKeys = ranked.slice(MAX_DISTINCT_COLORS).map((t) => t.taskKey);
  const otherAllHidden = tailKeys.length > 0 && tailKeys.every((k) => hidden.has(k));

  function segmentsFor(bucket: IntervalBucket): Segment[] {
    const segments: Segment[] = [];
    if (bucket.breakSeconds > 0 && !hidden.has(BREAK_KEY)) {
      segments.push({ key: BREAK_KEY, label: "Break", seconds: bucket.breakSeconds, color: "var(--stats-break)" });
    }
    const visible = bucket.tickets.filter((t) => t.seconds > 0 && !hidden.has(t.taskKey));
    const distinct = visible
      .filter((t) => (rankByKey.get(t.taskKey) ?? Infinity) < MAX_DISTINCT_COLORS)
      .sort((a, b) => rankByKey.get(a.taskKey)! - rankByKey.get(b.taskKey)!);
    for (const t of distinct) {
      segments.push({ key: t.taskKey, label: t.taskKey, seconds: t.seconds, color: colorForRank(rankByKey.get(t.taskKey)!) });
    }
    const otherSeconds = visible
      .filter((t) => (rankByKey.get(t.taskKey) ?? Infinity) >= MAX_DISTINCT_COLORS)
      .reduce((sum, t) => sum + t.seconds, 0);
    if (otherSeconds > 0) {
      segments.push({ key: OTHER_KEY, label: "Other", seconds: otherSeconds, color: "var(--stats-other)" });
    }
    return segments;
  }

  const bucketSegments = buckets.map((b) => segmentsFor(b));
  const maxSeconds = Math.max(...bucketSegments.map((segs) => segs.reduce((sum, s) => sum + s.seconds, 0)), 0);
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
            const segments = bucketSegments[i];
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
        <button
          type="button"
          className={`stats-legend-item ${hidden.has(BREAK_KEY) ? "stats-legend-item-hidden" : ""}`}
          onClick={() => toggle(BREAK_KEY)}
        >
          <span className="stats-legend-swatch" style={{ background: "var(--stats-break)" }} />
          Break
        </button>
        {ranked.slice(0, MAX_DISTINCT_COLORS).map((t, i) => (
          <button
            type="button"
            key={t.taskKey}
            className={`stats-legend-item ${hidden.has(t.taskKey) ? "stats-legend-item-hidden" : ""}`}
            onClick={() => toggle(t.taskKey)}
          >
            <span className="stats-legend-swatch" style={{ background: colorForRank(i) }} />
            {t.taskKey}
          </button>
        ))}
        {tailKeys.length > 0 && (
          <button
            type="button"
            className={`stats-legend-item ${otherAllHidden ? "stats-legend-item-hidden" : ""}`}
            onClick={() => toggleOther(tailKeys)}
            title={`Show/hide the remaining ${tailKeys.length} ticket${tailKeys.length === 1 ? "" : "s"}`}
          >
            <span className="stats-legend-swatch" style={{ background: "var(--stats-other)" }} />
            Other
          </button>
        )}
      </div>
    </div>
  );
}

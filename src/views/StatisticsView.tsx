import { format, startOfMonth, startOfWeek } from "date-fns";
import { useEffect, useState } from "react";
import { getIntervalStats, getTicketStats } from "../api/commands";
import type { Granularity, IntervalBucket, SettingsDto, TicketTotal } from "../api/types";
import { IntervalStatsChart } from "./IntervalStatsChart";
import { TicketStatsTable } from "./TicketStatsTable";

type Preset = "week" | "month" | "custom";

function toDateOnly(date: Date): string {
  return format(date, "yyyy-MM-dd");
}

function rangeForPreset(preset: Preset, customFrom: string, customTo: string): { from: string; to: string } {
  const today = new Date();
  if (preset === "week") return { from: toDateOnly(startOfWeek(today, { weekStartsOn: 1 })), to: toDateOnly(today) };
  if (preset === "month") return { from: toDateOnly(startOfMonth(today)), to: toDateOnly(today) };
  return { from: customFrom, to: customTo };
}

interface StatisticsViewProps {
  settings: SettingsDto;
}

export function StatisticsView({ settings }: StatisticsViewProps) {
  const today = toDateOnly(new Date());
  const [preset, setPreset] = useState<Preset>("week");
  const [customFrom, setCustomFrom] = useState(today);
  const [customTo, setCustomTo] = useState(today);
  const [granularity, setGranularity] = useState<Granularity>("day");
  const [ticketTotals, setTicketTotals] = useState<TicketTotal[]>([]);
  const [buckets, setBuckets] = useState<IntervalBucket[]>([]);
  const [error, setError] = useState<string | null>(null);

  const { from, to } = rangeForPreset(preset, customFrom, customTo);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [totals, intervalBuckets] = await Promise.all([
          getTicketStats({ from, to }),
          getIntervalStats({ from, to, granularity }),
        ]);
        if (cancelled) return;
        setTicketTotals(totals);
        setBuckets(intervalBuckets);
        setError(null);
      } catch (err) {
        if (!cancelled) setError(err as string);
      }
    }
    load();
    return () => {
      cancelled = true;
    };
  }, [from, to, granularity]);

  return (
    <div className="stats-view">
      <div className="stats-controls">
        <div className="stats-preset-group">
          <button className={preset === "week" ? "active" : ""} onClick={() => setPreset("week")}>
            This Week
          </button>
          <button className={preset === "month" ? "active" : ""} onClick={() => setPreset("month")}>
            This Month
          </button>
          <button className={preset === "custom" ? "active" : ""} onClick={() => setPreset("custom")}>
            Custom
          </button>
          {preset === "custom" && (
            <>
              <input type="date" value={customFrom} max={customTo} onChange={(e) => setCustomFrom(e.target.value)} />
              <span>to</span>
              <input type="date" value={customTo} min={customFrom} onChange={(e) => setCustomTo(e.target.value)} />
            </>
          )}
        </div>
        <div className="stats-granularity-group">
          {(["day", "week", "month"] as const).map((g) => (
            <button key={g} className={granularity === g ? "active" : ""} onClick={() => setGranularity(g)}>
              {g === "day" ? "Day" : g === "week" ? "Week" : "Month"}
            </button>
          ))}
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      <section className="panel">
        <div className="panel-header">
          <h2>Time by {granularity}</h2>
        </div>
        <IntervalStatsChart buckets={buckets} granularity={granularity} />
      </section>

      <section className="panel">
        <div className="panel-header">
          <h2>Time by ticket</h2>
        </div>
        <TicketStatsTable totals={ticketTotals} jiraBaseUrl={settings.jiraBaseUrl ?? ""} />
      </section>
    </div>
  );
}

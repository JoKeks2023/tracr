import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { dayLabel, duration, estimate, hhmm, intervals } from "./format";
import { t } from "./i18n";
import { Bars, CostLine } from "./Overview";
import { SessionRow, agentMs } from "./SessionRow";
import type { DayAgents } from "./types";

const HOUR = 3_600_000;

/** Your day on one axis: what counted or is unresolved, and when agents ran. No lanes of sensors. */
function DayStrip({ view }: { view: DayAgents }) {
  const human = view.human.atoms.filter((a) => a.status !== "non_work");
  const runs = view.sessions.flatMap((s) => s.actors.flatMap((a) => a.segments.map((g) => [g.start, g.end] as const)));
  const times = [...human.flatMap((a) => [a.start, a.end]), ...runs.flat()];
  if (times.length === 0) return null;
  const start = Math.floor(Math.min(...times) / HOUR) * HOUR;
  const end = Math.max(Math.ceil(Math.max(...times) / HOUR) * HOUR, start + HOUR);
  const pos = (ms: number) => `${(((ms - start) / (end - start)) * 100).toFixed(2)}%`;
  const w = (a: number, b: number) => `${Math.max(0.4, ((b - a) / (end - start)) * 100).toFixed(2)}%`;
  const count = Math.min(7, Math.round((end - start) / HOUR) + 1);
  const ticks = Array.from({ length: count }, (_, i) => start + (i * (end - start)) / Math.max(1, count - 1));
  const tz = view.timezone;
  return (
    <section className="strip" aria-label={t("todayStrip")}>
      <h2 className="section-title">{t("todayStrip")}</h2>
      <div className="strip-lane">
        <span className="strip-label">{t("stripYou")}</span>
        <div className="strip-track">
          {human.map((a, i) => (
            <i
              key={i}
              className={`strip-seg ${a.status === "work" ? "counted" : "unclear"}`}
              style={{ left: pos(a.start), width: w(a.start, a.end) }}
              title={`${a.status === "work" ? t("stripCounted") : t("stripUnclear")} · ${hhmm(a.start, tz)}–${hhmm(a.end, tz)} · ${duration(a.end - a.start)}`}
            />
          ))}
        </div>
      </div>
      <div className="strip-lane">
        <span className="strip-label">{t("stripAgents")}</span>
        <div className="strip-track">
          {runs.map(([a, b], i) => (
            <i key={i} className="strip-seg agent" style={{ left: pos(a), width: w(a, b) }} title={`${hhmm(a, tz)}–${hhmm(b, tz)}`} />
          ))}
        </div>
      </div>
      <div className="strip-lane axis">
        <span className="strip-label" />
        <div className="strip-track ticks">
          {ticks.map((tk) => (
            <span key={tk} className="mono" style={{ left: pos(tk) }}>
              {hhmm(tk, tz)}
            </span>
          ))}
        </div>
      </div>
      <p className="note legend-line">
        <span><i className="swatch counted" aria-hidden /> {t("stripCounted")}</span>
        <span><i className="swatch unclear" aria-hidden /> {t("stripUnclear")}</span>
        <span><i className="swatch agent" aria-hidden /> {t("stripAgents")}</span>
      </p>
    </section>
  );
}

function Fact({ label, value, note, tone }: { label: string; value: string; note?: string; tone?: "warn" | "muted" }) {
  return (
    <div className={`fact ${tone ?? ""}`}>
      <dt>{label}</dt>
      <dd>{value}</dd>
      {note && <span className="fact-note">{note}</span>}
    </div>
  );
}

/** The start page: how the day stands, what needs you, what the agents did. */
export function Today({
  day,
  hasData,
  reloadKey,
  onOpenSession,
  onOpenWorktime,
  onOpenSessions,
  onError,
}: {
  day: string;
  hasData: boolean;
  reloadKey: unknown;
  onOpenSession: (id: string) => void;
  onOpenWorktime: () => void;
  onOpenSessions: () => void;
  onError: (text: string) => void;
}) {
  const [view, setView] = useState<DayAgents | null>(null);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    if (!hasData) {
      setView(null);
      setLoaded(true);
      return;
    }
    let live = true;
    api
      .dayAgents(day)
      .then((v) => live && (setView(v), setLoaded(true)))
      .catch((e) => live && (setLoaded(true), onError(errorText(e))));
    return () => {
      live = false;
    };
  }, [day, hasData, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const summary = useMemo(() => {
    if (!view) return null;
    const reviewed = view.review?.snapshot;
    const counted = reviewed ? reviewed.counted_ms : view.human.counted_ms;
    const unknown = reviewed ? reviewed.unknown_ms : view.human.unknown_ms;
    const windows = view.human.atoms.filter((a) => a.status === "unknown").length;
    const latest = [...view.sessions].sort((a, b) => (b.last_evidence_at ?? 0) - (a.last_evidence_at ?? 0));
    return { counted, unknown, windows, agentMs: view.sessions.reduce((n, s) => n + agentMs(s), 0), latest };
  }, [view]);

  if (!loaded) return <div className="today" aria-busy="true" />;
  if (!view || !summary) {
    return (
      <div className="empty">
        <h2>{t("nothingToday")}</h2>
        <p className="muted">{t("nothingTodayHint")}</p>
      </div>
    );
  }
  const shown = summary.latest.slice(0, 6);
  const pricing = view.cost.pricing;
  const byProject = view.counted_by_project.length > 0 || Object.keys(view.human.counted_by_target).length > 0;
  return (
    <div className="today">
      <p className="today-date muted">{dayLabel(day)}</p>
      <dl className="facts-strip">
        <Fact label={t("todayCounted")} value={duration(summary.counted)} note={view.review ? t("reviewed") : t("provisional")} />
        <Fact label={t("todayUnclear")} value={duration(summary.unknown)} tone={summary.unknown > 0 ? "warn" : "muted"} note={summary.windows > 0 ? intervals(summary.windows) : undefined} />
        <Fact label={t("todayAgents")} value={duration(summary.agentMs)} note={t("sessionsToday", { n: view.sessions.length })} />
        <Fact
          label={t("todayCost")}
          value={view.cost.identified.amount_nano === null ? "—" : estimate(view.cost.identified, pricing.currency)}
          note={view.cost.identified.amount_nano === null ? t("unpricedWord") : t("todayCostNote")}
          tone={view.cost.identified.amount_nano === null ? "muted" : undefined}
        />
      </dl>

      {summary.unknown > 0 && (
        <section className="needs" aria-label={t("todayNeedsYou")}>
          <h2 className="section-title">{t("todayNeedsYou")}</h2>
          <div className="needs-row">
            <span className="needs-dot" aria-hidden />
            <span>{t("unclearRow", { time: duration(summary.unknown), intervals: intervals(summary.windows) })}</span>
            <span className="spacer" />
            <button className="button secondary small" onClick={onOpenWorktime}>
              {t("resolve")}
            </button>
          </div>
        </section>
      )}

      <DayStrip view={view} />

      <section>
        <div className="section-head">
          <h2 className="section-title">{t("todaySessions")}</h2>
          {view.sessions.length > shown.length && (
            <button className="button ghost small" onClick={onOpenSessions}>
              {t("moreSessions", { n: view.sessions.length })}
            </button>
          )}
        </div>
        <div className="session-group">
          {shown.map((s) => (
            <SessionRow key={s.session_id} s={s} pricing={pricing} tz={view.timezone} selected={false} onOpen={() => onOpenSession(s.session_id)} />
          ))}
        </div>
      </section>

      {byProject && summary.counted > 0 && <Bars view={view} human={view.human} />}
      <CostLine cost={view.cost} />
    </div>
  );
}

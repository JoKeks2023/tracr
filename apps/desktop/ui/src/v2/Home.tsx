import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../api";
import { ago, mergeCost, sessionsN, subagentsN, compact, dayLabel, duration, estimate, hhmm, intervals, modelName, providerName, sessionTokens, spanLabel } from "../format";
import { t } from "../i18n";
import { shiftAnchor } from "../nav";
import type { DayAgents, HooksView, Limits, LiveSession, SessionDay, SessionList } from "../types";
import { useQuotaPrefs } from "./quota";
import { Panel, QuotaBar, QuotaRings, QuotaViewSwitch, Stat, clockLabel, windowName } from "./ui";

const HOUR = 3_600_000;
const ACTIVE_MS = 15 * 60_000;

/** Your day on one axis: what counted or is unresolved, and when agents ran. */
function DayStrip({ view }: { view: DayAgents }) {
  const human = view.human.atoms.filter((a) => a.status !== "non_work");
  const runs = view.sessions.flatMap((s) => s.actors.flatMap((a) => a.segments.map((g) => [g.start, g.end] as const)));
  const times = [...human.flatMap((a) => [a.start, a.end]), ...runs.flat()];
  if (times.length === 0) return null;
  const start = Math.floor(Math.min(...times) / HOUR) * HOUR;
  const end = Math.max(Math.ceil(Math.max(...times) / HOUR) * HOUR, start + HOUR);
  const pos = (ms: number) => `${(((ms - start) / (end - start)) * 100).toFixed(2)}%`;
  const wid = (a: number, b: number) => `${Math.max(0.3, ((b - a) / (end - start)) * 100).toFixed(2)}%`;
  const count = Math.min(7, Math.round((end - start) / HOUR) + 1);
  const ticks = Array.from({ length: count }, (_, i) => start + (i * (end - start)) / Math.max(1, count - 1));
  const tz = view.timezone;
  return (
    <Panel
      title={t("todayStrip")}
      aside={
        <span className="vlegend">
          <span><i className="sw" style={{ background: "var(--acc)" }} />{t("stripCounted")}</span>
          <span><i className="sw" style={{ background: "var(--warn)" }} />{t("stripUnclear")}</span>
          <span><i className="sw" style={{ background: "var(--dim)" }} />{t("stripAgents")}</span>
        </span>
      }
    >
      <div className="daystrip">
        <div className="dlane">
          <span className="vnote">{t("stripYou")}</span>
          <div className="tr">
            {human.map((a, i) => (
              <i key={i} style={{ left: pos(a.start), width: wid(a.start, a.end), background: a.status === "work" ? "var(--acc)" : "var(--warn)" }} title={`${a.status === "work" ? t("stripCounted") : t("stripUnclear")} · ${hhmm(a.start, tz)}–${hhmm(a.end, tz)} · ${duration(a.end - a.start)}`} />
            ))}
          </div>
        </div>
        <div className="dlane">
          <span className="vnote">{t("stripAgents")}</span>
          <div className="tr">
            {runs.map(([a, b], i) => (
              <i key={i} style={{ left: pos(a), width: wid(a, b), background: "var(--dim)" }} title={`${hhmm(a, tz)}–${hhmm(b, tz)}`} />
            ))}
          </div>
        </div>
        <div className="ticks">
          <i />
          <div className="mono">
            {ticks.map((tk) => (
              <span key={tk} style={{ left: pos(tk) }}>{hhmm(tk, tz)}</span>
            ))}
          </div>
        </div>
      </div>
    </Panel>
  );
}

function SessionCard({ s, view, now, live, onOpen }: { s: SessionDay; view: DayAgents; now: number; live?: LiveSession; onOpen: () => void }) {
  const models = [...new Set(s.usage.filter((u) => u.identified).map((u) => modelName(u.model)))];
  const subs = s.actors.filter((a) => a.actor_kind === "known").length;
  const run = s.actors.flatMap((a) => a.segments);
  const first = run.length ? Math.min(...run.map((g) => g.start)) : s.first_evidence_at;
  const last = s.last_evidence_at ?? first;
  const agentMs = s.actors.reduce((n, a) => n + a.execution_ms, 0);
  return (
    <button className="card" onClick={onOpen}>
      <div className="t1">
        <b>{s.label ?? t("noWorkingDir")}</b>
        <span className="mono vnote">{live?.state === "waiting" ? t("liveWaiting", { time: ago(live.since, now) }) : live?.state === "your_turn" ? t("liveYourTurn", { time: ago(live.since, now) }) : last ? ago(last, now) : ""}</span>
      </div>
      <div className="chips">
        {s.git_branch && <span className="chip mono">{s.git_branch}</span>}
        <span className="chip">{providerName(s.provider)}</span>
        {models.slice(0, 2).map((m) => (
          <span key={m} className="chip">{m}</span>
        ))}
        {subs > 0 && <span className="chip">{subagentsN(subs)}</span>}
      </div>
      <div className="meters">
        <span>{first && last ? spanLabel(first, last, view.timezone) : ""}</span>
        <span>{duration(agentMs)}</span>
        <span>{compact(sessionTokens(s.usage))} {t("metaTokens")}</span>
        <span className="grow" />
        <span className="mono">{s.cost.amount_nano === null ? "—" : estimate(s.cost, view.cost.pricing.currency)}</span>
      </div>
    </button>
  );
}

/** The start page: how the day stands, the quota, what needs you, what the agents are doing. */
export function Home({
  day,
  hasData,
  reloadKey,
  limits,
  onOpenSession,
  onOpenWorktime,
  onOpenLimits,
  onOpenSettings,
  onError,
}: {
  day: string;
  hasData: boolean;
  reloadKey: unknown;
  limits: Limits | null;
  onOpenSession: (id: string) => void;
  onOpenWorktime: () => void;
  onOpenLimits: () => void;
  onOpenSettings: () => void;
  onError: (text: string) => void;
}) {
  const { prefs } = useQuotaPrefs();
  const [raw, setRaw] = useState<DayAgents | null>(null);
  const [loaded, setLoaded] = useState(false);
  // Automated sessions (e.g. Codex's auto-review) are not work you do: left out unless asked for.
  const view = useMemo<DayAgents | null>(() => {
    if (!raw || prefs.showAutomated) return raw;
    const sessions = raw.sessions.filter((s) => !s.automated);
    return { ...raw, sessions, cost: { ...raw.cost, identified: mergeCost(sessions.map((s) => s.cost)) } };
  }, [raw, prefs.showAutomated]);
  const [now, setNow] = useState(Date.now());
  const [yesterday, setYesterday] = useState<SessionList | null>(null);
  const [hooks, setHooks] = useState<HooksView | null>(null);
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(id);
  }, []);
  useEffect(() => {
    if (!hasData) {
      setRaw(null);
      setLoaded(true);
      return;
    }
    let live = true;
    api
      .dayAgents(day)
      .then((v) => live && (setRaw(v), setLoaded(true)))
      .catch((e) => live && (setLoaded(true), onError(errorText(e))));
    return () => {
      live = false;
    };
  }, [day, hasData, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  // Who is working, who needs you: every few seconds once the hooks are on.
  useEffect(() => {
    const read = () => void api.hooksState().then(setHooks).catch(() => setHooks(null));
    read();
    const id = window.setInterval(read, 5_000);
    return () => window.clearInterval(id);
  }, [reloadKey]);

  useEffect(() => {
    let live = true;
    const y = shiftAnchor("day", day, -1);
    api
      .sessions(y, y)
      .then((l) => live && setYesterday(l))
      .catch(() => live && setYesterday(null));
    return () => {
      live = false;
    };
  }, [day, reloadKey]);

  const sum = useMemo(() => {
    if (!view) return null;
    const snap = view.review?.snapshot;
    const windows = view.human.atoms.filter((a) => a.status === "unknown").length;
    const sessions = [...view.sessions].sort((a, b) => (b.last_evidence_at ?? 0) - (a.last_evidence_at ?? 0));
    return {
      counted: snap ? snap.counted_ms : view.human.counted_ms,
      unknown: snap ? snap.unknown_ms : view.human.unknown_ms,
      windows,
      agentMs: sessions.reduce((n, s) => n + s.actors.reduce((m, a) => m + a.execution_ms, 0), 0),
      sessions,
    };
  }, [view]);

  if (!loaded) return <div className="body" aria-busy="true" />;
  if (!view || !sum) {
    return (
      <div className="body">
        <div className="vempty">
          <h2>{t("nothingToday")}</h2>
          <p>{t("nothingTodayHint")}</p>
        </div>
      </div>
    );
  }

  const tz = view.timezone;
  // Tightest first: what runs out soonest is what you look at first.
  const windows = [...(limits?.windows ?? [])].sort((a, b) => Number(a.expired) - Number(b.expired) || b.used_percent - a.used_percent);
  const reached = windows.filter((w) => !w.expired && w.used_percent >= 100);
  const hasClaude = view.sessions.some((s) => s.provider === "claude_code");
  const needs: { key: string; color: string; title: string; note: string; action: string; run: () => void }[] = [];
  if (sum.unknown > 0) {
    needs.push({ key: "unclear", color: "var(--warn)", title: t("unclearTitle", { time: duration(sum.unknown) }), note: intervals(sum.windows), action: t("needResolve"), run: onOpenWorktime });
  }
  for (const w of reached) {
    needs.push({ key: `lim-${w.provider}-${w.window}`, color: "var(--bad)", title: t("needLimit", { name: windowName(w) }), note: w.resets_at ? t("needLimitNote", { time: clockLabel(w.resets_at, tz) }) : "", action: t("needOpen"), run: onOpenLimits });
  }
  if (hasClaude && limits && !limits.claude_tap.installed) {
    needs.push({ key: "tap", color: "var(--info)", title: t("needTap"), note: t("needTapNote"), action: t("needOpen"), run: onOpenLimits });
  }
  if (hasClaude && hooks && !hooks.installed) {
    needs.push({ key: "hooks", color: "var(--info)", title: t("needHooks"), note: t("needHooksNote"), action: t("needOpen"), run: onOpenSettings });
  }
  const liveBy = new Map<string, LiveSession>((hooks?.installed ? hooks.live : []).map((l) => [l.session, l]));
  const liveOf = (s: SessionDay) => liveBy.get(s.native_session_id);
  const waiting = sum.sessions.filter((s) => ["waiting", "your_turn"].includes(liveOf(s)?.state ?? ""));
  const isWorking = (s: SessionDay) => {
    const l = liveOf(s);
    return l ? l.state === "working" || l.state === "started" : (s.last_evidence_at ?? 0) >= now - ACTIVE_MS;
  };
  const active = sum.sessions.filter((s) => !waiting.includes(s) && isWorking(s));
  const earlier = sum.sessions.filter((s) => !waiting.includes(s) && !isWorking(s)).slice(0, 8);
  const cost = view.cost.identified;

  return (
    <div className="body">
      <div className="stats">
        <Stat label={t("todayCounted")} value={duration(sum.counted)} note={view.review ? t("reviewed") : t("provisional")} />
        <Stat label={t("todayUnclear")} value={duration(sum.unknown)} tone={sum.unknown > 0 ? "warn" : "muted"} note={sum.windows > 0 ? intervals(sum.windows) : undefined} />
        <Stat label={t("todayAgents")} value={duration(sum.agentMs)} note={sessionsN(view.sessions.length)} />
        <Stat label={t("statCost")} value={cost.amount_nano === null ? "—" : estimate(cost, view.cost.pricing.currency)} note={[cost.amount_nano === null ? t("unpricedWord") : t("todayCostNote"), yesterday && yesterday.sessions.length > 0 ? t("yesterdayCost", { x: yesterday.cost.amount_nano === null ? "—" : estimate(yesterday.cost, yesterday.pricing.currency) }) : ""].filter(Boolean).join(" · ")} tone={cost.amount_nano === null ? "muted" : undefined} />
      </div>

      <div className="row2">
        <Panel title={t("quotaTitle")} aside={windows.length > 0 ? <QuotaViewSwitch /> : t("quotaStated")}>
          {windows.length === 0 ? (
            <p className="vnote">
              {t("quotaNone")}. {t("quotaNoneHint")}
            </p>
          ) : (
            prefs.rings ? (
              <QuotaRings windows={windows} />
            ) : (
              <div className="quota">
                {windows.map((w) => (
                  <QuotaBar key={`${w.provider}-${w.window}`} w={w} />
                ))}
              </div>
            )
          )}
        </Panel>
        <Panel title={t("todayNeedsYou")} aside={needs.length > 0 ? String(needs.length) : undefined}>
          {needs.length === 0 && <p className="vnote">{t("needNone")}</p>}
          {needs.map((n) => (
            <div key={n.key} className="need">
              <i className="sdot" style={{ background: n.color }} />
              <div className="t">
                <b>{n.title}</b>
                <div className="vnote">{n.note}</div>
              </div>
              <button className="btn s" onClick={n.run}>{n.action}</button>
            </div>
          ))}
        </Panel>
      </div>

      <div className="board">
        {hooks?.installed && (
          <section className="col" aria-label={t("colWaiting")}>
            <h2><i className="sdot" style={{ background: "var(--bad)" }} />{t("colWaiting")} <em>{waiting.length}</em></h2>
            {waiting.map((s) => <SessionCard key={s.session_id} s={s} view={view} now={now} live={liveOf(s)} onOpen={() => onOpenSession(s.session_id)} />)}
            {waiting.length === 0 && <p className="vnote">{t("needNone")}</p>}
          </section>
        )}
        <section className="col" aria-label={t("colActive")}>
          <h2><i className="sdot collecting" />{t("colActive")} <em>{active.length}</em></h2>
          {active.map((s) => <SessionCard key={s.session_id} s={s} view={view} now={now} live={liveOf(s)} onOpen={() => onOpenSession(s.session_id)} />)}
          {active.length === 0 && <p className="vnote">{t("boardNone")}</p>}
        </section>
        <section className="col" aria-label={t("colEarlier")}>
          <h2><i className="sdot" />{t("colEarlier")} <em>{sum.sessions.length - active.length}</em></h2>
          {earlier.map((s) => <SessionCard key={s.session_id} s={s} view={view} now={now} onOpen={() => onOpenSession(s.session_id)} />)}
        </section>
      </div>

      <DayStrip view={view} />
      <p className="vnote">{dayLabel(day)} · {tz}</p>
    </div>
  );
}

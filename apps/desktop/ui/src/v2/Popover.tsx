import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { ago, duration, estimate, mergeCost, providerName } from "../format";
import { locale, t } from "../i18n";
import { isoToday } from "../nav";
import type { DayAgents, Fleet, HooksView, Limits, LimitWindow, SessionDay, Status } from "../types";
import { levelOf, useNow, useQuotaPrefs } from "./quota";
import { clockLabel, spanText } from "./ui";

/** How often the popover reads again while it is open. */
const POLL_MS = 5_000;

const go = (action: string) => void invoke("popover_go", { action }).catch(() => {});

/** The tightest window of each provider first, its other windows after it. */
function byProvider(windows: LimitWindow[]): LimitWindow[][] {
  const m = new Map<string, LimitWindow[]>();
  for (const w of windows) m.set(w.provider, [...(m.get(w.provider) ?? []), w]);
  return [...m.values()].map((ws) => [...ws].sort((a, b) => Number(a.expired) - Number(b.expired) || b.used_percent - a.used_percent));
}

function span(w: LimitWindow): string {
  return w.window === "five_hour" ? t("wFive") : w.window === "weekly" ? t("wWeek") : t("wMinutes", { n: w.window_minutes ?? "?" });
}

/** One provider: a small ring for its tightest window, the window and reset beside it, the others as lines. */
function LimitRow({ ws, now }: { ws: LimitWindow[]; now: number }) {
  const { prefs } = useQuotaPrefs();
  const [w, ...rest] = ws;
  const used = Math.max(0, Math.min(100, w.used_percent));
  const shown = (x: LimitWindow) => Math.round(prefs.remaining ? 100 - x.used_percent : x.used_percent);
  const level = levelOf(w, prefs.paceAware);
  const color = level === "ok" ? "var(--acc)" : level === "warn" ? "var(--warn)" : "var(--bad)";
  const r = 15;
  const c = 2 * Math.PI * r;
  const what = prefs.remaining ? t("quotaLeft") : t("quotaUsed");
  return (
    <button className="pv-row" onClick={() => go("area_limits")} aria-label={`${providerName(w.provider)} ${span(w)}: ${shown(w)} % ${what}`}>
      <svg viewBox="0 0 40 40" width="36" height="36" aria-hidden="true" className="pv-ring">
        <circle cx="20" cy="20" r={r} fill="none" stroke="var(--track)" strokeWidth="4.5" />
        <circle cx="20" cy="20" r={r} fill="none" stroke={color} strokeWidth="4.5" strokeLinecap="round" strokeDasharray={`${(used / 100) * c} ${c}`} transform="rotate(-90 20 20)" opacity={w.expired ? 0.35 : 1} />
      </svg>
      <span className="pv-main">
        <span className="pv-l1">
          <b>{providerName(w.provider)}</b>
          <span className="mono pv-fig" style={level === "ok" ? undefined : { color }}>{w.expired ? "—" : `${shown(w)} %`}</span>
        </span>
        <span className="pv-l2">
          <span>{span(w)} · {what}</span>
          {w.resets_at && !w.expired && <span className="mono">{t("pvResets", { time: spanText(w.resets_at - now) })}</span>}
        </span>
        {rest.map((o) => (
          <span key={o.window} className="pv-l2">
            <span>{span(o)}</span>
            <span className="mono">{o.expired ? "—" : `${shown(o)} %`}{o.resets_at && !o.expired ? ` · ${clockLabel(o.resets_at)}` : ""}</span>
          </span>
        ))}
      </span>
    </button>
  );
}

function sessionName(s: SessionDay): string {
  return s.label || s.repository_label || (s.cwd ? s.cwd.split("/").filter(Boolean).pop() ?? "" : "") || providerName(s.provider);
}

function Section({ title, aside, children }: { title: string; aside?: string; children: ReactNode }) {
  return (
    <section className="pv-sec" aria-label={title}>
      <h2>
        {title}
        {aside && <em>{aside}</em>}
      </h2>
      {children}
    </section>
  );
}

/** The menu bar popover (T-0019): quota, who needs you, who works, today, the timer. */
export function Popover() {
  const { prefs } = useQuotaPrefs();
  const now = useNow(15_000);
  const [status, setStatus] = useState<Status | null>(null);
  const [limits, setLimits] = useState<Limits | null>(null);
  const [day, setDay] = useState<DayAgents | null>(null);
  const [hooks, setHooks] = useState<HooksView | null>(null);
  const [fleet, setFleet] = useState<Fleet | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [timerBusy, setTimerBusy] = useState(false);
  const [menu, setMenu] = useState(false);
  const [, tick] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  const read = useCallback(async () => {
    const today = isoToday();
    const [st, li, d, h, f] = await Promise.allSettled([api.status(), api.limits(), api.dayAgents(today), api.hooksState(), api.fleetState()]);
    if (st.status === "fulfilled") setStatus(st.value);
    if (li.status === "fulfilled") setLimits(li.value);
    setDay(d.status === "fulfilled" ? d.value : null);
    setHooks(h.status === "fulfilled" ? h.value : null);
    setFleet(f.status === "fulfilled" ? f.value : null);
    setLoaded(true);
  }, []);

  // Read when shown, then every few seconds while visible.
  useEffect(() => {
    void read();
    let id: number | undefined;
    const start = () => {
      window.clearInterval(id);
      id = window.setInterval(() => void read(), POLL_MS);
    };
    const stop = () => window.clearInterval(id);
    start();
    const shown = listen("popover-shown", () => (setMenu(false), void read(), start()));
    const onVis = () => (document.hidden ? stop() : start());
    document.addEventListener("visibilitychange", onVis);
    window.addEventListener("blur", () => setMenu(false));
    return () => {
      stop();
      document.removeEventListener("visibilitychange", onVis);
      void shown.then((f) => f());
    };
  }, [read]);

  // The running timer ticks every second.
  const running = status?.running_timer ?? null;
  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => tick((n) => n + 1), 1000);
    return () => window.clearInterval(id);
  }, [running]);

  // Keys: Esc closes, ⏎ opens the app, ⌘R reads again, ⌘, settings, ⌘Q quits.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") return menu ? setMenu(false) : void invoke("popover_go", { action: "hide" }).catch(() => {});
      if (e.metaKey && e.key === "r") return (e.preventDefault(), void read());
      if (e.metaKey && e.key === ",") return (e.preventDefault(), go("settings"));
      if (e.metaKey && e.key === "q") return (e.preventDefault(), go("quit"));
      if (e.key === "Enter" && (document.activeElement === document.body || document.activeElement === root.current)) go("open");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [menu, read]);

  // The window fits the page.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const fit = () => void invoke("popover_fit", { height: Math.ceil(el.scrollHeight) }).catch(() => {});
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [loaded]);

  const view = useMemo(() => {
    if (!day) return null;
    const sessions = prefs.showAutomated ? day.sessions : day.sessions.filter((s) => !s.automated);
    return { ...day, sessions, cost: { ...day.cost, identified: mergeCost(sessions.map((s) => s.cost)) } };
  }, [day, prefs.showAutomated]);

  const live = new Map((hooks?.installed ? hooks.live : []).map((l) => [l.session, l]));
  const reg = new Map((fleet?.entries ?? []).map((e) => [e.native_session_id, e]));
  const stat = new Map((fleet?.stats ?? []).map((x) => [x.native_session_id, x]));
  const registry = !!fleet?.registry_available;
  const sessions = view ? [...view.sessions].sort((a, b) => (b.last_evidence_at ?? 0) - (a.last_evidence_at ?? 0)) : [];
  const waiting = sessions.filter((s) => ["waiting", "your_turn"].includes(live.get(s.native_session_id)?.state ?? ""));
  const working = sessions.filter((s) => {
    if (waiting.includes(s)) return false;
    const l = live.get(s.native_session_id);
    if (l) return l.state === "working" || l.state === "started";
    const r = reg.get(s.native_session_id);
    if (registry && r) return r.alive && r.status === "busy";
    return !registry && (s.last_evidence_at ?? 0) >= now - 15 * 60_000;
  });

  const counted = view ? (view.review?.snapshot?.counted_ms ?? view.human.counted_ms) : 0;
  const unknown = view ? (view.review?.snapshot?.unknown_ms ?? view.human.unknown_ms) : 0;
  const agentMs = sessions.reduce((n, s) => n + s.actors.reduce((m, a) => m + a.execution_ms, 0), 0);
  const cost = view?.cost.identified;
  const providers = byProvider(limits?.windows ?? []);
  const collecting = status?.collector === "collecting";
  const dateText = new Intl.DateTimeFormat(locale, { weekday: "short", day: "numeric", month: "short" }).format(now);
  const timerText = running ? (() => {
    const s = Math.max(0, Math.floor((Date.now() - running.start) / 1000));
    return `${Math.floor(s / 3600)}:${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  })() : null;

  // Waits for the collector, so the button stays disabled until the timer
  // really started or stopped: a second click can no longer toggle twice
  // or seem to be ignored while the collector is busy (GH #33).
  const toggleTimer = async () => {
    setTimerBusy(true);
    try {
      await api.manual({ op: running ? "stop_timer" : "start_timer" });
    } catch {
      /* the next read shows the state */
    }
    await read();
    setTimerBusy(false);
  };

  const refresh = async () => {
    setBusy(true);
    try {
      if (collecting) await api.refresh();
    } catch {
      /* the next read shows the state */
    }
    await read();
    setBusy(false);
  };

  return (
    <div className="v2 pv" ref={root} tabIndex={-1}>
      <header className="pv-head">
        <span className={`sdot ${status?.collector ?? ""}`} title={collecting ? t("pvRecording") : t("pvNotRecording")} aria-hidden="true" />
        <b>tracr</b>
        <span className="pv-sub">{collecting ? t("pvRecording") : t("pvNotRecording")} · {dateText}</span>
        <span className="grow" />
        <button className="pv-icon" onClick={() => void refresh()} disabled={busy} title={`${t("refresh")} (⌘R)`} aria-label={t("refresh")}>
          <svg viewBox="0 0 16 16" aria-hidden="true" className={busy ? "spin" : ""}><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v3h-3" /></svg>
        </button>
      </header>

      {!loaded ? (
        <p className="pv-empty">{t("startingUp")}</p>
      ) : (
        <>
          <Section title={t("pvLimits")}>
            {providers.length === 0 ? (
              <button className="pv-row pv-link" onClick={() => go("area_limits")}>{t("pvNoLimits")}</button>
            ) : (
              providers.map((ws) => <LimitRow key={ws[0].provider} ws={ws} now={now} />)
            )}
          </Section>

          {waiting.length > 0 && (
            <Section title={t("pvNeedsYou")} aside={String(waiting.length)}>
              {waiting.map((s) => {
                const l = live.get(s.native_session_id)!;
                const st = stat.get(s.native_session_id);
                return (
                  <button key={s.session_id} className="pv-row pv-sess" onClick={() => go(`session:${s.session_id}`)}>
                    <span className="pv-dot wait" aria-hidden="true" />
                    <span className="pv-main">
                      <span className="pv-l1"><b>{sessionName(s)}</b><span className="mono">{ago(l.since, now)}</span></span>
                      <span className="pv-l2">
                        <span>{l.state === "your_turn" ? t("pvYourTurn") : t("pvWaiting")}</span>
                        {st && st.cache_ttl_left_ms > 0 && <span className="mono">{t("pvCache", { time: spanText(st.cache_ttl_left_ms) })}</span>}
                      </span>
                    </span>
                  </button>
                );
              })}
            </Section>
          )}

          <Section title={t("pvWorking")} aside={working.length > 0 ? String(working.length) : undefined}>
            {working.length === 0 ? (
              <p className="pv-empty">{t("pvNobody")}</p>
            ) : (
              working.slice(0, 3).map((s) => (
                <button key={s.session_id} className="pv-row pv-sess" onClick={() => go(`session:${s.session_id}`)}>
                  <span className="pv-dot work" aria-hidden="true" />
                  <span className="pv-main">
                    <span className="pv-l1"><b>{sessionName(s)}</b><span className="mono">{providerName(s.provider)}</span></span>
                  </span>
                </button>
              ))
            )}
            {working.length > 3 && <button className="pv-more" onClick={() => go("area_today")}>{t("pvMore", { n: working.length - 3 })}</button>}
          </Section>

          <Section title={t("pvToday")}>
            <button className="pv-figs" onClick={() => go("area_today")} aria-label={t("pvOpenToday")}>
              <span><small>{t("todayCounted")}</small><b className="mono">{duration(counted)}</b></span>
              <span><small>{t("todayUnclear")}</small><b className={`mono ${unknown > 0 ? "w" : "m"}`}>{duration(unknown)}</b></span>
              <span><small>{t("todayAgents")}</small><b className="mono">{duration(agentMs)}</b></span>
              <span title={t("pvCostNote")}><small>{t("pvCost")}</small><b className="mono">{!cost || cost.amount_nano === null ? "—" : `≈ ${estimate(cost, view!.cost.pricing.currency)}`}</b></span>
            </button>
          </Section>
        </>
      )}

      <footer className="pv-foot">
        <button className={`btn ${running ? "pv-running" : ""}`} onClick={() => void toggleTimer()} disabled={!collecting || timerBusy} aria-busy={timerBusy} title={collecting ? undefined : t("pvNotRecording")}>
          {running ? <>■ <span className="mono">{timerText}</span></> : <>▶ {t("pvStartTimer")}</>}
        </button>
        <span className="grow" />
        <button className="btn p" onClick={() => go("open")}>{t("pvOpen")}</button>
        <span className="pv-menuwrap">
          <button className="pv-icon" aria-haspopup="menu" aria-expanded={menu} aria-label={t("pvMoreActions")} onClick={() => setMenu((m) => !m)}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="3.5" cy="8" r="1.2" /><circle cx="8" cy="8" r="1.2" /><circle cx="12.5" cy="8" r="1.2" /></svg>
          </button>
          {menu && (
            <span className="pv-menu" role="menu">
              <button role="menuitem" onClick={() => go("settings")}>{t("settings")}<kbd>⌘,</kbd></button>
              <button role="menuitem" onClick={() => go("area_limits")}>{t("pvLimits")}</button>
              <hr />
              <button role="menuitem" onClick={() => go("quit")}>{t("pvQuit")}<kbd>⌘Q</kbd></button>
            </span>
          )}
        </span>
      </footer>
    </div>
  );
}

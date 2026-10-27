import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "./api";
import { dayLabel, duration, tildify } from "./format";
import { listen } from "@tauri-apps/api/event";
import { lang, t } from "./i18n";
import { ExportSheet } from "./ExportSheet";
import { EntrySheet } from "./EntrySheet";
import { IntervalSheet } from "./IntervalSheet";
import { Attention, Bars, BigNumber, CostLine } from "./Overview";
import { Palette, type Command } from "./Palette";
import { Bar } from "./v2/Bar";
import { Home } from "./v2/Home";
import { LimitsPage } from "./v2/LimitsPage";
import { Projects } from "./v2/Projects";
import { Rail } from "./v2/Rail";
import { QuotaPrefsProvider } from "./v2/quota";
import "./v2/theme.css";
import { isoToday, periodLabel, rangeOf, shiftAnchor, type Area, type Scope } from "./nav";
import { TimerControl } from "./TimerControl";
import { ReviewDaySheet } from "./ReviewDaySheet";
import { ReviewList } from "./ReviewList";
import { RootsPicker } from "./RootsPicker";
import { SessionSheet } from "./SessionSheet";
import { Settings } from "./v2/Settings";
import { Timeline } from "./Timeline";
import { Toast } from "./Toast";
import { compact } from "./format";
import type { DayAgents, DayList, HumanAtom, Limits, ManualEntryInfo, Status, TargetOption } from "./types";

type Load =
  | { state: "loading" }
  | { state: "setup" }
  | { state: "no-derived"; message: string }
  | { state: "ready"; days: DayList };

function Setup({
  status,
  onApprove,
  onChoose,
  busy,
}: {
  status: Status;
  onApprove: (paths: string[]) => void;
  onChoose: () => void;
  busy: boolean;
}) {
  const proposed = status.proposed_roots.filter((r) => !r.approved).map((r) => r.path);
  return (
    <div className="empty">
      <h2>{t("setupTitle")}</h2>
      <p>{t("setupText")}</p>
      {proposed.length === 0 ? (
        <p className="muted">{t("setupNone")}</p>
      ) : (
        <div className="roots">
          <RootsPicker paths={proposed} home={status.home} busy={busy} onApprove={onApprove} label={t("setupApprove")} />
        </div>
      )}
      <button className="button secondary" disabled={busy} onClick={onChoose}>
        {t("setupChoose")}
      </button>
      {status.data_dir && <p className="muted">{t("dataDirectory", { path: tildify(status.data_dir, status.home) })}</p>}
    </div>
  );
}

function signed(ms: number): string {
  const m = Math.round(ms / 60_000);
  return `${m > 0 ? "+" : m < 0 ? "−" : "±"}${duration(Math.abs(ms))}`;
}

export function App() {
  const [hideRoots, setHideRoots] = useState(false);
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [day, setDay] = useState<string | null>(null);
  const [view, setView] = useState<DayAgents | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [area, setArea] = useState<Area>("today");
  const [scope, setScope] = useState<Scope>("week");
  const [anchor, setAnchor] = useState<string>(isoToday());
  const [focusSession, setFocusSession] = useState<string | null>(null);
  const [limits, setLimits] = useState<Limits | null>(null);
  const [pendingEntry, setPendingEntry] = useState(false);
  const [palette, setPalette] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [approving, setApproving] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [atom, setAtom] = useState<HumanAtom | null>(null);
  const [reviewing, setReviewing] = useState(false);
  const [targets, setTargets] = useState<TargetOption[]>([]);
  const [entry, setEntry] = useState<ManualEntryInfo | "new" | null>(null);
  const [reviewDay, setReviewDay] = useState(false);
  const loadTargets = () => void api.targets().then(setTargets).catch(() => setTargets([]));
  const openAtom = (a: HumanAtom) => {
    setAtom(a);
    void api.targets().then(setTargets).catch(() => setTargets([]));
  };
  const corrected = (text: string, error?: boolean) => {
    setNotice({ text, error });
    if (!error) {
      void loadDays(day);
      void api.status().then(setStatus);
    }
  };

  const loadDays = useCallback(async (keep?: string | null) => {
    const status = await api.status();
    setStatus(status);
    if (status.approved_roots.length === 0) {
      setLoad({ state: "setup" });
      return;
    }
    try {
      const days = await api.days();
      setLoad({ state: "ready", days });
      setDay((current) => {
        const want = keep ?? current;
        return want && days.days.includes(want) ? want : (days.days[0] ?? null);
      });
    } catch (e) {
      setLoad({ state: "no-derived", message: errorText(e) });
    }
  }, []);

  useEffect(() => {
    void loadDays();
  }, [loadDays]);

  // The collector state changes on its own (heartbeats, rebuilds): poll it
  // quietly, and reload the days whenever the collector finished a rebuild
  // (or while nothing is derived yet).
  const lastRebuild = useRef<number | null>(null);
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    const t = window.setInterval(() => {
      void api.status().then((st) => {
        setStatus(st);
        const rebuilt = st.runtime?.last_rebuild ?? null;
        const changed = rebuilt !== null && rebuilt !== lastRebuild.current;
        lastRebuild.current = rebuilt;
        if (changed || loadRef.current.state === "no-derived") void loadDays();
      });
    }, 5_000);
    return () => window.clearInterval(t);
  }, [loadDays]);

  // Provider-reported limit figures: on every rebuild and once a minute (they change on their own).
  useEffect(() => {
    if (load.state !== "ready") return;
    const read = () => void api.limits().then(setLimits).catch(() => setLimits(null));
    read();
    const id = window.setInterval(read, 60_000);
    return () => window.clearInterval(id);
  }, [load]);

  const approve = async (paths: string[]) => {
    setApproving(true);
    try {
      await api.approveRoots(paths);
      await loadDays();
    } catch (e) {
      setNotice({ text: errorText(e), error: true });
    } finally {
      setApproving(false);
    }
  };

  const chooseFolder = async () => {
    setApproving(true);
    try {
      if (await api.chooseRoot("provider")) await loadDays();
    } catch (e) {
      setNotice({ text: errorText(e), error: true });
    } finally {
      setApproving(false);
    }
  };

  useEffect(() => {
    if (!day) return;
    let live = true;
    api
      .dayAgents(day)
      .then((v) => live && setView(v))
      .catch((e) => live && setNotice({ text: errorText(e), error: true }));
    return () => {
      live = false;
    };
  }, [day, load]);

  const refresh = async () => {
    setBusy(true);
    setNotice(null);
    try {
      const r = await api.refresh();
      setNotice({ text: t("refreshed", { inserted: r.inserted, sessions: r.sessions, seconds: r.seconds.toFixed(1) }) });
      await loadDays(day);
    } catch (e) {
      setNotice({ text: errorText(e), error: true });
    } finally {
      setBusy(false);
    }
  };

  const totals = useMemo(() => {
    if (!view) return null;
    // A reviewed day reports its revision (worklog time), not the draft.
    const reviewed = view.review?.snapshot;
    const human = reviewed
      ? { ...view.human, counted_ms: reviewed.counted_ms, unknown_ms: reviewed.unknown_ms, counted_by_target: reviewed.counted_by_target, unknown_by_reason: {} }
      : view.human;
    let mainExec = 0;
    let subExec = 0;
    let output = 0;
    let prompts = 0;
    for (const s of view.sessions) {
      prompts += s.engagement_instants.length;
      for (const a of s.actors) {
        if (a.actor_kind === "known") subExec += a.execution_ms;
        else mainExec += a.execution_ms;
      }
      for (const u of s.usage) output += u.output.sum;
    }
    return { mainExec, subExec, output, prompts, sessions: view.sessions.length, human };
  }, [view]);

  const session = view?.sessions.find((s) => s.session_id === selected) ?? null;

  const days = load.state === "ready" ? load.days.days : [];
  const idx = day ? days.indexOf(day) : -1;
  const olderDay = idx >= 0 ? days[idx + 1] : undefined;
  const newerDay = idx > 0 ? days[idx - 1] : undefined;
  const goTo = (d: string) => {
    setSelected(null);
    setDay(d);
  };
  const [page, setPage] = useState<"day" | "history">("day");
  const today = isoToday();
  const oldest = days[days.length - 1];
  const [from, to] = rangeOf(scope, anchor);
  const showProject = (id: string | null) => {
    setScope("week");
    setAnchor(today);
    setFocusSession(id);
    setArea("projects");
  };
  const shiftPeriod = (dir: -1 | 1) => {
    if (area === "worktime") {
      const target = dir < 0 ? olderDay : newerDay;
      if (target) goTo(target);
    } else if (area === "projects" || area === "limits") {
      setAnchor(shiftAnchor(scope, anchor, dir));
      setFocusSession(null);
    }
  };
  const goToday = () => {
    if (area === "worktime") return days[0] ? goTo(days[0]) : undefined;
    setAnchor(today);
    setFocusSession(null);
  };
  // "Add time" from the start page: bring today forward, then open the sheet once its day is loaded.
  const addTimeToday = () => {
    setArea("worktime");
    if (days.includes(today) && day !== today) goTo(today);
    setPendingEntry(true);
  };
  useEffect(() => {
    if (pendingEntry && view && (!days.includes(today) || view.day === today)) {
      setPendingEntry(false);
      loadTargets();
      setEntry("new");
    }
  }); // eslint-disable-line react-hooks/exhaustive-deps
  const unknownWindows = view ? view.human.atoms.filter((a) => a.status === "unknown").length : 0;
  const canReview = !!view && status?.collector === "collecting" && !view.review;
  const rootsOpen = !!status && !hideRoots && status.proposed_roots.some((r) => !r.approved);

  // The menu bar (native, see main.rs) drives the window through "menu" events.
  const act = useRef<(action: string) => void>(() => {});
  act.current = (action) => {
    const canEdit = status?.collector === "collecting";
    switch (action) {
      case "settings": return setArea("settings");
      case "area_today": return setArea("today");
      case "area_projects": return setArea("projects");
      case "area_limits": return setArea("limits");
      case "area_worktime": return setArea("worktime");
      case "palette": return setPalette(true);
      case "overview": return (setArea("worktime"), setPage("day"));
      case "history": return (setArea("worktime"), setPage("history"));
      case "prev_day": return shiftPeriod(-1);
      case "next_day": return shiftPeriod(1);
      case "today": return goToday();
      case "export": return days.length > 0 ? setExporting(true) : undefined;
      case "refresh": return busy || !canEdit ? undefined : void refresh();
      // Work-time commands act on the work-time day: bring it forward first.
      case "new_entry": return view && canEdit ? (setArea("worktime"), loadTargets(), setEntry("new")) : undefined;
      case "review_day": return canReview ? (setArea("worktime"), setReviewDay(true)) : undefined;
      case "resolve": return unknownWindows > 0 ? (setArea("worktime"), setReviewing(true)) : undefined;
    }
  };
  useEffect(() => {
    void api.setMenuLanguage(lang === "de").catch(() => {});
    const off = listen<string>("menu", (e) => act.current(e.payload));
    return () => void off.then((f) => f());
  }, []);

  const canEditNow = status?.collector === "collecting";
  const commands: Command[] = [
    { id: "today", label: t("cmdGoToday"), hint: "⌘1", run: () => setArea("today") },
    { id: "projects", label: t("cmdGoProjects"), hint: "⌘2", run: () => setArea("projects") },
    { id: "worktime", label: t("cmdGoWorktime"), hint: "⌘3", run: () => setArea("worktime") },
    { id: "limits", label: t("cmdGoLimits"), hint: "⌘4", run: () => setArea("limits") },
    { id: "settings", label: t("settings"), hint: "⌘,", run: () => setArea("settings") },
    ...(days.length > 0 ? [{ id: "export", label: t("exportCsv"), hint: "⌘E", run: () => setExporting(true) }] : []),
    ...(canEditNow ? [{ id: "refresh", label: t("refresh"), hint: "⌘R", run: () => void refresh() }] : []),
    ...(canEditNow && view ? [{ id: "add", label: t("addTime"), hint: "⌘N", run: () => (setArea("worktime"), loadTargets(), setEntry("new")) }] : []),
    ...(canReview ? [{ id: "review", label: t("reviewDayShort"), hint: "⇧⌘R", run: () => (setArea("worktime"), setReviewDay(true)) }] : []),
    ...(unknownWindows > 0 ? [{ id: "resolve", label: t("resolve"), hint: "⌥⌘K", run: () => (setArea("worktime"), setReviewing(true)) }] : []),
  ];

  const tapNow = async (install: boolean) => {
    try {
      await api.limitsTap(install);
      setLimits(await api.limits());
      setNotice({ text: install ? t("tapOnDone") : t("tapOffDone") });
    } catch (e) {
      setNotice({ text: errorText(e), error: true });
    }
  };
  const reportError = (text: string) => setNotice({ text, error: true });
  const limitAlert = limits?.windows.some((w) => !w.expired && w.used_percent >= 100) ? "limits" : null;
  const bar = (
    <>
      {status && status.collector === "collecting" && (
        <TimerControl running={status.running_timer} loadTargets={loadTargets} targets={targets} onChanged={corrected} />
      )}
    </>
  );

  return (
    <QuotaPrefsProvider>
    <div className="v2">
      <Rail area={area} alert={limitAlert} onArea={setArea} onSettings={() => setArea("settings")} />
      <div className="main">
        {area === "today" && (
          <Bar title={t("today")} sub={dayLabel(today)}>
            {status && (
              <span className="live">
                <i className={`sdot ${status.collector}`} />
                {status.collector === "collecting" ? t("collecting") : status.collector === "stalled" ? t("collectorStalled") : status.collector === "elsewhere" ? t("collectorElsewhere") : status.collector === "stopped" ? t("collectorStopped") : t("collectorNone")}
              </span>
            )}
            {canEditNow && (
              <button className="btn" onClick={addTimeToday}>
                {t("addTime")}
              </button>
            )}
            {bar}
          </Bar>
        )}
        {area === "worktime" && (
          <Bar title={t("areaWorktime")} period={day ? { label: dayLabel(day), canPrev: !!olderDay, canNext: !!newerDay, onShift: shiftPeriod } : undefined}>
            {view && load.state === "ready" && (
              <div className="seg" role="group" aria-label={t("overview") + " / " + t("history")}>
                <button aria-pressed={page === "day"} onClick={() => setPage("day")}>{t("overview")}</button>
                <button aria-pressed={page === "history"} onClick={() => setPage("history")}>{t("history")}</button>
              </div>
            )}
            {canEditNow && view && (
              <button className="btn" onClick={() => (loadTargets(), setEntry("new"))}>
                {t("addTime")}
              </button>
            )}
            {bar}
          </Bar>
        )}
        {area === "limits" && (
          <Bar
            title={t("limitsTitle")}
            period={{ label: periodLabel(scope, anchor), scope, onScope: setScope, canPrev: !oldest || from > oldest, canNext: to < today, onShift: shiftPeriod }}
          />
        )}

        {area === "settings" && status && (
          <Settings
            status={status}
            limits={limits}
            onTap={tapNow}
            onChanged={(text, error) => {
              setNotice({ text, error });
              if (!error) {
                void loadDays(day);
                void api.status().then(setStatus);
              }
            }}
          />
        )}
        {area !== "settings" && load.state === "setup" && status && (
          <div className="body">
            <Setup status={status} onApprove={approve} onChoose={chooseFolder} busy={approving} />
          </div>
        )}
        {area !== "settings" && load.state === "no-derived" && (
          <div className="body">
            <div className="empty">
              <h2>{t("nothingDerived")}</h2>
              <p>{t("nothingDerivedHint")}</p>
              <p>{load.message}</p>
            </div>
          </div>
        )}

        {load.state === "ready" && area === "today" && (
          <Home
            day={today}
            hasData={days.includes(today)}
            reloadKey={load}
            limits={limits}
            onOpenSession={(id) => showProject(id)}
            onOpenWorktime={() => (days.includes(today) && goTo(today), setArea("worktime"))}
            onOpenLimits={() => setArea("limits")}
            onOpenSettings={() => setArea("settings")}
            onError={reportError}
          />
        )}
        {load.state === "ready" && area === "projects" && (
          <Projects
            from={from}
            to={to}
            scope={scope}
            anchor={anchor}
            canPrev={!oldest || from > oldest}
            canNext={to < today}
            reloadKey={load}
            focusSession={focusSession}
            onScope={(sc) => (setScope(sc), setFocusSession(null))}
            onShift={shiftPeriod}
            onError={reportError}
          />
        )}
        {load.state === "ready" && area === "limits" && (
          <LimitsPage from={from} to={to} limits={limits} reloadKey={load} onTap={tapNow} onError={reportError} />
        )}

        {area === "worktime" && view && totals && load.state === "ready" && page === "history" && (
          <div className="body">
            <div className="history">
              <CostLine cost={view.cost} />
              <dl className="facts">
                <div>
                  <dd>{totals.sessions}</dd>
                  <dt>{t("factSessions")}</dt>
                </div>
                <div>
                  <dd>{duration(totals.mainExec + totals.subExec)}</dd>
                  <dt>{t("factAgentTime")}</dt>
                </div>
                <div>
                  <dd>{totals.prompts}</dd>
                  <dt>{t("factPrompts")}</dt>
                </div>
                <div>
                  <dd>{compact(totals.output)}</dd>
                  <dt>{t("factTokens")}</dt>
                </div>
              </dl>
              <Timeline day={view} selected={selected} onSelect={setSelected} onAtom={openAtom} />
            </div>
          </div>
        )}
      {area === "worktime" && view && totals && load.state === "ready" && page === "day" && (
        <div className="body">
          <div className="main-card">
            <BigNumber
              human={totals.human}
              canReview={canReview}
              onReview={() => setReviewDay(true)}
              state={`${view.review ? t("reviewed", { rev: view.review.rev }) : t("provisional")}${view.discrepancy ? ` · ${t("newEvidenceDiffers")}` : ""}`}
            />
            <Attention human={totals.human} windows={unknownWindows} onResolve={() => setReviewing(true)} />
            <Bars view={view} human={totals.human} />
            {rootsOpen && status && (
              <div className="banner" role="status" title={status.proposed_roots.filter((r) => !r.approved).map((r) => tildify(r.path, status.home)).join("\n")}>
                <span>{t("rootsFound")}</span>
                <span className="spacer" />
                <span className="banner-actions">
                  <button className="button secondary small" disabled={approving} onClick={() => approve(status.proposed_roots.filter((r) => !r.approved).map((r) => r.path))}>
                    {approving ? <span className="spinner" aria-label="…" /> : t("approveAll", { n: status.proposed_roots.filter((r) => !r.approved).length })}
                  </button>
                  <button className="button ghost small" onClick={() => setHideRoots(true)}>
                    {t("notNow")}
                  </button>
                </span>
              </div>
            )}
            {view.discrepancy && view.review && (
              <div
                className="banner"
                role="status"
                title={`${t("legendCounted")}: ${signed(view.discrepancy.counted_delta_ms)} · ${t("legendUnknown")}: ${signed(view.discrepancy.unknown_delta_ms)} · ${t("legendBreak")}: ${signed(view.discrepancy.non_work_delta_ms)}`}
              >
                <span>{t("differsFromReview", { counted: signed(view.discrepancy.counted_delta_ms) })}</span>
                <span className="spacer" />
                <button className="button secondary small" onClick={() => setReviewDay(true)}>
                  {t("acceptAs", { rev: view.review.rev + 1 })}
                </button>
                <button
                  className="button ghost small"
                  onClick={async () => {
                    try {
                      await api.dismiss(view.day, view.discrepancy!.fingerprint);
                      corrected(t("keptRev", { rev: view.review!.rev }));
                    } catch (e) {
                      corrected(errorText(e), true);
                    }
                  }}
                >
                  {t("keepRev", { rev: view.review.rev })}
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      </div>

      {exporting && load.state === "ready" && (
        <ExportSheet
          days={load.days.days}
          current={day}
          onClose={() => setExporting(false)}
          onDone={(text, error) => setNotice({ text, error })}
        />
      )}
      {reviewDay && view && <ReviewDaySheet day={view} onClose={() => setReviewDay(false)} onChanged={corrected} />}
      {reviewing && view && (
        <ReviewList
          day={view}
          onOpen={(a) => {
            setReviewing(false);
            openAtom(a);
          }}
          onClose={() => setReviewing(false)}
          onChanged={corrected}
        />
      )}
      {atom && view && (
        <IntervalSheet
          atom={atom}
          day={view}
          targets={targets}
          onClose={() => setAtom(null)}
          onChanged={corrected}
          onEditEntry={(e) => {
            setAtom(null);
            setEntry(e);
          }}
        />
      )}
      {entry && view && (
        <EntrySheet
          day={view.day}
          tz={view.timezone}
          entry={entry === "new" ? null : entry}
          targets={targets}
          onClose={() => setEntry(null)}
          onChanged={corrected}
        />
      )}
      {session && view && <SessionSheet s={session} tz={view.timezone} pricing={view.cost.pricing} onClose={() => setSelected(null)} />}
      {palette && <Palette commands={commands} onClose={() => setPalette(false)} />}
      {notice && <Toast text={notice.text} error={notice.error} onGone={() => setNotice(null)} />}
    </div>
    </QuotaPrefsProvider>
  );
}

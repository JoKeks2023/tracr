import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "./api";
import { dayLabel, duration, tildify } from "./format";
import { t } from "./i18n";
import { ExportSheet } from "./ExportSheet";
import { EntrySheet } from "./EntrySheet";
import { IntervalSheet } from "./IntervalSheet";
import { Menu, MenuItem } from "./Menu";
import { Attention, Bars, BigNumber, CostLine } from "./Overview";
import { TimerControl } from "./TimerControl";
import { ReviewDaySheet } from "./ReviewDaySheet";
import { ReviewList } from "./ReviewList";
import { RootsPicker } from "./RootsPicker";
import { SessionSheet } from "./SessionSheet";
import { SettingsSheet } from "./SettingsSheet";
import { Timeline } from "./Timeline";
import { Toast } from "./Toast";
import { compact } from "./format";
import type { DayAgents, DayList, HumanAtom, ManualEntryInfo, Status, TargetOption } from "./types";

type Load =
  | { state: "loading" }
  | { state: "setup" }
  | { state: "no-derived"; message: string }
  | { state: "ready"; days: DayList };

function collectorLabel(c: Status["collector"]): string {
  return {
    collecting: t("collecting"),
    stalled: t("collectorStalled"),
    elsewhere: t("collectorElsewhere"),
    stopped: t("collectorStopped"),
    not_configured: t("collectorNone"),
  }[c];
}

/** A dot with the details in its tooltip; the words appear only when something is wrong. */
function CollectorStatus({ status }: { status: Status }) {
  const rt = status.runtime;
  const sensorOff = rt && rt.keys.os_sensor && rt.keys.os_sensor !== "collecting";
  const title = [
    collectorLabel(status.collector),
    status.collector_detail,
    rt?.last_error && `Last error: ${rt.last_error}`,
    rt && `Sensor: ${rt.sensor}${sensorOff ? " (unavailable)" : ""}`,
    status.data_dir && `Data: ${tildify(status.data_dir, status.home)}`,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <span className="collector" title={title}>
      <span className={`dot ${status.collector}`} aria-hidden />
      {status.collector !== "collecting" && collectorLabel(status.collector)}
    </span>
  );
}

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
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [approving, setApproving] = useState(false);
  const [settings, setSettings] = useState(false);
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

  // ⌘, opens the settings, as in any Mac app.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === ",") {
        e.preventDefault();
        setSettings(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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
  const unknownWindows = view ? view.human.atoms.filter((a) => a.status === "unknown").length : 0;
  const canReview = !!view && status?.collector === "collecting" && !view.review;
  const rootsOpen = !!status && !hideRoots && status.proposed_roots.some((r) => !r.approved);

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region />
      <div className="page">
      <header className="day-header">
        <div>
          <div className="nav">
            <button className="button ghost icon" aria-label={t("prevDay")} disabled={!olderDay} onClick={() => olderDay && goTo(olderDay)}>
              ‹
            </button>
            <h1>{day ? dayLabel(day) : "tracr"}</h1>
            <button className="button ghost icon" aria-label={t("nextDay")} disabled={!newerDay} onClick={() => newerDay && goTo(newerDay)}>
              ›
            </button>
          </div>
          <div className="status-line">
            {status && load.state !== "setup" && <CollectorStatus status={status} />}
            {view && (
              <span title={view.quality.provisional_reasons.join("\n")}>
                {view.review ? t("reviewed", { rev: view.review.rev }) : t("provisional")}
                {view.discrepancy && ` · ${t("newEvidenceDiffers")}`}
              </span>
            )}
          </div>
        </div>
        <div className="spacer" />
        <div className="header-actions">
          {status && status.collector === "collecting" && (
            <TimerControl running={status.running_timer} loadTargets={loadTargets} targets={targets} onChanged={corrected} />
          )}
          {view && status?.collector === "collecting" && (
            <button
              className="button ghost"
              onClick={() => {
                loadTargets();
                setEntry("new");
              }}
            >
              {t("addTime")}
            </button>
          )}
          {load.state === "ready" && (
            <Menu label={t("more")}>
              {(close) => (
                <>
                  {days.length > 1 && (
                    <label className="menu-field">
                      {t("chooseDay")}
                      <select className="select" value={day ?? ""} onChange={(e) => (goTo(e.target.value), close())}>
                        {days.map((d) => (
                          <option key={d} value={d}>
                            {dayLabel(d)} · {d}
                          </option>
                        ))}
                      </select>
                    </label>
                  )}
                  {days.length > 0 && (
                    <MenuItem onClick={() => (close(), setExporting(true))}>{t("exportCsv")}</MenuItem>
                  )}
                  <MenuItem disabled={busy || status?.collector !== "collecting"} onClick={() => (close(), void refresh())}>
                    {t("refresh")}
                  </MenuItem>
                </>
              )}
            </Menu>
          )}
          {status && (
            <button className="button ghost icon" onClick={() => setSettings(true)} title={t("settingsHint")} aria-label={t("settings")}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h0a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h0a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v0a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
            </button>
          )}
        </div>
      </header>

      {load.state === "setup" && status && <Setup status={status} onApprove={approve} onChoose={chooseFolder} busy={approving} />}
      {load.state === "no-derived" && (
        <div className="empty">
          <h2>{t("nothingDerived")}</h2>
          <p>{t("nothingDerivedHint")}</p>
          <p className="muted">{load.message}</p>
        </div>
      )}

      {view && totals && load.state === "ready" && page === "history" && (
        <div className="history">
          <button className="back-link" onClick={() => setPage("day")}>
            <span aria-hidden>‹</span> {t("back")}
          </button>
          <h2 className="history-title">{t("historyTitle", { day: dayLabel(view.day) })}</h2>
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
      )}
      {view && totals && load.state === "ready" && page === "day" && (
        <>
          <div className="main-card">
            <BigNumber human={totals.human} canReview={canReview} onReview={() => setReviewDay(true)} />
            <Bars view={view} human={totals.human} />
            <Attention human={totals.human} windows={unknownWindows} onResolve={() => setReviewing(true)} />
            <CostLine cost={view.cost} />
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
            <button className="history-link" onClick={() => setPage("history")}>
              {t("history")} <span aria-hidden>›</span>
            </button>
          </div>
        </>
      )}

      </div>

      {settings && status && (
        <SettingsSheet
          status={status}
          onClose={() => setSettings(false)}
          onChanged={(text, error) => {
            setNotice({ text, error });
            if (!error) {
              void loadDays(day);
              void api.status().then(setStatus);
            }
          }}
        />
      )}
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
      {notice && <Toast text={notice.text} error={notice.error} onGone={() => setNotice(null)} />}
    </div>
  );
}

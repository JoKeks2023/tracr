import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "./api";
import { compact, dayLabel, duration, estimate, reasonText, tildify } from "./format";
import { ExportSheet } from "./ExportSheet";
import { EntrySheet } from "./EntrySheet";
import { IntervalSheet } from "./IntervalSheet";
import { TimerControl } from "./TimerControl";
import { ReviewDaySheet } from "./ReviewDaySheet";
import { ReviewList } from "./ReviewList";
import { RootsPicker } from "./RootsPicker";
import { SettingsSheet } from "./SettingsSheet";
import { SessionSheet } from "./SessionSheet";
import { Timeline } from "./Timeline";
import type { DayAgents, DayCost, DayList, HumanAtom, ManualEntryInfo, Status, TargetOption } from "./types";

type Load =
  | { state: "loading" }
  | { state: "setup" }
  | { state: "no-derived"; message: string }
  | { state: "ready"; days: DayList };

const COLLECTOR_LABEL: Record<Status["collector"], string> = {
  collecting: "Collecting",
  stalled: "Collector not responding",
  elsewhere: "Collecting in another tracr process",
  stopped: "Not collecting",
  not_configured: "No approved history",
};

function CostMetric({ cost }: { cost: DayCost }) {
  const c = cost.identified;
  const p = cost.pricing;
  const tokens = c.priced_tokens + c.unpriced_tokens;
  const unpricedShare = tokens > 0 ? c.unpriced_tokens / tokens : 0;
  const lines = [
    `${p.label}.`,
    `Prices: catalog ${p.catalog_version} as of ${p.oldest_fetched_at ?? "unknown"} (${p.source_urls.join(", ")}).`,
    ...c.reasons.map((r) => `Unpriced: ${reasonText(r)}`),
    ...c.assumptions.map((a) => `Assumed: ${reasonText(a)}`),
    cost.unidentified.priced_records + cost.unidentified.unpriced_records + cost.unidentified.partially_priced_records > 0
      ? `Unidentified usage, not included: ${estimate(cost.unidentified, p.currency)}`
      : null,
    cost.spanning_records > 0
      ? `Usage spanning the day boundary, not included: ${cost.spanning_records} records, ${estimate(cost.spanning, p.currency)}`
      : null,
  ].filter(Boolean);
  return (
    <div className="metric" title={lines.join("\n")}>
      <div className="value">{estimate(c, p.currency)}</div>
      <div className="label">
        est. API cost · not spend
        {unpricedShare > 0 && ` · ${unpricedShare < 0.01 ? "<1" : Math.round(unpricedShare * 100)}% of tokens unpriced`}
      </div>
    </div>
  );
}

function CollectorStatus({ status }: { status: Status }) {
  const rt = status.runtime;
  const sensorOff = rt && rt.keys.os_sensor && rt.keys.os_sensor !== "collecting";
  const title = [
    status.collector_detail,
    rt?.last_error && `Last error: ${rt.last_error}`,
    rt && `Sensor: ${rt.sensor}${sensorOff ? " (unavailable)" : ""}`,
    status.data_dir && `Data: ${status.data_dir}`,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <span className="collector" title={title}>
      <span className={`dot ${status.collector}`} aria-hidden />
      {COLLECTOR_LABEL[status.collector]}
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
      <h2>Approve what tracr may read</h2>
      <p>
        tracr reads agent transcripts only from folders you approve, and stores metadata only: never prompts, messages
        or tool output.
      </p>
      {proposed.length === 0 ? (
        <p className="muted">No agent history found in the usual places on this Mac.</p>
      ) : (
        <div className="roots">
          <RootsPicker paths={proposed} home={status.home} busy={busy} onApprove={onApprove} label="Approve" />
        </div>
      )}
      <button className="button secondary" disabled={busy} onClick={onChoose}>
        Choose another folder…
      </button>
      {status.data_dir && <p className="muted">Data directory: {tildify(status.data_dir, status.home)}</p>}
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
      setNotice({ text: `Imported ${r.inserted} new records · rebuilt ${r.sessions} sessions in ${r.seconds.toFixed(1)}s` });
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
  const asOf = load.state === "ready" ? load.days.derived_as_of : null;

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region />
      <div className="header">
        <div>
          <h1>{day ? dayLabel(day) : "tracr"}</h1>
          <div className="sub">
            {view ? (
              <span title={view.quality.provisional_reasons.join("\n")}>
                {view.review
                  ? `Reviewed r${view.review.rev} · ${new Date(view.review.at).toLocaleString("en-GB", { timeZone: view.timezone, day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}${view.discrepancy ? " · new evidence differs" : ""} · `
                  : "Provisional · "}
                {view.quality.ingestion_complete
                  ? "data complete"
                  : view.quality.ingestion_verified_through
                    ? `data still arriving (verified through ${new Date(view.quality.ingestion_verified_through).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: view.timezone })})`
                    : "ingestion not verified"}
                {view.quality.recovered_records > 0 && ` · ${view.quality.recovered_records} records recovered late`}
                {view.quality.stream_extents.some((e) => e.state === "gap") && " · ingestion gaps"}
              </span>
            ) : asOf ? (
              `Derived ${new Date(asOf).toLocaleString("en-GB")}`
            ) : (
              "Local development diary"
            )}
            {load.state === "ready" && ` · ${load.days.timezone}`}
          </div>
        </div>
        <div className="spacer" />
        <div className="header-actions">
          {notice && <span className={`notice${notice.error ? " error" : ""}`}>{notice.text}</span>}
          {status && status.collector === "collecting" && (
            <TimerControl running={status.running_timer} loadTargets={loadTargets} targets={targets} onChanged={corrected} />
          )}
          {status && load.state !== "setup" && <CollectorStatus status={status} />}
          {load.state === "ready" && load.days.days.length > 0 && (
            <select
              className="select"
              aria-label="Day"
              value={day ?? ""}
              onChange={(e) => {
                setSelected(null);
                setDay(e.target.value);
              }}
            >
              {load.days.days.map((d) => (
                <option key={d} value={d}>
                  {dayLabel(d)} · {d}
                </option>
              ))}
            </select>
          )}
          {view && status?.collector === "collecting" && !view.review && (
            <button className="button ghost" onClick={() => setReviewDay(true)}>
              Review day
            </button>
          )}
          {view && view.human.atoms.some((a) => a.status === "unknown") && (
            <button className="button ghost" onClick={() => setReviewing(true)}>
              {view.human.atoms.filter((a) => a.status === "unknown").length} to review
            </button>
          )}
          {view && status?.collector === "collecting" && (
            <button
              className="button ghost"
              onClick={() => {
                loadTargets();
                setEntry("new");
              }}
            >
              Add time
            </button>
          )}
          {load.state === "ready" && load.days.days.length > 0 && (
            <button className="button ghost" onClick={() => setExporting(true)}>
              Export CSV
            </button>
          )}
          {status && (
            <button className="button ghost icon" onClick={() => setSettings(true)} title="Settings (⌘,)" aria-label="Settings">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h0a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h0a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v0a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
            </button>
          )}
          <button
            className="button primary"
            onClick={refresh}
            disabled={busy || status?.collector !== "collecting"}
            title={status?.collector === "elsewhere" ? "Another tracr process is collecting; it refreshes on its own." : undefined}
          >
            {busy ? <span className="spinner" aria-label="Refreshing" /> : "Refresh"}
          </button>
        </div>
      </div>

      {load.state === "setup" && status && <Setup status={status} onApprove={approve} onChoose={chooseFolder} busy={approving} />}
      {load.state === "no-derived" && (
        <div className="empty">
          <h2>Nothing derived yet</h2>
          <p>The collector builds the day views after its first import. Press Refresh if it takes long.</p>
          <p className="muted">{load.message}</p>
        </div>
      )}

      {view && totals && load.state === "ready" && (
        <>
          <div className="summary">
            <div className="metric">
              <div className="value human-value">{duration(totals.human.counted_ms)}</div>
              <div className="label">
                {view.review ? `counted human time · reviewed r${view.review.rev}` : "counted human time · inferred, provisional"}
              </div>
            </div>
            <div
              className="metric"
              title={Object.entries(totals.human.unknown_by_reason)
                .map(([r, ms]) => `${r.replaceAll("_", " ")}: ${duration(ms)}`)
                .join("\n")}
            >
              <div className="value unknown-value">{duration(totals.human.unknown_ms)}</div>
              <div className="label">unknown · visible, not counted</div>
            </div>
            <div className="metric">
              <div className="value">{totals.sessions}</div>
              <div className="label">agent sessions</div>
            </div>
            <div className="metric">
              <div className="value">{duration(totals.mainExec)}</div>
              <div className="label">main-chain execution · agent time, not human time</div>
            </div>
            <div className="metric">
              <div className="value">{duration(totals.subExec)}</div>
              <div className="label">subagent execution · parallel, may exceed the day</div>
            </div>
            <div className="metric">
              <div className="value">{totals.prompts}</div>
              <div className="label">human prompts</div>
            </div>
            <div className="metric">
              <div className="value">{compact(totals.output)}</div>
              <div className="label">output tokens</div>
            </div>
            <CostMetric cost={view.cost} />
          </div>
          {view.counted_by_project.some((p) => p.kind === "project") && (
            <div className="by-target">
              <span className="muted">Counted by project</span>
              {view.counted_by_project.map((p) => (
                <span
                  key={p.project_id ?? p.label}
                  className={p.kind === "other" ? "muted" : undefined}
                  title={p.parts.map((x) => `${x.label} ${duration(x.ms)}`).join("\n") || undefined}
                >
                  {p.label} <span className="mono">{duration(p.ms)}</span>
                </span>
              ))}
            </div>
          )}
          {Object.keys(totals.human.counted_by_target).length > 0 && (
            <div className={`by-target${view.counted_by_project.some((p) => p.kind === "project") ? " secondary" : ""}`}>
              <span className="muted">Counted by repository</span>
              {Object.entries(totals.human.counted_by_target)
                .sort((a, b) => b[1] - a[1])
                .map(([target, ms]) => (
                  <span key={target} className={target === "unattributed" || target === "conflicting" ? "muted" : undefined}>
                    {target} <span className="mono">{duration(ms)}</span>
                  </span>
                ))}
            </div>
          )}
          {status && !hideRoots && status.proposed_roots.some((r) => !r.approved) && (
            <div className="banner" role="status">
              <span>
                Agent history found that tracr does not read yet:{" "}
                {status.proposed_roots
                  .filter((r) => !r.approved)
                  .map((r) => (
                    <span key={r.path} className="mono">
                      {tildify(r.path, status.home)}{" "}
                    </span>
                  ))}
                <span className="muted">· metadata only, never prompts or output</span>
              </span>
              <span className="spacer" />
              <span className="banner-actions">
                <button
                  className="button secondary small"
                  style={{ whiteSpace: "nowrap" }}
                  disabled={approving}
                  onClick={() => approve(status.proposed_roots.filter((r) => !r.approved).map((r) => r.path))}
                >
                  {approving ? <span className="spinner" aria-label="Approving" /> : `Approve all (${status.proposed_roots.filter((r) => !r.approved).length})`}
                </button>
                <button className="button ghost small" onClick={() => setSettings(true)}>
                  Choose…
                </button>
                <button className="button ghost small" onClick={() => setHideRoots(true)}>
                  Not now
                </button>
              </span>
            </div>
          )}
          {view.discrepancy && view.review && (
            <div className="banner" role="status">
              <span>
                New evidence differs from your review (r{view.review.rev}): counted{" "}
                <span className="mono">{signed(view.discrepancy.counted_delta_ms)}</span>, unknown{" "}
                <span className="mono">{signed(view.discrepancy.unknown_delta_ms)}</span>, break{" "}
                <span className="mono">{signed(view.discrepancy.non_work_delta_ms)}</span>
              </span>
              <span className="spacer" />
              <button className="button secondary small" onClick={() => setReviewDay(true)}>
                Accept as r{view.review.rev + 1}…
              </button>
              <button
                className="button ghost small"
                onClick={async () => {
                  try {
                    await api.dismiss(view.day, view.discrepancy!.fingerprint);
                    corrected(`Kept r${view.review!.rev}`);
                  } catch (e) {
                    corrected(errorText(e), true);
                  }
                }}
              >
                Keep r{view.review.rev}
              </button>
            </div>
          )}
          <Timeline day={view} selected={selected} onSelect={setSelected} onAtom={openAtom} />
        </>
      )}

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
    </div>
  );
}

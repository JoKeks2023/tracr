import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { compact, dayLabel, duration } from "./format";
import { ExportSheet } from "./ExportSheet";
import { SessionSheet } from "./SessionSheet";
import { Timeline } from "./Timeline";
import type { DayAgents, DayList, Status } from "./types";

type Load =
  | { state: "loading" }
  | { state: "setup" }
  | { state: "no-derived"; message: string }
  | { state: "ready"; days: DayList };

const COLLECTOR_LABEL: Record<Status["collector"], string> = {
  collecting: "Collecting",
  elsewhere: "Collecting in another tracr process",
  stopped: "Not collecting",
  not_configured: "No approved history",
};

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

function Setup({ status, onApprove, busy }: { status: Status; onApprove: (p: string) => void; busy: string | null }) {
  const proposed = status.proposed_roots;
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
          {proposed.map((r) => (
            <div className="root" key={r.path}>
              <span className="mono">{r.path}</span>
              {r.approved ? (
                <span className="muted">Approved</span>
              ) : (
                <button className="button primary" disabled={busy !== null} onClick={() => onApprove(r.path)}>
                  {busy === r.path ? <span className="spinner" aria-label="Approving" /> : "Approve"}
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      {status.data_dir && <p className="muted">Data directory: {status.data_dir}</p>}
    </div>
  );
}

export function App() {
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [day, setDay] = useState<string | null>(null);
  const [view, setView] = useState<DayAgents | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [approving, setApproving] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);

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

  // The collector state changes on its own (heartbeats, rebuilds): poll it quietly.
  useEffect(() => {
    const t = window.setInterval(() => {
      void api.status().then(setStatus);
    }, 15_000);
    return () => window.clearInterval(t);
  }, []);

  const approve = async (path: string) => {
    setApproving(path);
    try {
      await api.approveRoot(path);
      await loadDays();
    } catch (e) {
      setNotice({ text: errorText(e), error: true });
    } finally {
      setApproving(null);
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
    const human = view.human;
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
            {asOf ? `Derived ${new Date(asOf).toLocaleString("en-GB")}` : "Local development diary"}
            {load.state === "ready" && ` · ${load.days.timezone}`}
          </div>
        </div>
        <div className="spacer" />
        <div className="header-actions">
          {notice && <span className={`notice${notice.error ? " error" : ""}`}>{notice.text}</span>}
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
          {load.state === "ready" && load.days.days.length > 0 && (
            <button className="button ghost" onClick={() => setExporting(true)}>
              Export CSV
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

      {load.state === "setup" && status && <Setup status={status} onApprove={approve} busy={approving} />}
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
              <div className="label">counted human time · inferred, provisional</div>
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
          </div>
          <Timeline day={view} selected={selected} onSelect={setSelected} />
        </>
      )}

      {exporting && load.state === "ready" && (
        <ExportSheet
          days={load.days.days}
          current={day}
          onClose={() => setExporting(false)}
          onDone={(text, error) => setNotice({ text, error })}
        />
      )}
      {session && view && <SessionSheet s={session} tz={view.timezone} onClose={() => setSelected(null)} />}
    </div>
  );
}

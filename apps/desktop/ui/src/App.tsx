import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { compact, dayLabel, duration } from "./format";
import { SessionSheet } from "./SessionSheet";
import { Timeline } from "./Timeline";
import type { DayAgents, DayList, Status } from "./types";

type Load =
  | { state: "loading" }
  | { state: "no-ledger"; status: Status }
  | { state: "no-derived"; message: string }
  | { state: "ready"; days: DayList };

export function App() {
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [day, setDay] = useState<string | null>(null);
  const [view, setView] = useState<DayAgents | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);

  const loadDays = useCallback(async (keep?: string | null) => {
    const status = await api.status();
    if (!status.ledger_present) {
      setLoad({ state: "no-ledger", status });
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
    return { mainExec, subExec, output, prompts, sessions: view.sessions.length };
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
          <button className="button primary" onClick={refresh} disabled={busy || load.state === "no-ledger"}>
            {busy ? <span className="spinner" aria-label="Refreshing" /> : "Refresh"}
          </button>
        </div>
      </div>

      {load.state === "no-ledger" && (
        <div className="empty">
          <h2>No ledger yet</h2>
          <p>
            {load.status.data_dir ? (
              <>
                tracr looks in <code>{load.status.data_dir}</code>.
              </>
            ) : (
              "No data directory is configured."
            )}{" "}
            Approve your Claude Code history once with <code>tracr roots add --provider ~/.claude/projects</code>, then
            press Refresh.
          </p>
        </div>
      )}
      {load.state === "no-derived" && (
        <div className="empty">
          <h2>Nothing derived yet</h2>
          <p>Press Refresh to import and build the day views.</p>
          <p className="muted">{load.message}</p>
        </div>
      )}

      {view && totals && load.state === "ready" && (
        <>
          <div className="summary">
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
            <div className="metric pending">
              <div className="value">Not derived yet</div>
              <div className="label">human time · counted and unknown arrive with S1.4</div>
            </div>
          </div>
          <Timeline day={view} selected={selected} onSelect={setSelected} />
        </>
      )}

      {session && view && <SessionSheet s={session} tz={view.timezone} onClose={() => setSelected(null)} />}
    </div>
  );
}

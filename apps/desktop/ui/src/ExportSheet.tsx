import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { dayLabel } from "./format";

/** Days are newest first. Defaults to the month of the day being viewed. */
export function ExportSheet({ days, current, onClose, onDone }: {
  days: string[];
  current: string | null;
  onClose: () => void;
  onDone: (text: string, error?: boolean) => void;
}) {
  const ascending = useMemo(() => [...days].reverse(), [days]);
  const month = (current ?? ascending[ascending.length - 1] ?? "").slice(0, 7);
  const inMonth = ascending.filter((d) => d.startsWith(month));
  const [from, setFrom] = useState(inMonth[0] ?? ascending[0] ?? "");
  const [to, setTo] = useState(inMonth[inMonth.length - 1] ?? ascending[ascending.length - 1] ?? "");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const run = async () => {
    setBusy(true);
    try {
      const path = await api.exportAgentSessions(from, to);
      if (path) onDone(`Exported to ${path}`);
      onClose();
    } catch (e) {
      onDone(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };

  const select = (value: string, set: (v: string) => void, label: string) => (
    <label className="field">
      <span>{label}</span>
      <select className="select" value={value} onChange={(e) => set(e.target.value)}>
        {ascending.map((d) => (
          <option key={d} value={d}>
            {dayLabel(d)} · {d}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet export-sheet" role="dialog" aria-label="Export CSV">
        <header>
          <div style={{ flex: 1 }}>
            <h2>Export CSV</h2>
            <div className="muted">Agent sessions: one row per day, session and agent</div>
          </div>
        </header>
        <div className="body">
          <div className="fields">
            {select(from, setFrom, "From")}
            {select(to, setTo, "To")}
          </div>
          <p className="note">
            Columns: repository, branch, agent execution, turns, tool calls, human prompts and tokens by category.
            Agent execution is agent time, not human time. The timesheet export of human time follows once human
            time is derived.
          </p>
          <div className="actions">
            <button className="button ghost" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" onClick={run} disabled={busy || !from || !to || to < from}>
              {busy ? <span className="spinner" aria-label="Exporting" /> : "Export…"}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

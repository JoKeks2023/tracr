import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { dayLabel } from "./format";

type Kind = "timesheet" | "agent_sessions" | "everything";

const KINDS: Record<Kind, { label: string; note: string }> = {
  timesheet: {
    label: "Timesheet (human time)",
    note: "One row per interval: counted minutes (inferred, provisional) and unknown minutes in separate columns, never one total. Blank time has no row.",
  },
  agent_sessions: {
    label: "Agent sessions",
    note: "One row per day, session and agent: repository, branch, agent execution, turns, tool calls, human prompts and tokens. Agent time is not human time.",
  },
  everything: {
    label: "Everything (folder of CSV files)",
    note: "All data sets, all columns, all days: sessions, agents, execution, turns, tool calls, token usage, prompts, human time, device and coverage, with a README.",
  },
};

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
  const [kind, setKind] = useState<Kind>("timesheet");

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const run = async () => {
    setBusy(true);
    try {
      const path = kind === "everything" ? await api.exportEverything() : await api.exportCsv(kind, from, to);
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
            <div className="muted">{KINDS[kind].label}</div>
          </div>
        </header>
        <div className="body">
          <label className="field">
            <span>What</span>
            <select className="select" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>
              {(Object.keys(KINDS) as Kind[]).map((k) => (
                <option key={k} value={k}>
                  {KINDS[k].label}
                </option>
              ))}
            </select>
          </label>
          {kind !== "everything" && (
            <div className="fields">
              {select(from, setFrom, "From")}
              {select(to, setTo, "To")}
            </div>
          )}
          <p className="note">{KINDS[kind].note}</p>
          <div className="actions">
            <button className="button ghost" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" onClick={run} disabled={busy || (kind !== "everything" && (!from || !to || to < from))}>
              {busy ? <span className="spinner" aria-label="Exporting" /> : "Export…"}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

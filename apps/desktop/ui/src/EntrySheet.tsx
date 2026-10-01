import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import { TargetSelect, targetKey } from "./TargetSelect";
import type { ManualEntryInfo, TargetOption } from "./types";

/** "HH:MM" in `tz` on `day` → epoch ms. */
function toMs(day: string, time: string, tz: string): number | null {
  const [h, m] = time.split(":").map(Number);
  if (Number.isNaN(h) || Number.isNaN(m)) return null;
  // Find the instant whose wall time in `tz` is day + time (two passes handle DST).
  let guess = Date.parse(`${day}T${time}:00Z`);
  for (let i = 0; i < 2; i++) {
    const parts = new Intl.DateTimeFormat("en-GB", {
      timeZone: tz, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false,
    }).formatToParts(guess);
    const get = (t: string) => Number(parts.find((p) => p.type === t)?.value);
    const shown = Date.UTC(get("year"), get("month") - 1, get("day"), get("hour") % 24, get("minute"));
    guess += Date.parse(`${day}T${time}:00Z`) - shown;
  }
  return guess;
}

function toTime(ms: number, tz: string): string {
  return new Intl.DateTimeFormat("en-GB", { timeZone: tz, hour: "2-digit", minute: "2-digit", hour12: false }).format(ms);
}

/** Add a retroactive entry, or edit an existing entry. */
export function EntrySheet({ day, tz, entry, targets, onClose, onChanged }: {
  day: string;
  tz: string;
  entry: ManualEntryInfo | null;
  targets: TargetOption[];
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
}) {
  const [start, setStart] = useState(entry ? toTime(entry.start, tz) : "09:00");
  const [end, setEnd] = useState(entry?.end ? toTime(entry.end, tz) : entry ? "" : "10:00");
  const known = entry?.target && targets.some((t) => t.key === entry.target);
  const [target, setTarget] = useState(entry?.target ? (known ? entry.target : "__other") : "");
  const [other, setOther] = useState(entry?.target?.startsWith("other:") && !known ? entry.target.slice(6) : "");
  const [note, setNote] = useState(entry?.note ?? "");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const save = async () => {
    const s = toMs(day, start, tz);
    const e = entry?.running ? undefined : toMs(day, end, tz);
    if (s === null || e === null || (e !== undefined && e <= s)) {
      onChanged("The end must be after the start", true);
      return;
    }
    setBusy(true);
    try {
      await api.manual({
        op: entry ? "edit" : "add",
        id: entry?.id,
        start: s,
        end: e,
        target: targetKey(target, other) || undefined,
        note: note.trim() || undefined,
      });
      onChanged(entry ? "Entry updated" : "Time added");
      onClose();
    } catch (err) {
      onChanged(errorText(err), true);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!entry) return;
    setBusy(true);
    try {
      await api.manual({ op: "delete", id: entry.id });
      onChanged("Entry deleted");
      onClose();
    } catch (err) {
      onChanged(errorText(err), true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet export-sheet" role="dialog" aria-label={entry ? "Edit entry" : "Add time"}>
        <header>
          <div style={{ flex: 1 }}>
            <h2>{entry ? (entry.kind === "timer" ? "Edit timer" : "Edit entry") : "Add time"}</h2>
            <div className="muted">{day} · counts as work you state; never changes agent data</div>
          </div>
        </header>
        <div className="body">
          <div className="fields">
            <label className="field">
              <span>From</span>
              <input className="input" type="time" value={start} onChange={(e) => setStart(e.target.value)} />
            </label>
            <label className="field">
              <span>To</span>
              <input
                className="input"
                type="time"
                value={end}
                disabled={entry?.running}
                onChange={(e) => setEnd(e.target.value)}
              />
            </label>
          </div>
          <label className="field" style={{ marginTop: "var(--space-4)" }}>
            <span>For</span>
            <div className="target-row">
              <TargetSelect targets={targets} value={target} onChange={setTarget} other={other} onOther={setOther} allowNone />
            </div>
          </label>
          <label className="field">
            <span>Note</span>
            <input className="input" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} placeholder="Optional" />
          </label>
          <div className="actions">
            {entry && (
              <button className="button ghost" disabled={busy} onClick={remove} style={{ marginRight: "auto" }}>
                Delete
              </button>
            )}
            <button className="button ghost" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={busy} onClick={save}>
              {busy ? <span className="spinner" /> : entry ? "Save" : "Add"}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

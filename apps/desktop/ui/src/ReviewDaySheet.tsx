import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import { duration } from "./format";
import type { DayAgents } from "./types";

/** Review the day: freeze its counting as the next revision (AD-18). */
export function ReviewDaySheet({ day, onClose, onChanged }: {
  day: DayAgents;
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
}) {
  const next = (day.review?.rev ?? 0) + 1;
  const [reason, setReason] = useState(day.discrepancy ? "Accepted new evidence" : "");
  const [busy, setBusy] = useState(false);
  const draft = day.discrepancy?.draft;
  const counted = draft?.counted_ms ?? day.human.counted_ms;
  const unknown = draft?.unknown_ms ?? day.human.unknown_ms;
  const byTarget = draft?.counted_by_target ?? day.human.counted_by_target;
  const nonWork = draft?.non_work_ms ?? day.human.atoms.filter((a) => a.status === "non_work").reduce((n, a) => n + a.end - a.start, 0);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const review = async () => {
    setBusy(true);
    try {
      await api.review(day.day, reason.trim() || undefined);
      onChanged(`Reviewed · r${next}`);
      onClose();
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet export-sheet" role="dialog" aria-label="Review day">
        <header>
          <div style={{ flex: 1 }}>
            <h2>{next === 1 ? "Review day" : `Accept as revision r${next}`}</h2>
            <div className="muted">{day.day} · stored as you see it now; new evidence never changes it silently</div>
          </div>
        </header>
        <div className="body">
          <dl className="kv">
            <dt>Counted</dt>
            <dd className="mono">{duration(counted)}</dd>
            {Object.entries(byTarget)
              .sort((a, b) => b[1] - a[1])
              .map(([t, ms]) => (
                <div key={t} style={{ display: "contents" }}>
                  <dt className="muted">· {t}</dt>
                  <dd className="mono">{duration(ms)}</dd>
                </div>
              ))}
            <dt>Unknown</dt>
            <dd className="mono">{duration(unknown)}</dd>
            <dt>Break</dt>
            <dd className="mono">{duration(nonWork)}</dd>
          </dl>
          {unknown > 0 && (
            <p className="note">
              Unknown time stays unknown and is not counted in the reviewed day. Count or exclude it first if you know
              what it was.
            </p>
          )}
          <label className="field" style={{ marginTop: "var(--space-4)" }}>
            <span>{next === 1 ? "Note (optional)" : "Reason (required)"}</span>
            <input className="input" value={reason} maxLength={200} onChange={(e) => setReason(e.target.value)} />
          </label>
          <div className="actions">
            <button className="button ghost" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={busy || (next > 1 && !reason.trim())} onClick={review}>
              {busy ? <span className="spinner" /> : next === 1 ? "Review" : `Save r${next}`}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

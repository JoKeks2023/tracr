import { useEffect, useState } from "react";
import { useSheet } from "./useSheet";
import { t } from "./i18n";
import { api, errorText } from "./api";
import { duration } from "./format";
import type { DayAgents } from "./types";

/** Review the day: freeze its counting as the next revision (AD-18). */
export function ReviewDaySheet({ day, onClose, onChanged }: {
  day: DayAgents;
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
}) {
  const sheetRef = useSheet<HTMLElement>();
  const next = (day.review?.rev ?? 0) + 1;
  const [reason, setReason] = useState(day.discrepancy ? t("acceptedNew") : "");
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
      onChanged(next > 1 ? t("reviewUpdatedToast") : t("reviewedToast"));
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
      <aside ref={sheetRef} aria-modal="true" className="sheet export-sheet" role="dialog" aria-label={t("reviewDay")}>
        <header>
          <div style={{ flex: 1 }}>
            <h2>{next === 1 ? t("reviewDay") : t("acceptRevision", { rev: next })}</h2>
            <div className="muted">{t("reviewDayNote", { day: day.day })}</div>
          </div>
        </header>
        <div className="body">
          <dl className="kv">
            <dt>{t("countedWord")}</dt>
            <dd className="mono">{duration(counted)}</dd>
            {Object.entries(byTarget)
              .sort((a, b) => b[1] - a[1])
              .map(([label, ms]) => (
                <div key={label} style={{ display: "contents" }}>
                  <dt className="muted">· {bucketLabel(label)}</dt>
                  <dd className="mono">{duration(ms)}</dd>
                </div>
              ))}
            <dt>{t("unknownWord")}</dt>
            <dd className="mono">{duration(unknown)}</dd>
            <dt>{t("breakWord")}</dt>
            <dd className="mono">{duration(nonWork)}</dd>
          </dl>
          {unknown > 0 && (
            <p className="note">{t("unknownStays")}</p>
          )}
          <label className="field" style={{ marginTop: "var(--space-4)" }}>
            <span>{next === 1 ? t("noteOptional") : t("reasonRequired")}</span>
            <input className="input" value={reason} maxLength={200} onChange={(e) => setReason(e.target.value)} />
          </label>
          <div className="actions">
            <button className="btn s" onClick={onClose}>
              {t("cancel")}
            </button>
            <button className="btn p" disabled={busy || (next > 1 && !reason.trim())} onClick={review}>
              {busy ? <span className="spinner" /> : next === 1 ? t("reviewDayShort") : t("saveRev", { rev: next })}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

/** Counted time without a resolved target comes from the backend as a state word. */
function bucketLabel(label: string): string {
  if (label === "unattributed") return t("unattributed");
  if (label === "conflicting") return t("conflictingWord");
  if (label === "ambiguous") return t("ambiguousWord");
  return label;
}

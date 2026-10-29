import { useEffect, useState } from "react";
import { t } from "./i18n";
import { api, errorText } from "./api";
import { duration, hhmm } from "./format";
import { statusText } from "./IntervalSheet";
import type { DayAgents, HumanAtom } from "./types";

/** The day's human time to review: unknown windows first, then counted time. */
export function ReviewList({ day, onOpen, onClose, onChanged }: {
  day: DayAgents;
  onOpen: (a: HumanAtom) => void;
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
}) {
  const tz = day.timezone;
  const [busy, setBusy] = useState<number | null>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const order = { unknown: 0, work: 1, non_work: 2 } as const;
  const atoms = [...day.human.atoms].sort((a, b) => order[a.status] - order[b.status] || a.start - b.start);
  const quick = async (a: HumanAtom, op: "confirm_work" | "mark_non_work") => {
    setBusy(a.start);
    try {
      await api.correct({ op, start: a.start, end: a.end, ui_op: "review_list" });
      onChanged(op === "confirm_work" ? t("countedAsWork") : t("markedBreak"));
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet" role="dialog" aria-label={t("reviewTheDay")}>
        <header>
          <div style={{ flex: 1 }}>
            <h2>{t("reviewTheDay")}</h2>
            <div className="muted">
              {t("unknownWindows", { n: atoms.filter((a) => a.status === "unknown").length, time: duration(day.human.unknown_ms) })}
            </div>
          </div>
          <button className="button ghost" onClick={onClose} style={{ minWidth: 0 }}>
            {t("close")}
          </button>
        </header>
        <div className="body">
          {atoms.length === 0 && <p className="muted">{t("noHumanTime")}</p>}
          <ul className="review">
            {atoms.map((a) => (
              <li key={`${a.start}-${a.status}`} className={`review-row ${a.status}`}>
                <button className="review-main" onClick={() => onOpen(a)}>
                  <span className="mono">
                    {hhmm(a.start, tz)}–{hhmm(a.end, tz)}
                  </span>
                  <span className="mono muted">{duration(a.end - a.start)}</span>
                  <span className="review-status">{statusText(a)}</span>
                </button>
                {a.status === "unknown" && (
                  <span className="review-actions">
                    <button className="button secondary small" disabled={busy !== null} onClick={() => quick(a, "confirm_work")}>
                      {t("work")}
                    </button>
                    <button className="button secondary small" disabled={busy !== null} onClick={() => quick(a, "mark_non_work")}>
                      {t("breakWord")}
                    </button>
                  </span>
                )}
              </li>
            ))}
          </ul>
        </div>
      </aside>
    </>
  );
}

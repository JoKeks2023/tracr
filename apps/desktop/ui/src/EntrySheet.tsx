import { useEffect, useState } from "react";
import { useSheet } from "./useSheet";
import { t } from "./i18n";
import { api, errorText } from "./api";
import { toMs, toTime } from "./format";
import { TargetSelect, targetKey } from "./TargetSelect";
import type { ManualEntryInfo, TargetOption } from "./types";

/** Add a retroactive entry, or edit an existing entry. */
export function EntrySheet({ day, tz, entry, targets, onClose, onChanged }: {
  day: string;
  tz: string;
  entry: ManualEntryInfo | null;
  targets: TargetOption[];
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
}) {
  const sheetRef = useSheet<HTMLElement>();
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
      onChanged(t("endAfterStart"), true);
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
      onChanged(entry ? t("entryUpdated") : t("timeAdded"));
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
      onChanged(t("entryDeleted"));
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
      <aside ref={sheetRef} aria-modal="true" className="sheet export-sheet" role="dialog" aria-label={entry ? t("editEntry") : t("addTime")}>
        <header>
          <div style={{ flex: 1 }}>
            <h2>{entry ? (entry.kind === "timer" ? t("editTimer") : t("editEntry")) : t("addTime")}</h2>
            <div className="muted">{t("entryNote", { day })}</div>
          </div>
        </header>
        <div className="body">
          <div className="fields">
            <label className="field">
              <span>{t("from")}</span>
              <input className="input" type="time" value={start} onChange={(e) => setStart(e.target.value)} />
            </label>
            <label className="field">
              <span>{t("to")}</span>
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
            <span>{t("for")}</span>
            <div className="target-row">
              <TargetSelect targets={targets} value={target} onChange={setTarget} other={other} onOther={setOther} allowNone />
            </div>
          </label>
          <label className="field">
            <span>{t("noteLabel")}</span>
            <input className="input" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} placeholder={t("optional")} />
          </label>
          <div className="actions">
            {entry && (
              <button className="button ghost" disabled={busy} onClick={remove} style={{ marginRight: "auto" }}>
                {t("delete")}
              </button>
            )}
            <button className="button ghost" onClick={onClose}>
              {t("cancel")}
            </button>
            <button className="button primary" disabled={busy} onClick={save}>
              {busy ? <span className="spinner" /> : entry ? t("save") : t("add")}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

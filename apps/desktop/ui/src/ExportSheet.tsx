import { useEffect, useMemo, useState } from "react";
import { useSheet } from "./useSheet";
import { t } from "./i18n";
import { api, errorText } from "./api";
import { dayLabel } from "./format";

type Kind = "timesheet" | "agent_sessions" | "everything";

const KINDS: Record<Kind, { label: () => string; note: () => string }> = {
  timesheet: {
    label: () => t("exportKindTimesheet"),
    note: () => t("exportNoteTimesheet"),
  },
  agent_sessions: {
    label: () => t("exportKindSessions"),
    note: () => t("exportNoteSessions"),
  },
  everything: {
    label: () => t("exportKindEverything"),
    note: () => t("exportNoteEverything"),
  },
};

/** Days are newest first. Defaults to the month of the day being viewed. */
export function ExportSheet({ days, current, onClose, onDone }: {
  days: string[];
  current: string | null;
  onClose: () => void;
  onDone: (text: string, error?: boolean) => void;
}) {
  const sheetRef = useSheet<HTMLElement>();
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
      if (path) onDone(t("exportedTo", { path }));
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
      <aside ref={sheetRef} aria-modal="true" className="sheet export-sheet" role="dialog" aria-label={t("exportTitle")}>
        <header>
          <div style={{ flex: 1 }}>
            <h2>{t("exportTitle")}</h2>
            <div className="muted">{KINDS[kind].label()}</div>
          </div>
        </header>
        <div className="body">
          <label className="field">
            <span>{t("exportWhat")}</span>
            <select className="select" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>
              {(Object.keys(KINDS) as Kind[]).map((k) => (
                <option key={k} value={k}>
                  {KINDS[k].label()}
                </option>
              ))}
            </select>
          </label>
          {kind !== "everything" && (
            <div className="fields">
              {select(from, setFrom, t("from"))}
              {select(to, setTo, t("to"))}
            </div>
          )}
          <p className="note">{KINDS[kind].note()}</p>
          <div className="actions">
            <button className="button ghost" onClick={onClose}>
              {t("cancel")}
            </button>
            <button className="button primary" onClick={run} disabled={busy || (kind !== "everything" && (!from || !to || to < from))}>
              {busy ? <span className="spinner" aria-label="…" /> : t("exportAction")}
            </button>
          </div>
        </div>
      </aside>
    </>
  );
}

import { useEffect, useMemo, useState } from "react";
import { useSheet } from "./useSheet";
import { t } from "./i18n";
import { api, errorText } from "./api";
import { duration, hhmm } from "./format";
import { SplitEditor, requestsFor } from "./v2/SplitEditor";
import type { CorrectionInfo, CorrectionRequest, DayAgents, HumanAtom, ManualEntryInfo, TargetOption } from "./types";

function whyUnknown(reason: string | null): string {
  const known: Record<string, string> = {
    coverage_gap: t("whyCoverage"),
    conflicting_evidence: t("whyConflict"),
    unresolved_window: t("whyUnresolved"),
  };
  return known[reason ?? ""] ?? reason ?? "";
}

function opText(op: CorrectionInfo["op"]): string {
  return {
    confirm_work: t("countedAsWork"),
    mark_non_work: t("markedBreak"),
    set_target: t("targetSet"),
    reject_target: t("targetRemoved"),
    revoke: t("undone"),
  }[op];
}

export function statusText(a: HumanAtom): string {
  if (a.status === "work") {
    if (a.duration_basis === "inferred_continuity") return t("statCountedInferred");
    if (a.duration_basis === "timer_measured") return t("statCountedTimer");
    return a.manual_entry && !a.decided_by ? t("statCountedEntry") : t("statCountedConfirmed");
  }
  if (a.status === "non_work") return t("statBreak");
  return t("statUnknown", { why: whyUnknown(a.unknown_reason) });
}

function targetText(a: HumanAtom): string {
  const at = a.attribution;
  if (!at || a.status !== "work") return "—";
  if (at.state === "confirmed") {
    return `${at.labels.join(", ")}${at.level === "explicit" ? "" : ` · ${t("fromContext", { level: at.level })}`}`;
  }
  return at.state === "conflicting" ? t("conflictingLabels", { labels: at.labels.join(", ") }) : t("unattributed");
}

export function IntervalSheet({ atom, day, targets, onClose, onChanged, onEditEntry }: {
  atom: HumanAtom;
  day: DayAgents;
  targets: TargetOption[];
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
  onEditEntry: (e: ManualEntryInfo) => void;
}) {
  const sheetRef = useSheet<HTMLElement>();
  const tz = day.timezone;
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const history = useMemo(
    () =>
      day.corrections.filter((c) => {
        const range = c.op === "revoke" ? day.corrections.find((x) => x.id === c.revokes) : c;
        return range?.start != null && range.end != null && range.start < atom.end && range.end > atom.start;
      }),
    [day, atom],
  );
  const explicit = atom.attribution?.level === "explicit" ? atom.attribution.targets[0] : null;

  const run = async (req: CorrectionRequest, message: string) => {
    setBusy(true);
    try {
      await api.correct({ start: atom.start, end: atom.end, ui_op: "interval_panel", ...req });
      onChanged(message);
      onClose();
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  // Several parts, or counting and putting on a target, are one step: all recorded or none.
  const runBatch = async (reqs: CorrectionRequest[], message: string) => {
    setBusy(true);
    try {
      await api.correctBatch(reqs);
      onChanged(message);
      onClose();
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  // What tracr suggests (inferred or ambiguous), as the targets the user can confirm or reject.
  const at = atom.attribution;
  const suggestions =
    at && at.level === "repository" && at.state !== "confirmed"
      ? at.targets.map((k, i) => ({ key: `repository:${k}`, label: at.labels[i] ?? k }))
      : [];

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside ref={sheetRef} aria-modal="true" className="sheet interval-sheet" role="dialog" aria-label={t("interval")}>
        <header>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2>
              {hhmm(atom.start, tz)}–{hhmm(atom.end, tz)} <span className="muted">· {duration(atom.end - atom.start)}</span>
            </h2>
            <div className={`state-${atom.status}`}>{statusText(atom)}</div>
          </div>
          <button className="button ghost" onClick={onClose} style={{ minWidth: 0 }}>
            {t("close")}
          </button>
        </header>
        <div className="body">
          {atom.decided_by && (
            <p className="note">
              {t("decidedBy", {
                what: `${atom.candidate_status === "blank" ? t("noEvidenceBlank") : atom.candidate_status}${atom.candidate_reason ? ` (${whyUnknown(atom.candidate_reason)})` : ""}`,
              })}
            </p>
          )}

          {atom.manual_entry &&
            (() => {
              const e = day.manual_entries.find((m) => m.id === atom.manual_entry);
              return e ? (
                <section>
                  <h3>{e.kind === "timer" ? t("yourTimer") : t("yourEntry")}</h3>
                  <p className="current">
                    {hhmm(e.start, tz)}–{e.end ? hhmm(e.end, tz) : t("running")}
                    {e.target_label ? ` · ${e.target_label}` : ""}
                    {e.note ? ` · ${e.note}` : ""}
                  </p>
                  <button className="button secondary" onClick={() => onEditEntry(e)}>
                    {t("editOrDelete")}
                  </button>
                </section>
              ) : null;
            })()}

          {suggestions.length > 0 && (
            <section>
              <h3>{t("suggestionLabel")}</h3>
              <ul className="split-parts">
                {suggestions.map((sg) => (
                  <li key={sg.key}>
                    <span>{sg.label}</span>
                    <span className="acts">
                      <button className="btn s" disabled={busy} onClick={() => runBatch(requestsFor(atom, atom.start, atom.end, sg.key, "", "suggestion"), t("targetSet"))}>
                        {t("suggestionConfirm")}
                      </button>
                      <button className="btn s" disabled={busy} onClick={() => run({ op: "reject_target", target: sg.key }, t("targetRemoved"))}>
                        {t("suggestionReject")}
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {atom.status === "work" && <p className="current">{targetText(atom)}</p>}
          <SplitEditor atom={atom} day={day} targets={targets} busy={busy} onApply={runBatch} />
          <div className="choices">
            {atom.decided_by && (
              <button className="button ghost" disabled={busy} onClick={() => run({ op: "revoke", revokes: atom.decided_by ?? undefined }, t("undone"))}>
                {t("undo")}
              </button>
            )}
            {explicit && (
              <button className="button ghost" disabled={busy} onClick={() => run({ op: "reject_target", target: explicit }, t("targetRemoved"))}>
                {t("removeTarget")}
              </button>
            )}
          </div>

          {history.length > 0 && (
            <section>
              <h3>{t("yourCorrections")}</h3>
              <ul className="history">
                {history.map((c) => (
                  <li key={c.id} className={c.status === "effective" ? undefined : "muted"}>
                    {opText(c.op)}
                    {c.target_label ? ` · ${c.target_label}` : ""}
                    {c.start != null && c.end != null ? ` · ${hhmm(c.start, tz)}–${hhmm(c.end, tz)}` : ""}
                    <span className="muted"> · {c.status === "effective" ? t("effective") : c.status === "superseded" ? t("superseded") : c.status === "revoked" ? t("revoked") : c.status}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>
      </aside>
    </>
  );
}

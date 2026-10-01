import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { duration, hhmm } from "./format";
import type { CorrectionInfo, CorrectionRequest, DayAgents, HumanAtom, ManualEntryInfo, TargetOption } from "./types";

const REASON: Record<string, string> = {
  coverage_gap: "tracr was not collecting everything needed",
  conflicting_evidence: "prompts in several sessions interleave",
  unresolved_window: "the gap could not be bridged",
};

const OP_TEXT: Record<CorrectionInfo["op"], string> = {
  confirm_work: "Counted as work",
  mark_non_work: "Marked as break",
  set_target: "Target set",
  reject_target: "Target removed",
  revoke: "Undone",
};

export function statusText(a: HumanAtom): string {
  if (a.status === "work") {
    if (a.duration_basis === "inferred_continuity") return "Counted · inferred";
    if (a.duration_basis === "timer_measured") return "Counted · your timer";
    return a.manual_entry && !a.decided_by ? "Counted · your entry" : "Counted · confirmed by you";
  }
  if (a.status === "non_work") return "Break · excluded by you";
  return `Unknown · ${REASON[a.unknown_reason ?? ""] ?? a.unknown_reason}`;
}

function targetText(a: HumanAtom): string {
  const at = a.attribution;
  if (!at || a.status !== "work") return "—";
  if (at.state === "confirmed") {
    return `${at.labels.join(", ")}${at.level === "explicit" ? "" : ` · from ${at.level} context`}`;
  }
  return at.state === "conflicting" ? `conflicting: ${at.labels.join(", ")}` : "unattributed";
}

export function IntervalSheet({ atom, day, targets, onClose, onChanged, onEditEntry }: {
  atom: HumanAtom;
  day: DayAgents;
  targets: TargetOption[];
  onClose: () => void;
  onChanged: (message: string, error?: boolean) => void;
  onEditEntry: (e: ManualEntryInfo) => void;
}) {
  const tz = day.timezone;
  const [busy, setBusy] = useState(false);
  const [choice, setChoice] = useState("");
  const [other, setOther] = useState("");

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

  const setTarget = () => {
    const key = choice === "__other" ? `other:${other.trim()}` : choice;
    if (!key || key === "other:") return;
    void run({ op: "set_target", target: key }, "Target set");
  };

  const groups: [TargetOption["group"], string][] = [
    ["repository", "Repositories"],
    ["category", "Categories"],
    ["other", "Used before"],
  ];

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet interval-sheet" role="dialog" aria-label="Interval">
        <header>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2>
              {hhmm(atom.start, tz)}–{hhmm(atom.end, tz)} <span className="muted">· {duration(atom.end - atom.start)}</span>
            </h2>
            <div className={`state-${atom.status}`}>{statusText(atom)}</div>
          </div>
          <button className="button ghost" onClick={onClose} style={{ minWidth: 0 }}>
            Close
          </button>
        </header>
        <div className="body">
          {atom.decided_by && (
            <p className="note">
              Decided by your correction. Underneath, tracr derived:{" "}
              {atom.candidate_status === "blank" ? "no evidence (blank)" : atom.candidate_status}
              {atom.candidate_reason ? ` (${atom.candidate_reason.replaceAll("_", " ")})` : ""}.
            </p>
          )}

          {atom.manual_entry &&
            (() => {
              const e = day.manual_entries.find((m) => m.id === atom.manual_entry);
              return e ? (
                <section>
                  <h3>{e.kind === "timer" ? "Your timer" : "Your entry"}</h3>
                  <p className="current">
                    {hhmm(e.start, tz)}–{e.end ? hhmm(e.end, tz) : "running"}
                    {e.target_label ? ` · ${e.target_label}` : ""}
                    {e.note ? ` · ${e.note}` : ""}
                  </p>
                  <button className="button secondary" onClick={() => onEditEntry(e)}>
                    Edit or delete
                  </button>
                </section>
              ) : null;
            })()}

          <section>
            <h3>Does this time count?</h3>
            <div className="choices">
              <button
                className="button secondary"
                disabled={busy || (atom.status === "work" && atom.duration_basis !== "inferred_continuity")}
                onClick={() => run({ op: "confirm_work" }, "Counted as work")}
              >
                Count as work
              </button>
              <button
                className="button secondary"
                disabled={busy || atom.status === "non_work"}
                onClick={() => run({ op: "mark_non_work" }, "Marked as break")}
              >
                Mark as break
              </button>
              {atom.decided_by && (
                <button
                  className="button ghost"
                  disabled={busy}
                  onClick={() => run({ op: "revoke", revokes: atom.decided_by ?? undefined }, "Undone")}
                >
                  Undo
                </button>
              )}
            </div>
            <p className="note">
              Unknown time is never counted on its own. Counting it is your statement; tracr keeps the derivation
              underneath and shows both.
            </p>
          </section>

          <section>
            <h3>What was it for?</h3>
            <p className="current">{targetText(atom)}</p>
            <div className="target-row">
              <select className="select" value={choice} onChange={(e) => setChoice(e.target.value)} aria-label="Target">
                <option value="">Choose a target…</option>
                {groups.map(([g, label]) => {
                  const items = targets.filter((t) => t.group === g);
                  return items.length === 0 ? null : (
                    <optgroup key={g} label={label}>
                      {items.map((t) => (
                        <option key={t.key} value={t.key}>
                          {t.label}
                        </option>
                      ))}
                    </optgroup>
                  );
                })}
                <option value="__other">Other…</option>
              </select>
              {choice === "__other" && (
                <input
                  className="input"
                  placeholder="Short label"
                  maxLength={80}
                  value={other}
                  onChange={(e) => setOther(e.target.value)}
                />
              )}
              <button className="button primary" disabled={busy || !choice} onClick={setTarget}>
                Set
              </button>
            </div>
            {explicit && (
              <button
                className="button ghost"
                disabled={busy}
                onClick={() => run({ op: "reject_target", target: explicit }, "Target removed")}
              >
                Remove target
              </button>
            )}
            <p className="note">A target never changes the minutes, and never changes agent sessions or tokens.</p>
          </section>

          {history.length > 0 && (
            <section>
              <h3>Your corrections here</h3>
              <ul className="history">
                {history.map((c) => (
                  <li key={c.id} className={c.status === "effective" ? undefined : "muted"}>
                    {OP_TEXT[c.op]}
                    {c.target_label ? ` · ${c.target_label}` : ""}
                    {c.start != null && c.end != null ? ` · ${hhmm(c.start, tz)}–${hhmm(c.end, tz)}` : ""}
                    <span className="muted"> · {c.status}</span>
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

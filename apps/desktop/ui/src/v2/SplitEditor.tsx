import { useMemo, useState } from "react";
import { duration, hhmm, toMs, toTime } from "../format";
import { t } from "../i18n";
import { TargetSelect } from "../TargetSelect";
import type { CorrectionRequest, DayAgents, HumanAtom, TargetOption } from "../types";

const BREAK = "__break";
const WORK = "__work";
const MIN_PART = 60_000;

/** A part's destination → the corrections that make it so. Nothing is sent for "leave as is". */
export function requestsFor(atom: HumanAtom, start: number, end: number, dest: string, other: string, uiOp: string): CorrectionRequest[] {
  const range = { start, end, ui_op: uiOp };
  if (dest === "") return [];
  if (dest === BREAK) return [{ op: "mark_non_work", ...range }];
  const counts = atom.status === "work";
  if (dest === WORK) return counts && atom.duration_basis !== "inferred_continuity" ? [] : [{ op: "confirm_work", ...range }];
  const key = dest === "__other" ? (other.trim() ? `other:${other.trim()}` : "") : dest;
  if (!key) return [];
  // Time that already counts only needs its target; anything else is first counted by this statement.
  return [...(counts ? [] : [{ op: "confirm_work" as const, ...range }]), { op: "set_target" as const, target: key, ...range }];
}

/**
 * Divide a window into parts and say what each is: a project, a repository, a
 * category, a break, or leave it. The parts add up to the window; nothing is
 * split evenly on its own. All parts are sent as one step.
 */
export function SplitEditor({
  atom,
  day,
  targets,
  busy,
  onApply,
}: {
  atom: HumanAtom;
  day: DayAgents;
  targets: TargetOption[];
  busy: boolean;
  onApply: (reqs: CorrectionRequest[], message: string) => void;
}) {
  const tz = day.timezone;
  const explicit = atom.attribution?.level === "explicit" ? atom.attribution.targets[0] : "";
  const [cuts, setCuts] = useState<number[]>([]);
  const [dest, setDest] = useState<string[]>([targets.some((o) => o.key === explicit) ? explicit : ""]);
  const [other, setOther] = useState<string[]>([""]);
  const [time, setTime] = useState(() => toTime(Math.round((atom.start + atom.end) / 2 / 60_000) * 60_000, day.timezone));
  const [error, setError] = useState<string | null>(null);

  const bounds = useMemo(() => [atom.start, ...cuts, atom.end], [atom, cuts]);
  const parts = bounds.slice(0, -1).map((s, i) => ({ start: s, end: bounds[i + 1], i }));

  const cutAt = (ms: number) => {
    const part = parts.find((p) => ms > p.start && ms < p.end);
    if (!part || ms - part.start < MIN_PART || part.end - ms < MIN_PART) {
      setError(t("splitTooShort"));
      return;
    }
    setError(null);
    setCuts([...cuts, ms].sort((a, b) => a - b));
    // Both halves start with what the part had.
    setDest([...dest.slice(0, part.i + 1), dest[part.i], ...dest.slice(part.i + 1)]);
    setOther([...other.slice(0, part.i + 1), other[part.i], ...other.slice(part.i + 1)]);
  };
  const cutTyped = () => {
    const ms = toMs(day.day, time, tz);
    if (ms === null) return setError(t("splitPickTime"));
    cutAt(ms);
  };
  const cutMiddle = () => {
    // The middle of the longest part, to the minute.
    const longest = [...parts].sort((a, b) => b.end - b.start - (a.end - a.start))[0];
    cutAt(Math.round((longest.start + longest.end) / 2 / 60_000) * 60_000);
  };
  const merge = (i: number) => {
    setCuts(cuts.filter((_, k) => k !== i));
    setDest(dest.filter((_, k) => k !== i + 1));
    setOther(other.filter((_, k) => k !== i + 1));
  };

  const reqs = parts.flatMap((p) => requestsFor(atom, p.start, p.end, dest[p.i], other[p.i], "split"));
  const apply = () => {
    if (reqs.length === 0) return;
    onApply(reqs, parts.length > 1 ? t("splitDone", { n: parts.length }) : t("targetSet"));
  };
  const extras = [
    { value: "", label: t("splitLeave") },
    { value: WORK, label: t("splitWork") },
    { value: BREAK, label: t("splitBreak") },
  ];
  const label = (d: string, o: string) =>
    d === "" ? t("splitLeave") : d === BREAK ? t("splitBreak") : d === WORK ? t("splitWork") : d === "__other" ? o || t("otherEllipsis") : (targets.find((x) => x.key === d)?.label ?? d);

  return (
    <section className="split">
      <h3>{t("splitTitle")}</h3>
      <div className="split-bar" role="img" aria-label={parts.map((p) => `${hhmm(p.start, tz)}–${hhmm(p.end, tz)} ${label(dest[p.i], other[p.i])}`).join(", ")}>
        {parts.map((p) => (
          <div key={p.i} className={`split-seg ${dest[p.i] === BREAK ? "brk" : dest[p.i] === "" ? "leave" : "on"}`} style={{ flex: p.end - p.start }} title={`${hhmm(p.start, tz)}–${hhmm(p.end, tz)}`}>
            <span className="mono">{duration(p.end - p.start)}</span>
          </div>
        ))}
      </div>

      <ul className="split-parts">
        {parts.map((p) => (
          <li key={p.i}>
            <span className="mono">
              {hhmm(p.start, tz)}–{hhmm(p.end, tz)}
            </span>
            <span className="mono vnote">{duration(p.end - p.start)}</span>
            <TargetSelect
              targets={targets}
              value={dest[p.i]}
              onChange={(v) => setDest(dest.map((d, k) => (k === p.i ? v : d)))}
              other={other[p.i]}
              onOther={(v) => setOther(other.map((o, k) => (k === p.i ? v : o)))}
              extras={extras}
              allowNone
            />
            {p.i < parts.length - 1 && (
              <button className="btn s" type="button" onClick={() => merge(p.i)} title={t("splitMergeHint")}>
                {t("splitMerge")}
              </button>
            )}
          </li>
        ))}
      </ul>

      <div className="split-cut">
        <input type="time" className="sinput" value={time} onChange={(e) => setTime(e.target.value)} aria-label={t("splitAt")} />
        <button className="btn" type="button" disabled={busy || time === ""} onClick={cutTyped}>
          {t("splitAt")}
        </button>
        <button className="btn" type="button" disabled={busy} onClick={cutMiddle}>
          {t("splitMiddle")}
        </button>
        <span className="grow" />
        <button className="btn p" type="button" disabled={busy || reqs.length === 0} onClick={apply}>
          {t("splitApply")}
        </button>
      </div>
      {error && <p className="vnote" role="alert">{error}</p>}
      <p className="vnote">{t("splitNote", { start: toTime(atom.start, tz), end: toTime(atom.end, tz) })}</p>
    </section>
  );
}

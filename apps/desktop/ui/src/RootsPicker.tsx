import { useState } from "react";
import { tildify } from "./format";

/**
 * Proposed provider folders as a checklist, all checked, approved with one
 * click (PS-02: nothing is read before the user does this).
 */
export function RootsPicker({
  paths,
  home,
  busy,
  onApprove,
  label = "Approve selected",
}: {
  paths: string[];
  home: string | null;
  busy: boolean;
  onApprove: (paths: string[]) => void;
  label?: string;
}) {
  const [off, setOff] = useState<Set<string>>(new Set());
  const chosen = paths.filter((p) => !off.has(p));
  const toggle = (p: string) =>
    setOff((cur) => {
      const next = new Set(cur);
      if (next.has(p)) next.delete(p);
      else next.add(p);
      return next;
    });
  return (
    <div className="roots-picker">
      <ul className="checklist">
        {paths.map((p) => (
          <li key={p}>
            <label>
              <input type="checkbox" checked={!off.has(p)} onChange={() => toggle(p)} disabled={busy} />
              <span className="mono" title={p}>
                {tildify(p, home)}
              </span>
            </label>
          </li>
        ))}
      </ul>
      <button className="button primary" disabled={busy || chosen.length === 0} onClick={() => onApprove(chosen)}>
        {busy ? <span className="spinner" aria-label="Approving" /> : `${label}${chosen.length > 1 ? ` (${chosen.length})` : ""}`}
      </button>
    </div>
  );
}

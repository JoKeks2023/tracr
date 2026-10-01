import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import { TargetSelect, targetKey } from "./TargetSelect";
import type { ManualEntryInfo, TargetOption } from "./types";

function elapsed(start: number): string {
  const s = Math.max(0, Math.floor((Date.now() - start) / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${h}:${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

/** Start or stop the one timer (AD-15). */
export function TimerControl({ running, loadTargets, targets, onChanged }: {
  running: ManualEntryInfo | null;
  loadTargets: () => void;
  targets: TargetOption[];
  onChanged: (message: string, error?: boolean) => void;
}) {
  const [, tick] = useState(0);
  const [open, setOpen] = useState(false);
  const [target, setTarget] = useState("");
  const [other, setOther] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!running) return;
    const t = window.setInterval(() => tick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, [running]);

  const call = async (op: "start_timer" | "stop_timer") => {
    setBusy(true);
    try {
      await api.manual({ op, target: op === "start_timer" ? targetKey(target, other) || undefined : undefined });
      onChanged(op === "start_timer" ? "Timer started" : "Timer stopped");
      setOpen(false);
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };

  if (running) {
    return (
      <span className="timer running">
        <span className="dot recording" aria-hidden />
        <span className="mono">{elapsed(running.start)}</span>
        {running.target_label && <span className="muted">· {running.target_label}</span>}
        <button className="button secondary small" disabled={busy} onClick={() => call("stop_timer")}>
          Stop
        </button>
      </span>
    );
  }
  return (
    <span className="timer">
      <button
        className="button ghost"
        onClick={() => {
          loadTargets();
          setOpen(!open);
        }}
        aria-expanded={open}
      >
        Start timer
      </button>
      {open && (
        <div className="popover" role="dialog" aria-label="Start timer">
          <div className="target-row">
            <TargetSelect targets={targets} value={target} onChange={setTarget} other={other} onOther={setOther} allowNone />
          </div>
          <div className="actions">
            <button className="button ghost" onClick={() => setOpen(false)}>
              Cancel
            </button>
            <button className="button primary" disabled={busy} onClick={() => call("start_timer")}>
              Start
            </button>
          </div>
        </div>
      )}
    </span>
  );
}

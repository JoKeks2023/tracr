import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../api";
import { duration } from "../format";
import { t } from "../i18n";
import { TargetSelect, targetKey } from "../TargetSelect";
import type { CorrectionRequest, DayAgents, TargetOption } from "../types";
import { Panel } from "./ui";

/**
 * The day in a few steps: put all unassigned time on one target, confirm what
 * tracr suggests, or say that the unclear time was work. Each is one step,
 * recorded as the user's statement; unclear time is never counted on its own.
 */
export function DayTools({
  view,
  targets,
  loadTargets,
  onChanged,
}: {
  view: DayAgents;
  targets: TargetOption[];
  loadTargets: () => void;
  onChanged: (message: string, error?: boolean) => void;
}) {
  const [choice, setChoice] = useState("");
  const [other, setOther] = useState("");
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (targets.length === 0) loadTargets();
  }, [targets.length, loadTargets]);

  const sets = useMemo(() => {
    const atoms = view.human.atoms;
    const work = atoms.filter((a) => a.status === "work");
    return {
      unassigned: work.filter((a) => !a.attribution || a.attribution.state !== "confirmed"),
      unclear: atoms.filter((a) => a.status === "unknown"),
    };
  }, [view]);
  const sum = (xs: { start: number; end: number }[]) => xs.reduce((n, a) => n + (a.end - a.start), 0);
  if (sets.unassigned.length + sets.unclear.length === 0) return null;

  const send = async (reqs: CorrectionRequest[]) => {
    if (reqs.length === 0) return;
    setBusy(true);
    try {
      await api.correctBatch(reqs);
      onChanged(t("dayToolsDone"));
      setAsking(false);
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  const key = targetKey(choice, other);

  return (
    <Panel title={t("dayToolsTitle")}>
      {sets.unassigned.length > 0 && (
        <div className="srow">
          <div className="lbl">
            {t("dayToolsUnassigned", { n: sets.unassigned.length, time: duration(sum(sets.unassigned)) })}
          </div>
          <div className="ctl acts">
            <TargetSelect targets={targets} value={choice} onChange={setChoice} other={other} onOther={setOther} />
            <button
              className="btn"
              disabled={busy || !key}
              onClick={() => send(sets.unassigned.map((a) => ({ op: "set_target" as const, start: a.start, end: a.end, target: key, ui_op: "day_tools" })))}
            >
              {t("dayToolsAssign")}
            </button>
          </div>
        </div>
      )}
      {sets.unclear.length > 0 && (
        <div className="srow">
          <div className="lbl">
            <div>{t("dayToolsCountUnclear")}</div>
            {asking && <div className="vnote">{t("dayToolsCountUnclearAsk", { time: duration(sum(sets.unclear)), n: sets.unclear.length })}</div>}
          </div>
          <div className="ctl acts">
            {asking ? (
              <>
                <button className="btn" disabled={busy} onClick={() => send(sets.unclear.map((a) => ({ op: "confirm_work" as const, start: a.start, end: a.end, ui_op: "day_tools" })))}>
                  {t("dayToolsCountYes")}
                </button>
                <button className="btn" disabled={busy} onClick={() => setAsking(false)}>
                  {t("cancel")}
                </button>
              </>
            ) : (
              <button className="btn" disabled={busy} onClick={() => setAsking(true)}>
                {t("dayToolsCountUnclear")}
              </button>
            )}
          </div>
        </div>
      )}
    </Panel>
  );
}

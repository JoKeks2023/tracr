import { useEffect, useState } from "react";
import { useSheet } from "./useSheet";
import { t } from "./i18n";
import { api, errorText } from "./api";
import { SessionDetail } from "./SessionDetail";
import type { PricingInfo, SessionDay, TargetOption } from "./types";

/** A target key as a person reads it: the project's name, else the key without its kind. */
function targetName(key: string, targets: TargetOption[]): string {
  return targets.find((o) => o.key === key)?.label ?? key.replace(/^(other|category|project):/, "");
}

/**
 * What the session is for (TRACR-40): its bindings, and issue keys from the
 * branch name. A suggestion counts only once bound; a rejected one is not
 * suggested again for this session.
 */
function SessionTargets({ s, targets, onChanged }: {
  s: SessionDay;
  targets: TargetOption[];
  onChanged: (message: string, error?: boolean) => void;
}) {
  const [busy, setBusy] = useState(false);
  if (s.bound_to.length === 0 && s.suggestions.length === 0) return null;
  const subject = { kind: "session" as const, id: s.native_session_id };
  const run = async (op: Parameters<typeof api.bindingOp>[0], done: string) => {
    setBusy(true);
    try {
      await api.bindingOp(op);
      onChanged(done);
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section>
      <h3>{t("sessionTargets")}</h3>
      {s.bound_to.length > 0 && (
        <p>{t("boundTo", { list: s.bound_to.map((k) => targetName(k, targets)).join(", ") })}</p>
      )}
      {s.suggestions.map((g) => (
        <div key={g.target} className="srow">
          <div className="lbl">
            {t("suggestedTarget", { name: g.label })} <span className="muted">{t("fromBranch")}</span>
          </div>
          <div className="ctl acts">
            <button
              className="btn p"
              disabled={busy}
              onClick={() => run({ op: "bind", subject, target: g.target, valid_from: s.first_evidence_at }, t("boundDone", { name: g.label }))}
            >
              {t("bindAction")}
            </button>
            <button className="btn s" disabled={busy} onClick={() => run({ op: "reject", subject, target: g.target }, t("rejectedDone", { name: g.label }))}>
              {t("rejectAction")}
            </button>
          </div>
        </div>
      ))}
    </section>
  );
}

export function SessionSheet({ s, tz, pricing, targets = [], onChanged, onClose }: {
  s: SessionDay;
  tz: string;
  pricing: PricingInfo;
  /** Known targets, for project names. */
  targets?: TargetOption[];
  /** After a binding change: show the message and reload the day. */
  onChanged?: (message: string, error?: boolean) => void;
  onClose: () => void;
}) {
  const sheetRef = useSheet<HTMLElement>();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside ref={sheetRef} aria-modal="true" className="sheet" role="dialog" aria-label={t("sessionAria", { name: s.label ?? s.native_session_id })}>
        <header>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2>{s.label ?? t("noWorkingDir")}</h2>
            <div className="mono muted">{s.native_session_id}</div>
          </div>
          <button className="button ghost" onClick={onClose} style={{ minWidth: 0 }}>
            {t("close")}
          </button>
        </header>
        <div className="body">
          {onChanged && <SessionTargets s={s} targets={targets} onChanged={onChanged} />}
          <SessionDetail s={s} tz={tz} pricing={pricing} />
        </div>
      </aside>
    </>
  );
}

import { useEffect } from "react";
import { useSheet } from "./useSheet";
import { t } from "./i18n";
import { SessionDetail } from "./SessionDetail";
import type { PricingInfo, SessionDay } from "./types";

export function SessionSheet({ s, tz, pricing, onClose }: {
  s: SessionDay;
  tz: string;
  pricing: PricingInfo;
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
          <SessionDetail s={s} tz={tz} pricing={pricing} />
        </div>
      </aside>
    </>
  );
}

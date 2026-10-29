import type { ReactNode } from "react";
import { t } from "../i18n";
import type { Scope } from "../nav";

const SCOPES: Scope[] = ["day", "week", "month"];
const SCOPE_LABEL = { day: "scopeDay", week: "scopeWeek", month: "scopeMonth" } as const;

/** Top bar of an area: title, the period where it applies, commands on the right. */
export function Bar({
  title,
  sub,
  period,
  children,
}: {
  title: string;
  sub?: string;
  period?: { label: string; scope?: Scope; onScope?: (s: Scope) => void; canPrev: boolean; canNext: boolean; onShift: (dir: -1 | 1) => void };
  children?: ReactNode;
}) {
  return (
    <header className="topbar" data-tauri-drag-region>
      <h1>{title}</h1>
      {sub && <span className="sub">{sub}</span>}
      {period && (
        <>
          {period.scope && period.onScope && (
            <div className="seg" role="group" aria-label={t("scopeAria")}>
              {SCOPES.map((s) => (
                <button key={s} aria-pressed={period.scope === s} onClick={() => period.onScope!(s)}>
                  {t(SCOPE_LABEL[s])}
                </button>
              ))}
            </div>
          )}
          <div className="pager" role="group" aria-label={t("scopeAria")}>
            <button aria-label={t("prevPeriod")} disabled={!period.canPrev} onClick={() => period.onShift(-1)}>‹</button>
            <span className="lbl">{period.label}</span>
            <button aria-label={t("nextPeriod")} disabled={!period.canNext} onClick={() => period.onShift(1)}>›</button>
          </div>
        </>
      )}
      <span className="grow" />
      {children}
    </header>
  );
}

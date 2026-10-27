import type { ReactNode } from "react";
import { t } from "./i18n";
import type { Scope } from "./nav";

const SCOPES: Scope[] = ["day", "week", "month"];
const SCOPE_LABEL = { day: "scopeDay", week: "scopeWeek", month: "scopeMonth" } as const;

/** Title on the left, the period in the middle where it applies, commands on the right. */
export function Toolbar({
  title,
  period,
  children,
}: {
  title: string;
  period?: {
    label: string;
    scope?: Scope;
    onScope?: (s: Scope) => void;
    canPrev: boolean;
    canNext: boolean;
    onShift: (dir: -1 | 1) => void;
  };
  children?: ReactNode;
}) {
  return (
    <header className="day-header">
      <div className="day-title">
        <h1>{title}</h1>
      </div>
      {period && (
        <div className="period" role="group" aria-label={t("scopeAria")}>
          {period.scope && period.onScope && (
            <div className="segments compact" role="group" aria-label={t("scopeAria")}>
              {SCOPES.map((s) => (
                <button key={s} aria-pressed={period.scope === s} onClick={() => period.onScope!(s)}>
                  {t(SCOPE_LABEL[s])}
                </button>
              ))}
            </div>
          )}
          <button className="button ghost icon" aria-label={t("prevPeriod")} disabled={!period.canPrev} onClick={() => period.onShift(-1)}>
            ‹
          </button>
          <span className="period-label">{period.label}</span>
          <button className="button ghost icon" aria-label={t("nextPeriod")} disabled={!period.canNext} onClick={() => period.onShift(1)}>
            ›
          </button>
        </div>
      )}
      <div className="spacer" />
      <div className="header-actions">{children}</div>
    </header>
  );
}

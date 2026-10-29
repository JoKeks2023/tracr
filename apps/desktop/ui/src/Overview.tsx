import { useState } from "react";
import { duration, estimate, intervals, reasonText } from "./format";
import { t } from "./i18n";
import type { DayAgents, DayCost, HumanDay, ProjectTotal } from "./types";

/** The one number of the day, with the review action beside it. */
export function BigNumber({ human, canReview, onReview, state }: { human: { counted_ms: number }; canReview: boolean; onReview: () => void; state: string }) {
  return (
    <section className="big-number">
      <div>
        <div className="big-value">{duration(human.counted_ms)}</div>
        <div className="big-label">{t("countedTime")}</div>
      </div>
      <div className="review-state">
        <span className="muted">{state}</span>
        {canReview && (
          <button className="button secondary" onClick={onReview}>
            {t("reviewDay")}
          </button>
        )}
      </div>
    </section>
  );
}

/** The ledger's attribution states, in words. */
function targetLabel(label: string): string {
  const known: Record<string, "unattributed" | "conflicting" | "ambiguous"> = { unattributed: "unattributed", conflicting: "conflicting", ambiguous: "ambiguous" };
  const k = known[label];
  return k ? t(k === "unattributed" ? "tUnattributed" : k === "conflicting" ? "tConflicting" : "tAmbiguous") : label;
}

type Row = { key: string; label: string; ms: number; muted: boolean; parts: { label: string; ms: number }[] };

/** Counted time per project when projects exist, else per repository. */
export function Bars({ view, human }: { view: DayAgents; human: HumanDay | { counted_by_target: Record<string, number> } }) {
  const [open, setOpen] = useState<string | null>(null);
  const hasProjects = view.counted_by_project.some((p: ProjectTotal) => p.kind === "project");
  const rows: Row[] = hasProjects
    ? view.counted_by_project.map((p) => ({ key: p.project_id ?? p.label, label: p.label, ms: p.ms, muted: p.kind === "other", parts: p.parts }))
    : Object.entries(human.counted_by_target)
        .sort((a, b) => b[1] - a[1])
        .map(([label, ms]) => ({ key: label, label: targetLabel(label), ms, muted: ["unattributed", "conflicting", "ambiguous"].includes(label), parts: [] }));
  if (rows.length === 0) return null;
  const max = Math.max(...rows.map((r) => r.ms), 1);
  return (
    <section className="bars">
      <h2 className="section-title">{hasProjects ? t("byProject") : t("byRepository")}</h2>
      <ul>
        {rows.map((r) => (
          <li key={r.key} className={r.muted ? "muted" : undefined}>
            <button className="bar-row" disabled={r.parts.length === 0} aria-expanded={open === r.key} onClick={() => setOpen(open === r.key ? null : r.key)}>
              <span className="bar-label">{r.label}</span>
              <span className="bar-track" aria-hidden>
                <i style={{ width: `${Math.max(2, Math.round((r.ms / max) * 100))}%` }} />
              </span>
              <span className="mono bar-time">{duration(r.ms)}</span>
            </button>
            {open === r.key && (
              <ul className="bar-parts">
                {r.parts.map((p) => (
                  <li key={p.label}>
                    <span className="bar-label">{p.label}</span>
                    <span className="mono bar-time">{duration(p.ms)}</span>
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Unclear time is a task, not a statistic. */
export function Attention({ human, windows, onResolve }: { human: { unknown_ms: number; unknown_by_reason: Record<string, number> }; windows: number; onResolve: () => void }) {
  if (human.unknown_ms <= 0) return null;
  const why = Object.entries(human.unknown_by_reason)
    .map(([r, ms]) => `${reasonText(r)}: ${duration(ms)}`)
    .join(" · ");
  return (
    <section className="attention">
      <div>
        <div className="attention-title">{t("unclearTitle", { time: duration(human.unknown_ms) })}</div>
        <div className="muted">{intervals(windows)}</div>
        {why && (
          <details className="why">
            <summary>{t("why")}</summary>
            <p>{why}</p>
          </details>
        )}
      </div>
      <button className="button secondary" onClick={onResolve}>
        {t("resolve")}
      </button>
    </section>
  );
}

/** Estimated API cost: small, with the full explanation in the tooltip. */
export function CostLine({ cost }: { cost: DayCost }) {
  const c = cost.identified;
  const p = cost.pricing;
  const tokens = c.priced_tokens + c.unpriced_tokens;
  const share = tokens > 0 ? c.unpriced_tokens / tokens : 0;
  const lines = [
    t("tipLabel"),
    t("tipPrices", { catalog: p.catalog_version, asOf: p.oldest_fetched_at ?? "?", urls: p.source_urls.join(", ") }),
    ...c.reasons.map((r) => t("tipUnpriced", { r: reasonText(r) })),
    ...c.assumptions.map((a) => t("tipAssumed", { r: reasonText(a) })),
    cost.unidentified.priced_records + cost.unidentified.unpriced_records + cost.unidentified.partially_priced_records > 0
      ? t("tipUnidentified", { x: estimate(cost.unidentified, p.currency) })
      : null,
    cost.spanning_records > 0 ? t("tipSpanning", { n: cost.spanning_records, x: estimate(cost.spanning, p.currency) }) : null,
  ].filter(Boolean);
  return (
    <section className="cost-line" title={lines.join("\n")}>
      <span className="muted">{t("costLine")}</span>
      <span className="mono">{estimate(c, p.currency)}</span>
      <span className="muted">
        · {t("notSpend")}
        {share > 0 && ` · ${t("unpriced", { pct: share < 0.01 ? "<1%" : `${Math.round(share * 100)}%` })}`}
      </span>
    </section>
  );
}

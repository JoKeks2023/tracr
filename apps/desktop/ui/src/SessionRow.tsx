import { duration, estimate, providerName, spanLabel } from "./format";
import { t } from "./i18n";
import type { PricingInfo, SessionDay } from "./types";

/** What a session is called in lists: its folder, else a plain fallback. */
export function sessionTitle(s: SessionDay): string {
  return s.label ?? t("noWorkingDir");
}

export function agentMs(s: SessionDay): number {
  return s.actors.reduce((n, a) => n + a.execution_ms, 0);
}

/** One session as a row: title, then who/when/where, with time and estimated cost at the right. */
export function SessionRow({
  s,
  pricing,
  tz,
  selected,
  onOpen,
}: {
  s: SessionDay;
  pricing: PricingInfo;
  tz: string;
  selected: boolean;
  onOpen: () => void;
}) {
  const subs = s.actors.filter((a) => a.actor_kind === "known").length;
  const where = [s.repository_label, s.git_branch].filter(Boolean).join(" · ");
  // When the agent ran in the shown period (clipped), else the session's evidence span.
  const runs = s.actors.flatMap((a) => a.segments);
  const first = runs.length ? Math.min(...runs.map((g) => g.start)) : s.first_evidence_at;
  const last = runs.length ? Math.max(...runs.map((g) => g.end)) : s.last_evidence_at;
  const span = first && last ? spanLabel(first, last, tz) : null;
  return (
    <button className="session-row" aria-current={selected ? "true" : undefined} onClick={onOpen}>
      <span className="session-main">
        <span className="session-title">{sessionTitle(s)}</span>
        <span className="session-sub">
          <span>{providerName(s.provider)}</span>
          {span && <span className="mono">{span}</span>}
          {where && <span className="mono">{where}</span>}
          {subs > 0 && <span>{t("subagentCount", { n: subs })}</span>}
        </span>
      </span>
      <span className="session-figures">
        <span className="mono">{duration(agentMs(s))}</span>
        {s.cost.amount_nano === null ? (
          <span className="muted" title={t("unpricedWord")}>—</span>
        ) : (
          <span className="mono">{estimate(s.cost, pricing.currency)}</span>
        )}
      </span>
    </button>
  );
}

import { duration, estimate } from "./format";
import { t } from "./i18n";
import type { PricingInfo, SessionDay } from "./types";

/** What a session is called in lists: its folder, else a plain fallback. */
export function sessionTitle(s: SessionDay): string {
  return s.label ?? t("noWorkingDir");
}

export function agentMs(s: SessionDay): number {
  return s.actors.reduce((n, a) => n + a.execution_ms, 0);
}

/** One session as a row: title, where it ran, how long and what it cost (an estimate). */
export function SessionRow({
  s,
  pricing,
  selected,
  onOpen,
}: {
  s: SessionDay;
  pricing: PricingInfo;
  selected: boolean;
  onOpen: () => void;
}) {
  const subs = s.actors.filter((a) => a.actor_kind === "known").length;
  const where = [s.repository_label, s.git_branch].filter(Boolean).join(" · ");
  return (
    <button className="session-row" aria-current={selected ? "true" : undefined} onClick={onOpen}>
      <span className="session-main">
        <span className="session-title">{sessionTitle(s)}</span>
        <span className="session-sub">
          <span className="provider-tag">{s.provider}</span>
          {where && <span className="mono">{where}</span>}
          {subs > 0 && <span>{t("subagentCount", { n: subs })}</span>}
        </span>
      </span>
      <span className="session-figures">
        <span className="mono">{duration(agentMs(s))}</span>
        <span className={`mono ${s.cost.amount_nano === null ? "muted" : ""}`}>{estimate(s.cost, pricing.currency)}</span>
      </span>
    </button>
  );
}

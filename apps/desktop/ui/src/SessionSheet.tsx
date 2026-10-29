import { useEffect } from "react";
import { locale, t } from "./i18n";
import { duration, estimate, hhmm, money, reasonText, shortId, tokens } from "./format";
import type { PricingInfo, SessionDay, TokenCount, UsageLine } from "./types";

function Tok({ t }: { t: TokenCount }) {
  const { text, unknown } = tokens(t);
  return (
    <>
      {text}
      {unknown > 0 && (
        <span className="unknown-mark" title={`${unknown} records do not state this category (unknown, not zero)`}>
          +{unknown}?
        </span>
      )}
    </>
  );
}

function actorName(s: SessionDay, actorId: string | null): string {
  const a = s.actors.find((x) => x.actor_id === actorId);
  if (!a) return t("unattributedActor");
  return a.actor_kind === "known" && a.native_agent_id ? t("agentPrefix", { id: shortId(a.native_agent_id, 10) }) : t("mainChain");
}

export function SessionSheet({ s, tz, pricing, onClose }: {
  s: SessionDay;
  tz: string;
  pricing: PricingInfo;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const usage: UsageLine[] = [...s.usage].sort((a, b) => b.output.sum - a.output.sum);
  const diagnostics = Object.entries(s.diagnostics);

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet" role="dialog" aria-label={t("sessionAria", { name: s.label ?? s.native_session_id })}>
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
          <dl className="kv">
            <dt>{t("provider")}</dt>
            <dd>{s.provider}</dd>
            <dt>{t("workingDir")}</dt>
            <dd className="mono">{s.cwd ?? "—"}</dd>
            <dt>{t("branch")}</dt>
            <dd className="mono">{s.git_branch ?? "—"}</dd>
            <dt>{t("evidenceSpan")}</dt>
            <dd>
              {s.first_evidence_at && s.last_evidence_at
                ? `${new Date(s.first_evidence_at).toLocaleString(locale, { timeZone: tz })} – ${new Date(
                    s.last_evidence_at,
                  ).toLocaleString(locale, { timeZone: tz })}`
                : "—"}
            </dd>
            <dt>{t("humanPrompts")}</dt>
            <dd>
              {s.engagement_instants.length === 0
                ? t("noneThisDay")
                : s.engagement_instants.map((t) => hhmm(t, tz)).join("  ")}
            </dd>
          </dl>

          <section>
            <h3>{t("agentsThisDay")}</h3>
            <table className="data">
              <thead>
                <tr>
                  <th>{t("actor")}</th>
                  <th>{t("execution")}</th>
                  <th>{t("segments")}</th>
                  <th>{t("turns")}</th>
                  <th>{t("toolCalls")}</th>
                </tr>
              </thead>
              <tbody>
                {s.actors.map((a) => (
                  <tr key={a.actor_id}>
                    <td>{actorName(s, a.actor_id)}</td>
                    <td>{duration(a.execution_ms)}</td>
                    <td>{a.segments.length}</td>
                    <td>{a.turns}</td>
                    <td>{a.tool_calls}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>

          <section>
            <h3>{t("tokensThisDay")}</h3>
            <table className="data">
              <thead>
                <tr>
                  <th>{t("actorModel")}</th>
                  <th>{t("input")}</th>
                  <th>{t("cacheWrite")}</th>
                  <th>{t("cacheRead")}</th>
                  <th>{t("output")}</th>
                  <th title={pricing.label}>{t("estCost")}</th>
                </tr>
              </thead>
              <tbody>
                {usage.map((u, i) => (
                  <tr key={i}>
                    <td title={`${u.records} records${u.identified ? "" : ", unidentified"}`}>
                      {actorName(s, u.actor_id)} · <span className="mono">{u.model ?? t("modelUnknown")}</span>
                      {!u.identified && <span className="muted"> · {t("unidentified")}</span>}
                    </td>
                    <td>
                      <Tok t={u.input_fresh} />
                    </td>
                    <td>
                      <Tok t={u.cache_creation} />
                    </td>
                    <td>
                      <Tok t={u.cache_read} />
                    </td>
                    <td>
                      <Tok t={u.output} />
                    </td>
                    <td
                      className={u.cost.amount_nano === null ? "muted" : undefined}
                      title={[...u.cost.reasons, ...u.cost.assumptions].map(reasonText).join("\n") || undefined}
                    >
                      {estimate(u.cost, pricing.currency)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="note">
              {t("estCostNote", { catalog: pricing.catalog_version, asOf: pricing.oldest_fetched_at ?? "?" })}
              {s.cost.reasons.length > 0 && t("unpricedHere", { list: s.cost.reasons.map(reasonText).join("; ") })}
              {s.cost.assumptions.length > 0 && t("assumedHere", { list: s.cost.assumptions.map(reasonText).join("; ") })}
            </p>
          </section>

          {s.reported.length > 0 && (
            <section>
              <h3>{t("reportedByProvider")}</h3>
              <dl className="kv">
                {s.reported.map((r) => (
                  <div key={r.source_field} style={{ display: "contents" }}>
                    <dt className="mono">{r.value_nano !== null ? money(r.value_nano, r.currency) : `${r.value_as_stated} ${r.currency}`}</dt>
                    <dd>
                      {r.source_field} · {t("lastSeen")} {new Date(r.recorded_at).toLocaleString(locale, { timeZone: tz })}
                    </dd>
                  </div>
                ))}
              </dl>
              <p className="note">
                Claude Code's figure can carry over into a resumed session or restart, so it is not this session's cost. It
                is never added to the estimate.
              </p>
            </section>
          )}

          {diagnostics.length > 0 && (
            <section>
              <h3>{t("quality")}</h3>
              <dl className="kv">
                {diagnostics.map(([k, n]) => (
                  <div key={k} style={{ display: "contents" }}>
                    <dt className="mono">{n}</dt>
                    <dd>{k.replaceAll("_", " ")}</dd>
                  </div>
                ))}
              </dl>
            </section>
          )}
        </div>
      </aside>
    </>
  );
}

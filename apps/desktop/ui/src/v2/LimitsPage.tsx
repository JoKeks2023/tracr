import { useEffect, useState } from "react";
import { api, errorText } from "../api";
import { compact, modelName, providerName } from "../format";
import { t } from "../i18n";
import type { Analytics, Limits } from "../types";
import { useQuotaPrefs } from "./quota";
import { CostChart } from "./CostChart";
import { Panel, QuotaBar, QuotaRings, QuotaViewSwitch, clockLabel } from "./ui";

function money(nano: number, currency: string): string {
  const v = nano / 1e9;
  const sign = currency === "USD" ? "$" : "";
  return `${sign}${v.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}${currency === "USD" ? "" : ` ${currency}`}`;
}

/** Quota as the providers report it, then what each model cost in the period (estimates). */
export function LimitsPage({
  from,
  to,
  limits,
  reloadKey,
  onTap,
  onError,
}: {
  from: string;
  to: string;
  limits: Limits | null;
  reloadKey: unknown;
  onTap: (install: boolean) => Promise<void>;
  onError: (text: string) => void;
}) {
  const [an, setAn] = useState<Analytics | null>(null);
  const [busy, setBusy] = useState(false);
  const { prefs } = useQuotaPrefs();
  useEffect(() => {
    let live = true;
    api
      .analytics(from, to)
      .then((l) => live && setAn(l))
      .catch((e) => live && onError(errorText(e)));
    return () => {
      live = false;
    };
  }, [from, to, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const windows = [...(limits?.windows ?? [])].sort((a, b) => a.provider.localeCompare(b.provider) || (a.window_minutes ?? 0) - (b.window_minutes ?? 0));
  const tap = limits?.claude_tap;
  const toggle = async () => {
    setBusy(true);
    try {
      await onTap(!tap?.installed);
    } finally {
      setBusy(false);
    }
  };
  const currency = an?.pricing.currency ?? "USD";

  return (
    <div className="body">
      {windows.length > 0 ? (
        <>
          <div>
            <QuotaViewSwitch />
          </div>
          {prefs.rings ? (
            <Panel title={t("quotaTitle")} aside={t("quotaStated")}>
              <QuotaRings windows={windows} />
            </Panel>
          ) : (
            <div className="qgrid">
              {windows.map((w) => (
                <section key={`${w.provider}-${w.window}`} className="qcard">
                  <QuotaBar w={w} big />
                </section>
              ))}
            </div>
          )}
        </>
      ) : (
        <Panel title={t("quotaTitle")} aside={t("quotaStated")}>
          <p className="vnote">{t("quotaNone")}. {t("quotaNoneHint")}</p>
        </Panel>
      )}

      {an && (
        <>
          <Panel title={t("costPerDay")} aside={`${an.from} – ${an.to}`}>
            <CostChart days={an.days} currency={currency} />
          </Panel>

          <div className="row2">
            <Panel title={t("costByModel")}>
              <table className="t">
                <thead>
                  <tr>
                    <th>{t("costModel")}</th>
                    <th className="r">{t("costRequests")}</th>
                    <th className="r">{t("costTokens")}</th>
                    <th className="r">{t("metaCost")}</th>
                  </tr>
                </thead>
                <tbody>
                  {an.models.map((r) => (
                    <tr key={r.model}>
                      <td>{modelName(r.model)}</td>
                      <td className="r mono">{r.records.toLocaleString("en-US")}</td>
                      <td className="r mono">{compact(r.tokens)}</td>
                      <td className="r mono">{r.amount_nano !== null ? `${r.partial ? "≥ " : ""}${money(r.amount_nano, currency)}` : t("unpricedWord")}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {an.models.length === 0 && <p className="vnote">{t("sessionsEmpty")}</p>}
              {an.what_if && (
                <div className="hint">
                  {t("whatIf", {
                    model: modelName(an.what_if.to_model),
                    saving: `${an.what_if.partial || an.what_if.saving_nano < 0 ? "≥ " : ""}${money(Math.abs(an.what_if.saving_nano), currency)}`,
                    dir: an.what_if.saving_nano >= 0 ? t("whatIfLess") : t("whatIfMore"),
                  })}
                </div>
              )}
            </Panel>

            <Panel title={t("cacheTitle")}>
              <div className="srow">
                <div className="lbl">{t("cacheHit")}</div>
                <div className="ctl mono">{an.cache.hit_ratio === null ? "—" : `${Math.round(an.cache.hit_ratio * 100)} %`}</div>
              </div>
              <div className="srow">
                <div className="lbl">{t("cacheWrites")}</div>
                <div className="ctl mono">{compact(an.cache.write_5m_tokens)} / {compact(an.cache.write_1h_tokens)}</div>
              </div>
              <div className="srow">
                <div className="lbl">
                  <div>{t("cacheCold")}</div>
                  {an.cache.cold_requests > 0 && <div className="vnote">{t("cacheColdNote", { n: an.cache.cold_requests, tokens: compact(an.cache.cold_tokens), extra: money(an.cache.cold_extra_nano, currency) })}</div>}
                </div>
                <div className="ctl mono">{an.cache.cold_requests}</div>
              </div>
              <div className="srow">
                <div className="lbl">{t("cacheSaved")}</div>
                <div className="ctl mono">≈ {money(an.cache.saved_nano, currency)}</div>
              </div>
              {an.cache.worst_sessions.length > 0 && (
                <div className="vnote" style={{ marginTop: 8 }}>
                  {t("cacheWorst")}: {an.cache.worst_sessions.slice(0, 3).map((s) => `${s.label ?? "?"} ${Math.round(s.hit_ratio * 100)} %`).join(" · ")}
                </div>
              )}
              <p className="vnote" style={{ marginTop: 8 }}>{t("costNote")}</p>
            </Panel>
          </div>
        </>
      )}
      {(limits?.credits ?? []).map((c) => (
        <Panel key={c.provider} title={`${providerName(c.provider)} · ${t("credits")}`} aside={t("statedAt", { time: clockLabel(c.observed_at) })}>
          <p className="mono">{c.unlimited ? t("creditsUnlimited") : c.has_credits ? `${c.balance ?? "?"}` : t("creditsNone")}</p>
        </Panel>
      ))}

      <Panel title="Claude Code" aside={tap?.installed ? (tap.chained ? "✓ + status line" : "✓") : undefined}>
        <div className="need" style={{ borderTop: 0, paddingTop: 0 }}>
          <div className="t">
            <div className="vnote">{t("quotaClaudeHint")}</div>
          </div>
          <button className="btn s" disabled={busy || !tap} onClick={toggle}>
            {busy ? t("tapWorking") : tap?.installed ? t("quotaClaudeOff") : t("quotaClaudeOn")}
          </button>
        </div>
      </Panel>

    </div>
  );
}

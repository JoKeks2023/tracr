import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../api";
import { compact, modelName } from "../format";
import { t } from "../i18n";
import type { Limits, SessionList } from "../types";
import { useQuotaPrefs } from "./quota";
import { Panel, QuotaBar, QuotaRings, QuotaViewSwitch } from "./ui";

interface ModelRow {
  model: string;
  records: number;
  tokens: number;
  nano: number;
  priced: boolean;
  partial: boolean;
}

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
  const [list, setList] = useState<SessionList | null>(null);
  const [busy, setBusy] = useState(false);
  const { prefs } = useQuotaPrefs();
  useEffect(() => {
    let live = true;
    api
      .sessions(from, to)
      .then((l) => live && setList(l))
      .catch((e) => live && onError(errorText(e)));
    return () => {
      live = false;
    };
  }, [from, to, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const rows = useMemo(() => {
    const by = new Map<string, ModelRow>();
    for (const s of list?.sessions ?? []) {
      for (const u of s.usage) {
        if (!u.identified) continue;
        const key = u.model ?? "?";
        const r = by.get(key) ?? { model: key, records: 0, tokens: 0, nano: 0, priced: false, partial: false };
        r.records += u.records;
        r.tokens += u.input_fresh.sum + u.cache_creation.sum + u.cache_read.sum + u.output.sum;
        if (u.cost.amount_nano !== null) {
          r.nano += u.cost.amount_nano;
          r.priced = true;
        }
        if (u.cost.unpriced_records + u.cost.partially_priced_records + u.cost.not_estimated_records > 0) r.partial = true;
        by.set(key, r);
      }
    }
    return [...by.values()].sort((a, b) => b.nano - a.nano || b.tokens - a.tokens);
  }, [list]);

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
  const currency = list?.pricing.currency ?? "USD";

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

      <Panel title={t("costByModel")} aside={list ? `${list.from} – ${list.to}` : undefined}>
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
            {rows.map((r) => (
              <tr key={r.model}>
                <td>{modelName(r.model)}</td>
                <td className="r mono">{r.records.toLocaleString("en-US")}</td>
                <td className="r mono">{compact(r.tokens)}</td>
                <td className="r mono">{r.priced ? `${r.partial ? "≥ " : ""}${money(r.nano, currency)}` : t("unpricedWord")}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length === 0 && <p className="vnote">{t("sessionsEmpty")}</p>}
        <p className="vnote" style={{ marginTop: 8 }}>{t("costNote")}</p>
      </Panel>
    </div>
  );
}

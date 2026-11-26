import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../api";
import { compact, duration, estimate, modelName, money, providerName, sessionTokens, spanLabel } from "../format";
import { t } from "../i18n";
import { SessionSheet, targetName } from "../SessionSheet";
import type { SessionQuery, SessionSearch, TargetOption } from "../types";

/** Minimum estimated cost, in whole currency units (presets, never free text). */
const MIN_COST = [0.1, 1, 5, 10, 50];

type Filters = Omit<SessionQuery, "from" | "to">;

/** One dropdown over a facet; "" means any. A selected value stays listed even if the facet no longer has it. */
function Pick({ label, value, options, onPick }: { label: string; value: string | null | undefined; options: [string, string][]; onPick: (v: string | null) => void }) {
  const opts = value && !options.some(([v]) => v === value) ? [...options, [value, value] as [string, string]] : options;
  return (
    <label className="sfilter">
      <span className="vnote">{label}</span>
      <select className="ssel" value={value ?? ""} onChange={(e) => onPick(e.target.value === "" ? null : e.target.value)}>
        <option value="">{t("filterAny")}</option>
        {opts.map(([v, l]) => (
          <option key={v} value={v}>{l}</option>
        ))}
      </select>
    </label>
  );
}

/**
 * Session search (TRACR-21): metadata filters over the period, never content.
 * Every filter but the text is a dropdown of the values the period actually has.
 */
export function SessionsPage({
  from,
  to,
  home,
  reloadKey,
  onError,
  onChanged,
}: {
  from: string;
  to: string;
  home: string | null;
  reloadKey: unknown;
  onError: (text: string) => void;
  /** After a binding change in the sheet. */
  onChanged: (text: string, error?: boolean) => void;
}) {
  const [filters, setFilters] = useState<Filters>({});
  const [text, setText] = useState("");
  const [result, setResult] = useState<SessionSearch | null>(null);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState<string | null>(null);
  const [targets, setTargets] = useState<TargetOption[]>([]);
  const [bump, setBump] = useState(0);
  const seq = useRef(0);
  const errorRef = useRef(onError);
  errorRef.current = onError;

  useEffect(() => {
    void api.targets().then(setTargets).catch(() => setTargets([]));
  }, []);

  // Typing waits a moment; dropdowns and the period search at once.
  const [typed, setTyped] = useState("");
  useEffect(() => {
    const id = setTimeout(() => setTyped(text.trim()), 250);
    return () => clearTimeout(id);
  }, [text]);

  useEffect(() => {
    const mine = ++seq.current;
    setBusy(true);
    api
      .searchSessions({ from, to, ...filters, text: typed || null })
      .then((r) => mine === seq.current && setResult(r))
      .catch((e) => mine === seq.current && errorRef.current(errorText(e)))
      .finally(() => mine === seq.current && setBusy(false));
  }, [from, to, filters, typed, reloadKey, bump]);

  const set = (patch: Filters) => setFilters((f) => ({ ...f, ...patch }));
  const active = Object.values(filters).some((v) => v !== null && v !== undefined) || typed !== "";
  const list = result?.list ?? null;
  const facets = result?.facets;
  const currency = list?.pricing.currency ?? "USD";
  const session = list?.sessions.find((s) => s.session_id === open) ?? null;

  return (
    <div className="body">
      <div className="sfilters" role="search" aria-label={t("searchSessions")}>
        <label className="sfilter grow">
          <span className="vnote">{t("searchText")}</span>
          <input type="search" className="sinput" placeholder={t("searchTextHint")} aria-label={t("searchText")} value={text} onChange={(e) => setText(e.target.value)} />
        </label>
        <Pick label={t("provider")} value={filters.provider} options={(facets?.providers ?? []).map((p) => [p, providerName(p)])} onPick={(v) => set({ provider: v })} />
        <Pick label={t("projects")} value={filters.project} options={(facets?.projects ?? []).map((p) => [p.id, p.name])} onPick={(v) => set({ project: v })} />
        <Pick label={t("filterBranch")} value={filters.branch} options={(facets?.branches ?? []).map((b) => [b, b])} onPick={(v) => set({ branch: v })} />
        <Pick label={t("filterModel")} value={filters.model} options={(facets?.models ?? []).map((m) => [m, modelName(m)])} onPick={(v) => set({ model: v })} />
        <Pick label={t("filterTool")} value={filters.tool} options={(facets?.tools ?? []).map((x) => [x, x])} onPick={(v) => set({ tool: v })} />
        <Pick
          label={t("filterBoundTo")}
          value={filters.bound_to}
          options={(facets?.bound_to ?? []).map((b) => [b, targetName(b, targets)])}
          onPick={(v) => set({ bound_to: v })}
        />
        <Pick
          label={t("filterSubagents")}
          value={filters.subagents === true ? "yes" : filters.subagents === false ? "no" : null}
          options={[["yes", t("filterWithSubagents")], ["no", t("filterWithoutSubagents")]]}
          onPick={(v) => set({ subagents: v === null ? null : v === "yes" })}
        />
        <Pick
          label={t("filterMinCost")}
          value={filters.min_cost_nano ? String(filters.min_cost_nano) : null}
          options={MIN_COST.map((c) => [String(Math.round(c * 1e9)), `≥ ${money(Math.round(c * 1e9), currency)}`])}
          onPick={(v) => set({ min_cost_nano: v === null ? null : Number(v) })}
        />
        {active && (
          <button className="btn s" onClick={() => (setFilters({}), setText(""))}>{t("filterReset")}</button>
        )}
      </div>

      {result && list && (
        <p className="vnote" aria-live="polite">
          {t("searchCount", { n: list.sessions.length, total: result.total })}
          {filters.min_cost_nano ? ` ${t("searchUnpricedNote")}` : ""}
        </p>
      )}

      <section className="panel" aria-label={t("tabSessions")} aria-busy={busy}>
        {list && list.sessions.length === 0 ? (
          <div className="vempty" style={{ margin: "2rem 1rem" }}>
            <h2>{active ? t("searchNone") : t("sessionsEmpty")}</h2>
            <p>{active ? t("searchNoneHint") : t("sessionsEmptyHint")}</p>
          </div>
        ) : (
          <table className="t">
            <thead>
              <tr>
                <th>{t("tabSessions")}</th>
                <th>{t("provider")}</th>
                <th className="r">{t("execution")}</th>
                <th className="r">{t("metaTokens")}</th>
                <th className="r">{t("metaCost")}</th>
              </tr>
            </thead>
            <tbody>
              {(list?.sessions ?? []).map((s) => {
                const run = s.actors.flatMap((a) => a.segments);
                const first = run.length ? Math.min(...run.map((g) => g.start)) : s.first_evidence_at;
                const last = run.length ? Math.max(...run.map((g) => g.end)) : s.last_evidence_at;
                return (
                  <tr key={s.session_id} className="trow" tabIndex={0} onClick={() => setOpen(s.session_id)} onKeyDown={(e) => e.key === "Enter" && setOpen(s.session_id)}>
                    <td>
                      {s.label ?? t("noWorkingDir")} <span className="vnote mono">{s.git_branch ?? ""}</span>{" "}
                      <span className="vnote mono">{first && last ? spanLabel(first, last, list!.timezone) : ""}</span>
                      {s.bound_to.length > 0 && <span className="chip">{s.bound_to.map((b) => targetName(b, targets)).join(", ")}</span>}
                    </td>
                    <td>
                      {providerName(s.provider)}{" "}
                      <span className="vnote">{[...new Set(s.usage.filter((u) => u.identified).map((u) => modelName(u.model)))].slice(0, 1).join("")}</span>
                    </td>
                    <td className="r mono">{duration(s.actors.reduce((n, a) => n + a.execution_ms, 0))}</td>
                    <td className="r mono">{compact(sessionTokens(s.usage))}</td>
                    <td className="r mono">{s.cost.amount_nano === null ? "—" : estimate(s.cost, currency)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </section>

      {session && list && (
        <SessionSheet
          s={session}
          tz={list.timezone}
          pricing={list.pricing}
          home={home}
          targets={targets}
          onChanged={(msg, err) => (onChanged(msg, err), setBump((b) => b + 1))}
          onClose={() => setOpen(null)}
        />
      )}
    </div>
  );
}

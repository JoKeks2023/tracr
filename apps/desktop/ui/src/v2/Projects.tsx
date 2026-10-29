import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../api";
import { ago, branchesN, sessionsN, compact, duration, estimate, modelName, providerName, sessionTokens, spanLabel } from "../format";
import { t } from "../i18n";
import { periodLabel, type Scope } from "../nav";
import { SessionDetail } from "../SessionDetail";
import { ContextView, ToolsView, TraceView, useTrace } from "../SessionInsights";
import type { ProjectEntry, ProjectOverview, SessionDay, SessionList } from "../types";
import { Stat } from "./ui";

const SCOPES: Scope[] = ["day", "week", "month"];
const SCOPE_LABEL = { day: "scopeDay", week: "scopeWeek", month: "scopeMonth" } as const;

type Tab = "overview" | "trace" | "context" | "tools";

function entryTitle(e: ProjectEntry): string {
  return e.kind === "none" ? t("projectOther") : e.label;
}

/** A session of the project, laid out in full: overview, trace, context, tools. */
function SessionPage({ s, list, onBack, backLabel }: { s: SessionDay; list: SessionList; onBack: () => void; backLabel: string }) {
  const [tab, setTab] = useState<Tab>("overview");
  useEffect(() => setTab("overview"), [s.session_id]);
  const { trace, error } = useTrace(s.session_id, tab !== "overview");
  return (
    <div className="body">
      <div>
        <button className="btn s" onClick={onBack}>‹ {backLabel}</button>
      </div>
      <header>
        <h2 className="pj-title">{s.label ?? t("noWorkingDir")}</h2>
        <div className="vnote mono">{s.native_session_id}</div>
      </header>
      <div className="seg" role="group" aria-label={t("tabsAria")} style={{ alignSelf: "flex-start" }}>
        {(["overview", "trace", "context", "tools"] as const).map((k) => (
          <button key={k} aria-pressed={tab === k} onClick={() => setTab(k)}>
            {t(({ overview: "tabOverview", trace: "tabTrace", context: "tabContext", tools: "tabTools" } as const)[k])}
          </button>
        ))}
      </div>
      <div className="panel">
        {tab === "overview" && <SessionDetail s={s} tz={list.timezone} pricing={list.pricing} range />}
        {tab !== "overview" && error && <p className="vnote">{error}</p>}
        {tab !== "overview" && !error && !trace && <p className="vnote">…</p>}
        {tab === "trace" && trace && <TraceView trace={trace} />}
        {tab === "context" && trace && <ContextView trace={trace} />}
        {tab === "tools" && trace && <ToolsView trace={trace} />}
      </div>
    </div>
  );
}

/** Projects first: the list on the left, the project's sessions on the right. */
export function Projects({
  from,
  to,
  scope,
  anchor,
  canPrev,
  canNext,
  reloadKey,
  focusSession,
  onScope,
  onShift,
  onError,
}: {
  from: string;
  to: string;
  scope: Scope;
  anchor: string;
  canPrev: boolean;
  canNext: boolean;
  reloadKey: unknown;
  focusSession: string | null;
  onScope: (s: Scope) => void;
  onShift: (dir: -1 | 1) => void;
  onError: (text: string) => void;
}) {
  const [overview, setOverview] = useState<ProjectOverview | null>(null);
  const [list, setList] = useState<SessionList | null>(null);
  const [sel, setSel] = useState<string | null>(null);
  const [session, setSession] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [now] = useState(Date.now());

  useEffect(() => {
    let live = true;
    Promise.all([api.projectOverview(from, to), api.sessions(from, to)])
      .then(([o, l]) => live && (setOverview(o), setList(l)))
      .catch((e) => live && onError(errorText(e)));
    return () => {
      live = false;
    };
  }, [from, to, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  // Arriving from a session card: open the project that holds it.
  useEffect(() => {
    if (!focusSession || !overview) return;
    const e = overview.entries.find((x) => x.session_ids.includes(focusSession));
    if (e) {
      setSel(e.key);
      setSession(focusSession);
    }
  }, [focusSession, overview]);

  const entries = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (overview?.entries ?? []).filter((e) => !q || [entryTitle(e), ...e.repositories.map((r) => r.label), ...e.branches].some((x) => x.toLowerCase().includes(q)));
  }, [overview, query]);
  const current = overview?.entries.find((e) => e.key === sel) ?? entries[0] ?? null;
  const maxAgent = Math.max(1, ...entries.map((e) => e.agent_ms));
  const sessions = useMemo(() => {
    if (!current || !list) return [];
    const ids = new Set(current.session_ids);
    return list.sessions.filter((s) => ids.has(s.session_id));
  }, [current, list]);
  const open = sessions.find((s) => s.session_id === session) ?? null;

  const picker = (
    <div className="pj-list-head">
      <h1>{t("projectsTitle")}</h1>
      <input type="search" className="pj-search" placeholder={t("projectsSearch")} aria-label={t("projectsSearch")} value={query} onChange={(e) => setQuery(e.target.value)} />
      <div className="seg" role="group" aria-label={t("scopeAria")}>
        {SCOPES.map((s) => (
          <button key={s} aria-pressed={scope === s} onClick={() => onScope(s)}>{t(SCOPE_LABEL[s])}</button>
        ))}
      </div>
      <div className="pager" style={{ justifyContent: "space-between" }}>
        <button aria-label={t("prevPeriod")} disabled={!canPrev} onClick={() => onShift(-1)}>‹</button>
        <span className="lbl">{periodLabel(scope, anchor)}</span>
        <button aria-label={t("nextPeriod")} disabled={!canNext} onClick={() => onShift(1)}>›</button>
      </div>
    </div>
  );

  return (
    <div className="pj">
      <aside className="pj-list" aria-label={t("projectsTitle")}>
        {picker}
        <div className="pj-items">
          {entries.map((e) => (
            <button key={e.key} className="pj-item" aria-current={current?.key === e.key ? "true" : undefined} onClick={() => (setSel(e.key), setSession(null))}>
              <span className="t">
                <b>{entryTitle(e)}</b>
                <span className="mono">{e.cost.amount_nano === null ? "—" : estimate(e.cost, overview!.pricing.currency)}</span>
              </span>
              <span className="m">
                <span className="mono">{duration(e.agent_ms)}</span>
                <span>{sessionsN(e.sessions)}</span>
                <span>{e.last_at ? ago(e.last_at, now) : ""}</span>
              </span>
              <span className="meter" aria-hidden><i style={{ width: `${Math.max(2, (e.agent_ms / maxAgent) * 100)}%` }} /></span>
            </button>
          ))}
          {overview && entries.length === 0 && (
            <div className="vempty" style={{ margin: "2rem 1rem" }}>
              <h2>{t("projectsNone")}</h2>
              <p>{t("projectsNoneHint")}</p>
            </div>
          )}
        </div>
      </aside>
      <div className="pj-detail">
        {!current || !list || !overview ? (
          <div className="body" aria-busy={!overview} />
        ) : open ? (
          <SessionPage s={open} list={list} onBack={() => setSession(null)} backLabel={entryTitle(current)} />
        ) : (
          <>
            <header className="pj-head">
              <h2 className="pj-title">{entryTitle(current)}</h2>
              <div className="chips">
                {current.repositories.map((r) => (
                  <span key={r.key} className="chip mono">{r.key}</span>
                ))}
                {current.branches.length > 0 && <span className="chip">{branchesN(current.branches.length)}</span>}
                {current.providers.map((p) => (
                  <span key={p} className="chip">{providerName(p)}</span>
                ))}
              </div>
            </header>
            <div className="body">
              <div className="stats">
                <Stat label={t("statAgents")} value={duration(current.agent_ms)} />
                <Stat label={t("statSessions")} value={String(current.sessions)} />
                <Stat label={t("statCost")} value={current.cost.amount_nano === null ? "—" : estimate(current.cost, overview.pricing.currency)} tone={current.cost.amount_nano === null ? "muted" : undefined} note={current.cost.amount_nano === null ? t("unpricedWord") : undefined} />
                <Stat label={t("lastActive")} value={current.last_at ? ago(current.last_at, now) : "—"} />
              </div>
              <section className="panel" aria-label={t("tabSessions")}>
                <div className="ph"><h2>{t("tabSessions")}</h2></div>
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
                    {sessions.map((s) => {
                      const run = s.actors.flatMap((a) => a.segments);
                      const first = run.length ? Math.min(...run.map((g) => g.start)) : s.first_evidence_at;
                      const last = run.length ? Math.max(...run.map((g) => g.end)) : s.last_evidence_at;
                      return (
                        <tr key={s.session_id} className="trow" tabIndex={0} onClick={() => setSession(s.session_id)} onKeyDown={(e) => e.key === "Enter" && setSession(s.session_id)}>
                          <td>
                            {s.label ?? t("noWorkingDir")} <span className="vnote mono">{s.git_branch ?? ""}</span>{" "}
                            <span className="vnote mono">{first && last ? spanLabel(first, last, list.timezone) : ""}</span>
                          </td>
                          <td>{providerName(s.provider)} <span className="vnote">{[...new Set(s.usage.filter((u) => u.identified).map((u) => modelName(u.model)))].slice(0, 1).join("")}</span></td>
                          <td className="r mono">{duration(s.actors.reduce((n, a) => n + a.execution_ms, 0))}</td>
                          <td className="r mono">{compact(sessionTokens(s.usage))}</td>
                          <td className="r mono">{s.cost.amount_nano === null ? "—" : estimate(s.cost, list.pricing.currency)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </section>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

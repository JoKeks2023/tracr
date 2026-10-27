import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { dayLabel, estimate } from "./format";
import { t } from "./i18n";
import { SessionDetail } from "./SessionDetail";
import { ContextView, ToolsView, TraceView, useTrace } from "./SessionInsights";
import { SessionRow, sessionTitle } from "./SessionRow";
import type { SessionDay, SessionList } from "./types";

const dayIn = (ms: number, tz: string) => new Intl.DateTimeFormat("en-CA", { timeZone: tz }).format(ms);

/** All sessions of the period, newest first, with the chosen one beside it. */
export function Sessions({
  from,
  to,
  reloadKey,
  selected,
  onSelect,
  onError,
}: {
  from: string;
  to: string;
  reloadKey: unknown;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onError: (text: string) => void;
}) {
  const [list, setList] = useState<SessionList | null>(null);
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState("");
  const [tab, setTab] = useState<"overview" | "trace" | "context" | "tools">("overview");

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

  const providers = useMemo(() => [...new Set(list?.sessions.map((s) => s.provider) ?? [])].sort(), [list]);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (list?.sessions ?? []).filter((s) => {
      if (provider && s.provider !== provider) return false;
      if (!q) return true;
      return [sessionTitle(s), s.repository_label, s.git_branch, s.native_session_id, s.cwd].some((x) => x?.toLowerCase().includes(q));
    });
  }, [list, query, provider]);

  const groups = useMemo(() => {
    const by = new Map<string, SessionDay[]>();
    for (const s of shown) {
      const d = s.last_evidence_at ? dayIn(s.last_evidence_at, list!.timezone) : from;
      by.set(d, [...(by.get(d) ?? []), s]);
    }
    return [...by.entries()].sort((a, b) => (a[0] < b[0] ? 1 : -1));
  }, [shown, list, from]);

  const current = list?.sessions.find((s) => s.session_id === selected) ?? null;
  const { trace, error: traceError } = useTrace(current?.session_id ?? "", !!current && tab !== "overview");
  useEffect(() => setTab("overview"), [selected]);

  // ↑/↓ move between rows.
  const onKeys = (e: React.KeyboardEvent<HTMLElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const rows = [...e.currentTarget.querySelectorAll<HTMLElement>(".session-row")];
    const at = rows.indexOf(document.activeElement as HTMLElement);
    const next = rows[Math.min(rows.length - 1, Math.max(0, at + (e.key === "ArrowDown" ? 1 : -1)))];
    if (next) {
      e.preventDefault();
      next.focus();
    }
  };

  if (!list) return <div className="sessions-empty muted">…</div>;

  return (
    <div className="sessions" data-detail={current ? "open" : "closed"}>
      <section className="session-list" aria-label={t("areaSessions")} onKeyDown={onKeys}>
        <div className="list-tools">
          <input type="search" className="input" placeholder={t("searchSessions")} aria-label={t("searchSessions")} value={query} onChange={(e) => setQuery(e.target.value)} />
          {providers.length > 1 && (
            <select className="input" aria-label={t("allProviders")} value={provider} onChange={(e) => setProvider(e.target.value)}>
              <option value="">{t("allProviders")}</option>
              {providers.map((p) => (
                <option key={p} value={p}>
                  {p}
                </option>
              ))}
            </select>
          )}
        </div>
        <p className="list-summary muted">
          {t("sessionCount", { n: shown.length })} · {estimate(list.cost, list.pricing.currency)}
        </p>
        {shown.length === 0 ? (
          <div className="list-empty">
            <strong>{t("sessionsEmpty")}</strong>
            <span className="muted">{t("sessionsEmptyHint")}</span>
          </div>
        ) : (
          groups.map(([d, rows]) => (
            <div key={d} className="session-group">
              <h2 className="group-title">{dayLabel(d)}</h2>
              {rows.map((s) => (
                <SessionRow key={s.session_id} s={s} pricing={list.pricing} selected={s.session_id === selected} onOpen={() => onSelect(s.session_id)} />
              ))}
            </div>
          ))
        )}
      </section>
      <section className="session-detail" aria-label={current ? sessionTitle(current) : t("selectSession")}>
        {current ? (
          <>
            <header className="detail-head">
              <button className="button ghost back" onClick={() => onSelect(null)}>
                ‹ {t("backToList")}
              </button>
              <h2>{sessionTitle(current)}</h2>
              <div className="mono muted">{current.native_session_id}</div>
            </header>
            <div className="segments detail-tabs" role="group" aria-label={t("tabsAria")}>
              {(["overview", "trace", "context", "tools"] as const).map((k) => (
                <button key={k} aria-pressed={tab === k} onClick={() => setTab(k)}>
                  {t(({ overview: "tabOverview", trace: "tabTrace", context: "tabContext", tools: "tabTools" } as const)[k])}
                </button>
              ))}
            </div>
            <div className="detail-body">
              {tab === "overview" && <SessionDetail s={current} tz={list.timezone} pricing={list.pricing} range />}
              {tab !== "overview" && traceError && <p className="muted">{traceError}</p>}
              {tab !== "overview" && !traceError && !trace && <p className="muted">…</p>}
              {tab === "trace" && trace && <TraceView trace={trace} />}
              {tab === "context" && trace && <ContextView trace={trace} />}
              {tab === "tools" && trace && <ToolsView trace={trace} />}
            </div>
          </>
        ) : (
          <p className="detail-empty muted">{t("selectSession")}</p>
        )}
      </section>
    </div>
  );
}

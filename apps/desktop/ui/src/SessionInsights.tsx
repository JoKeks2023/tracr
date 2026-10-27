import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import { compact, duration, hhmm, shortId } from "./format";
import { t } from "./i18n";
import type { SessionTrace, TraceActor } from "./types";

/** Loads the trace once per session; the three views share it. */
export function useTrace(sessionId: string, enabled: boolean) {
  const [trace, setTrace] = useState<SessionTrace | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setTrace(null);
    setError(null);
    if (!enabled) return;
    let live = true;
    api
      .sessionTrace(sessionId)
      .then((v) => live && setTrace(v))
      .catch((e) => live && setError(errorText(e)));
    return () => {
      live = false;
    };
  }, [sessionId, enabled]);
  return { trace, error };
}

type Category = "read" | "write" | "exec" | "agent" | "mcp" | "other";
const CAT_LABEL = { read: "catRead", write: "catWrite", exec: "catExec", agent: "catAgent", mcp: "catMcp", other: "catOther" } as const;
const CATEGORIES: Category[] = ["read", "write", "exec", "agent", "mcp", "other"];

export function toolCategory(name: string, kind: string): Category {
  if (kind === "extension") return "mcp";
  if (/^(Read|Grep|Glob|LS|WebFetch|WebSearch|NotebookRead)$/.test(name)) return "read";
  if (/^(Edit|Write|MultiEdit|NotebookEdit)$/.test(name)) return "write";
  if (/^(Bash|BashOutput|KillShell|KillBash)$/.test(name)) return "exec";
  if (/^(Task|Agent)$/.test(name)) return "agent";
  return "other";
}

const BREAK_MS = 5 * 60_000;
const BREAK_PX = 28;

/** Time to pixels with every pause over five minutes shortened to a fixed gap. */
function buildAxis(times: number[], width: number) {
  const ts = [...new Set(times)].sort((a, b) => a - b);
  const breaks: { at: number; ms: number }[] = [];
  let active = 0;
  for (let i = 1; i < ts.length; i++) {
    const gap = ts[i] - ts[i - 1];
    if (gap > BREAK_MS) breaks.push({ at: ts[i - 1], ms: gap });
    else active += gap;
  }
  const perMs = active > 0 ? Math.max(0, width - breaks.length * BREAK_PX) / active : 0;
  const x = (t0: number): number => {
    let px = 0;
    let prev = ts[0];
    for (const b of breaks) {
      if (t0 <= b.at) break;
      px += (b.at - prev) * perMs + BREAK_PX;
      prev = b.at + b.ms;
    }
    return px + (t0 - prev) * perMs;
  };
  return { x, breaks: breaks.map((b) => ({ ...b, px: x(b.at) })) };
}

function actorLabel(a: TraceActor): string {
  return a.actor_kind === "known" && a.native_agent_id ? t("agentPrefix", { id: shortId(a.native_agent_id, 8) }) : t("mainChain");
}

const LABEL_W = 96;
const LANE_H = 30;

export function TraceView({ trace }: { trace: SessionTrace }) {
  const width = 720;
  const lanes = useMemo(
    () => [...trace.actors].sort((a, b) => Number(a.actor_kind === "known") - Number(b.actor_kind === "known")),
    [trace],
  );
  const axis = useMemo(() => {
    const times = [trace.start, trace.end, ...trace.prompts, ...trace.tools.map((x) => x.at), ...trace.actors.flatMap((a) => a.segments.flat())];
    return buildAxis(times, width);
  }, [trace]);
  if (trace.tools.length === 0 && trace.prompts.length === 0 && trace.actors.every((a) => a.segments.length === 0)) {
    return <p className="muted">{t("traceNone")}</p>;
  }
  const tz = trace.timezone;
  const laneOf = (id: string | null) => Math.max(0, lanes.findIndex((a) => a.actor_id === id));
  const top = LANE_H; // the first lane holds your prompts
  const height = top + lanes.length * LANE_H + 22;
  const ticks = [trace.start, trace.end];
  return (
    <figure className="trace" aria-label={t("traceAria", { tools: trace.tools.length, prompts: trace.prompts.length, actors: lanes.length })}>
      <svg viewBox={`0 0 ${LABEL_W + width} ${height}`} width="100%" role="img" aria-hidden="true">
        <text x={0} y={LANE_H / 2 + 4} className="trace-label">{t("traceYou")}</text>
        {lanes.map((a, i) => (
          <text key={a.actor_id} x={0} y={top + i * LANE_H + LANE_H / 2 + 4} className="trace-label">{actorLabel(a)}</text>
        ))}
        {[0, ...lanes.map((_, i) => i + 1)].map((i) => (
          <line key={i} x1={LABEL_W} x2={LABEL_W + width} y1={i * LANE_H + LANE_H} y2={i * LANE_H + LANE_H} className="trace-rule" />
        ))}
        {axis.breaks.map((b) => (
          <g key={b.at}>
            <rect x={LABEL_W + b.px} y={4} width={BREAK_PX} height={height - 26} className="trace-break" />
            <title>{t("traceBreak", { time: duration(b.ms) })}</title>
          </g>
        ))}
        {trace.prompts.map((p) => (
          <circle key={p} cx={LABEL_W + axis.x(p)} cy={LANE_H / 2} r={3} className="trace-prompt">
            <title>{`${t("traceYou")} · ${hhmm(p, tz)}`}</title>
          </circle>
        ))}
        {lanes.map((a, i) =>
          a.segments.map(([s, e], j) => (
            <rect key={`${a.actor_id}${j}`} x={LABEL_W + axis.x(s)} y={top + i * LANE_H + 8} width={Math.max(2, axis.x(e) - axis.x(s))} height={LANE_H - 16} rx={3} className="trace-run">
              <title>{`${actorLabel(a)} · ${hhmm(s, tz)}–${hhmm(e, tz)} · ${duration(e - s)}`}</title>
            </rect>
          )),
        )}
        {trace.tools.map((tool, k) => {
          const cat = toolCategory(tool.name, tool.kind);
          const lane = laneOf(tool.actor_id);
          const x = LABEL_W + axis.x(tool.at);
          const y = top + lane * LANE_H;
          return (
            <line key={k} x1={x} x2={x} y1={y + 5} y2={y + LANE_H - 5} className={`trace-tool cat-${cat}`}>
              <title>{`${tool.name} · ${t(CAT_LABEL[cat])} · ${hhmm(tool.at, tz)}`}</title>
            </line>
          );
        })}
        {ticks.map((tk, i) => (
          <text key={tk} x={LABEL_W + (i === 0 ? 0 : width)} y={height - 6} textAnchor={i === 0 ? "start" : "end"} className="trace-axis">
            {hhmm(tk, tz)}
          </text>
        ))}
      </svg>
      <figcaption className="note">
        <span className="legend">
          {CATEGORIES.map((c) => (
            <span key={c}>
              <i className={`swatch cat-${c}`} aria-hidden /> {t(CAT_LABEL[c])}
            </span>
          ))}
        </span>
        <br />
        {t("traceLegend")}
      </figcaption>
    </figure>
  );
}

export function ContextView({ trace }: { trace: SessionTrace }) {
  const subs = new Set(trace.actors.filter((a) => a.actor_kind === "known").map((a) => a.actor_id));
  const main = trace.points.filter((p) => p.context !== null && !subs.has(p.actor_id ?? ""));
  const all = trace.points.filter((p) => p.context !== null);
  if (main.length === 0) return <p className="muted">{t("ctxNone")}</p>;
  const peak = Math.max(...main.map((p) => p.context!));
  const last = main[main.length - 1].context!;
  const cacheRead = all.reduce((n, p) => n + (p.cache_read ?? 0), 0);
  const total = all.reduce((n, p) => n + p.context!, 0);
  const w = 720;
  const h = 150;
  const x = (i: number) => (main.length === 1 ? w / 2 : (i / (main.length - 1)) * w);
  const y = (v: number) => h - 4 - (v / peak) * (h - 12);
  const path = main.map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(p.context!).toFixed(1)}`).join(" ");
  const drops = main.flatMap((p, i) => (i > 0 && p.context! < main[i - 1].context! * 0.5 ? [i] : []));
  return (
    <div>
      <dl className="facts">
        <div>
          <dd>{compact(peak)}</dd>
          <dt>{t("ctxPeak")}</dt>
        </div>
        <div>
          <dd>{compact(last)}</dd>
          <dt>{t("ctxLast")}</dt>
        </div>
        <div>
          <dd>{main.length}</dd>
          <dt>{t("ctxRequests")}</dt>
        </div>
        <div>
          <dd>{total > 0 ? `${Math.round((cacheRead / total) * 100)}%` : "—"}</dd>
          <dt>{t("ctxCached")}</dt>
        </div>
      </dl>
      <figure className="context-chart" aria-label={t("ctxAria", { peak: compact(peak), n: main.length })}>
        <svg viewBox={`0 0 ${w} ${h}`} width="100%" aria-hidden="true">
          <path d={`${path} L${x(main.length - 1)},${h} L${x(0)},${h} Z`} className="ctx-area" />
          <path d={path} className="ctx-line" />
          {drops.map((i) => (
            <circle key={i} cx={x(i)} cy={y(main[i].context!)} r={4} className="ctx-drop">
              <title>{t("ctxDrop")}</title>
            </circle>
          ))}
        </svg>
        <figcaption className="note">{t("ctxNote")}</figcaption>
      </figure>
    </div>
  );
}

export function ToolsView({ trace }: { trace: SessionTrace }) {
  if (trace.tool_summary.length === 0) return <p className="muted">{t("toolsNone")}</p>;
  const max = Math.max(...trace.tool_summary.map((x) => x.calls));
  return (
    <div>
      <table className="data tools-table">
        <thead>
          <tr>
            <th>{t("tabTools")}</th>
            <th>{t("toolsKind")}</th>
            <th>{t("toolsCalls")}</th>
          </tr>
        </thead>
        <tbody>
          {trace.tool_summary.map((x) => (
            <tr key={`${x.kind}:${x.name}`}>
              <td className="mono">{x.name}</td>
              <td>{t(CAT_LABEL[toolCategory(x.name, x.kind)])}</td>
              <td>
                <span className="bar-track inline" aria-hidden>
                  <i style={{ width: `${Math.max(3, Math.round((x.calls / max) * 100))}%` }} />
                </span>
                <span className="mono">{x.calls}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="note">{t("toolsNote")}</p>
    </div>
  );
}

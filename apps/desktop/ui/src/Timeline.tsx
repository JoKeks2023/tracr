import { useMemo, useState } from "react";
import { duration, hhmm, shortId } from "./format";
import type { ActorDay, DayAgents, Segment, SessionDay } from "./types";

const HOUR = 3_600_000;
const PENDING_LANES: Record<string, string> = {
  human: "Human time",
  device: "Device",
  coverage: "Coverage",
};
/** More subagents than this collapse into one overlay row. */
const INLINE_SUBAGENTS = 3;

interface Range {
  start: number;
  end: number;
}

/** The part of the day with evidence, widened to whole hours. */
function visibleRange(day: DayAgents): Range {
  const times: number[] = [];
  for (const s of day.sessions) {
    times.push(...s.engagement_instants);
    for (const a of s.actors) for (const g of a.segments) times.push(g.start, g.end);
  }
  if (times.length === 0) return { start: day.start, end: day.end };
  const lo = Math.max(day.start, Math.floor(Math.min(...times) / HOUR) * HOUR);
  const hi = Math.min(day.end, Math.ceil(Math.max(...times) / HOUR) * HOUR);
  return hi - lo < 2 * HOUR ? { start: lo, end: Math.min(day.end, lo + 2 * HOUR) } : { start: lo, end: hi };
}

function ticks(r: Range): number[] {
  const hours = (r.end - r.start) / HOUR;
  const step = hours <= 12 ? 1 : hours <= 24 ? 2 : 3;
  const out: number[] = [];
  for (let t = Math.ceil(r.start / HOUR) * HOUR; t <= r.end; t += step * HOUR) out.push(t);
  return out;
}

const pct = (r: Range, t: number) => `${((t - r.start) / (r.end - r.start)) * 100}%`;
const width = (r: Range, a: number, b: number) => `${((b - a) / (r.end - r.start)) * 100}%`;

function Grid({ range, marks }: { range: Range; marks: number[] }) {
  return (
    <div className="grid" aria-hidden>
      {marks.map((t) => (
        <div key={t} className="line" style={{ left: pct(range, t) }} />
      ))}
    </div>
  );
}

function Bars({ segments, range, tz, sub, overlay }: {
  segments: Segment[];
  range: Range;
  tz: string;
  sub?: boolean;
  overlay?: boolean;
}) {
  return (
    <>
      {segments.map((g, i) => {
        const cls = [
          "bar",
          sub && "sub",
          overlay && "overlay",
          g.clipped_start ? "clipped-start" : g.start_basis === "bounded" && "bounded-start",
          g.clipped_end ? "clipped-end" : g.end_basis === "bounded" && "bounded-end",
        ]
          .filter(Boolean)
          .join(" ");
        const title =
          `${hhmm(g.start, tz)}–${hhmm(g.end, tz)} · ${duration(g.end - g.start)} agent execution\n` +
          `start ${g.clipped_start ? "before this day" : g.start_basis} · end ${g.clipped_end ? "after this day" : g.end_basis}`;
        return (
          <div
            key={i}
            className={cls}
            title={title}
            style={{ left: pct(range, g.start), width: width(range, g.start, g.end) }}
          />
        );
      })}
    </>
  );
}

function actorLabel(a: ActorDay) {
  if (a.actor_kind === "known" && a.native_agent_id) {
    return (
      <>
        agent <span className="mono">{shortId(a.native_agent_id, 10)}</span>
      </>
    );
  }
  return (
    <span title="Main-chain activity. The transcripts do not document which agent this is, so tracr does not claim one (AD-21).">
      main chain <span className="muted">· actor unknown</span>
    </span>
  );
}

function SessionLane({ s, range, tz, marks, selected, onSelect }: {
  s: SessionDay;
  range: Range;
  tz: string;
  marks: number[];
  selected: boolean;
  onSelect: () => void;
}) {
  const [open, setOpen] = useState(false);
  const main = s.actors.filter((a) => a.actor_kind !== "known");
  const subs = s.actors.filter((a) => a.actor_kind === "known");
  const collapse = subs.length > INLINE_SUBAGENTS;
  const subExec = subs.reduce((n, a) => n + a.execution_ms, 0);
  return (
    <div className={`session${selected ? " selected" : ""}`}>
      <div
        className="row session-head"
        onClick={onSelect}
        onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect()}
        role="button"
        tabIndex={0}
        aria-label={`Session ${s.label ?? s.native_session_id}, open details`}
      >
        <div className="label">
          <span className="name">{s.label ?? "no working directory"}</span>
          <span className="id mono">{shortId(s.native_session_id)}</span>
        </div>
        <div className="track">
          <Grid range={range} marks={marks} />
          {s.engagement_instants.map((t, i) => (
            <div key={i} className="instant" style={{ left: pct(range, t) }} title={`Human prompt ${hhmm(t, tz)}`} />
          ))}
        </div>
      </div>
      {main.map((a) => (
        <div className="row actor-row" key={a.actor_id}>
          <div className="label">{actorLabel(a)}</div>
          <div className="track">
            <Grid range={range} marks={marks} />
            <Bars segments={a.segments} range={range} tz={tz} />
          </div>
        </div>
      ))}
      {collapse && (
        <div className="row actor-row">
          <div className="label">
            <button className="expander" onClick={() => setOpen(!open)} aria-expanded={open}>
              <span className={`chevron${open ? " open" : ""}`}>▶</span>
              {subs.length} subagents <span className="muted">· {duration(subExec)}</span>
            </button>
          </div>
          <div className="track">
            <Grid range={range} marks={marks} />
            {!open && <Bars segments={subs.flatMap((a) => a.segments)} range={range} tz={tz} sub overlay />}
          </div>
        </div>
      )}
      {(!collapse || open) &&
        subs.map((a) => (
          <div className="row actor-row" key={a.actor_id}>
            <div className="label">{actorLabel(a)}</div>
            <div className="track">
              <Grid range={range} marks={marks} />
              <Bars segments={a.segments} range={range} tz={tz} sub />
            </div>
          </div>
        ))}
    </div>
  );
}

export function Timeline({ day, selected, onSelect }: {
  day: DayAgents;
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  const tz = day.timezone;
  const range = useMemo(() => visibleRange(day), [day]);
  const marks = useMemo(() => ticks(range), [range]);
  const sessions = useMemo(() => {
    const first = (s: SessionDay) =>
      Math.min(...s.engagement_instants, ...s.actors.flatMap((a) => a.segments.map((g) => g.start)), Infinity);
    return [...day.sessions].sort((a, b) => first(a) - first(b));
  }, [day]);
  const pending = day.pending.filter((p) => p.lane in PENDING_LANES);

  return (
    <div className="timeline">
      <div className="axis">
        <div className="muted" style={{ fontSize: 12 }}>
          {tz}
        </div>
        <div className="ticks">
          {marks.map((t) => (
            <span key={t} className="tick" style={{ left: pct(range, t) }}>
              {hhmm(t, tz)}
            </span>
          ))}
        </div>
      </div>

      {pending.map((p) => (
        <div className="row" key={p.lane}>
          <div className="label secondary">{PENDING_LANES[p.lane]}</div>
          <div className="track">
            <div className="pending-track" title={p.reason}>
              Not derived yet
            </div>
          </div>
        </div>
      ))}

      <div className="group-title">Agent sessions · each listed on its own, overlapping freely</div>
      {sessions.map((s) => (
        <SessionLane
          key={s.session_id}
          s={s}
          range={range}
          tz={tz}
          marks={marks}
          selected={selected === s.session_id}
          onSelect={() => onSelect(s.session_id)}
        />
      ))}

      <div className="legend">
        <span>
          <span className="swatch inst" />
          Human prompt (an instant, no duration)
        </span>
        <span>
          <span className="swatch bar-sw" />
          Agent execution, main chain
        </span>
        <span>
          <span className="swatch bar-sw sub" />
          Agent execution, subagent
        </span>
        <span>Square end: the agent may have run a little longer than its last record</span>
      </div>
    </div>
  );
}

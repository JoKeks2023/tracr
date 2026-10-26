import { QualityFooter } from "./QualityFooter";
import { t } from "./i18n";
import { useMemo, useState } from "react";
import { duration, hhmm, shortId } from "./format";
import type { ActorDay, DayAgents, HumanAtom, LaneInterval, Segment, SessionDay } from "./types";

const HOUR = 3_600_000;
const REASON_LABEL: Record<string, string> = {
  coverage_gap: "coverage gap: tracr was not collecting everything needed",
  conflicting_evidence: "conflicting evidence: prompts in several sessions interleave",
  unresolved_window: "unresolved: the gap could not be bridged",
};

const COVERAGE_RANK: Record<string, number> = { collecting: 0, unknown: 1, missing: 2 };

/** Worst-of coverage over all sources: the display roll-up (§4.7). */
function coverageRollup(coverage: Record<string, LaneInterval[]>): LaneInterval[] {
  const bounds = new Set<number>();
  const all = Object.entries(coverage);
  for (const [, v] of all) for (const i of v) {
    bounds.add(i.start);
    bounds.add(i.end);
  }
  const sorted = [...bounds].sort((a, b) => a - b);
  const out: LaneInterval[] = [];
  for (let n = 0; n + 1 < sorted.length; n++) {
    const [a, b] = [sorted[n], sorted[n + 1]];
    let worst: LaneInterval | null = null;
    const notes: string[] = [];
    for (const [key, v] of all) {
      const hit = v.find((i) => i.start <= a && i.end > a);
      if (!hit) continue;
      if (hit.state !== "collecting") notes.push(`${key}: ${hit.state}${hit.reason ? ` (${hit.reason.replaceAll("_", " ")})` : ""}`);
      if (!worst || COVERAGE_RANK[hit.state] > COVERAGE_RANK[worst.state]) worst = hit;
    }
    if (!worst) continue;
    const reason = notes.join("; ") || null;
    const last = out[out.length - 1];
    if (last && last.end === a && last.state === worst.state && last.reason === reason) last.end = b;
    else out.push({ start: a, end: b, state: worst.state, reason });
  }
  return out;
}
/** More subagents than this collapse into one overlay row. */
const INLINE_SUBAGENTS = 3;

interface Range {
  start: number;
  end: number;
}

/** The part of the day with evidence, widened to whole hours. */
function visibleRange(day: DayAgents): Range {
  const times: number[] = [];
  for (const a of day.human.atoms) times.push(a.start, a.end);
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
      {t("mainChain")} <span className="muted">· {t("actorUnknown")}</span>
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
        aria-label={t("sessionOpen", { name: s.label ?? s.native_session_id })}
      >
        <div className="label">
          <span className="name" title={s.cwd ?? undefined}>
            {s.repository_label ?? s.label ?? "no working directory"}
          </span>
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

export function Timeline({ day, selected, onSelect, onAtom }: {
  day: DayAgents;
  selected: string | null;
  onSelect: (id: string) => void;
  onAtom: (a: HumanAtom) => void;
}) {
  const tz = day.timezone;
  const range = useMemo(() => visibleRange(day), [day]);
  const marks = useMemo(() => ticks(range), [range]);
  const sessions = useMemo(() => {
    const first = (s: SessionDay) =>
      Math.min(...s.engagement_instants, ...s.actors.flatMap((a) => a.segments.map((g) => g.start)), Infinity);
    return [...day.sessions].sort((a, b) => first(a) - first(b));
  }, [day]);
  const coverage = useMemo(() => coverageRollup(day.coverage), [day]);

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

      <div className="row">
        <div className="label secondary">{day.review ? t("humanTimeNow") : t("humanTime")}</div>
        <div className="track">
          <Grid range={range} marks={marks} />
          {day.human.atoms.map((a, i) => (
            <div
              key={i}
              role="button"
              tabIndex={0}
              onClick={() => onAtom(a)}
              onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onAtom(a)}
              className={`human ${a.status}${a.decided_by ? " corrected" : ""}`}
              style={{ left: pct(range, Math.max(a.start, range.start)), width: width(range, Math.max(a.start, range.start), Math.min(a.end, range.end)) }}
              title={
                a.status === "work"
                  ? `${hhmm(a.start, tz)}–${hhmm(a.end, tz)} · ${duration(a.end - a.start)} counted (inferred, provisional)\n${
                      a.attribution?.state === "confirmed"
                        ? `→ ${a.attribution.labels.join(", ")} (${a.attribution.level})`
                        : a.attribution?.state === "conflicting"
                          ? `conflicting: ${a.attribution.labels.join(", ")}`
                          : "unattributed: no repository context (root not approved or not a repository)"
                    }`
                  : `${hhmm(a.start, tz)}–${hhmm(a.end, tz)} · ${duration(a.end - a.start)} unknown, not counted\n${REASON_LABEL[a.unknown_reason ?? ""] ?? a.unknown_reason}${a.proposals.length ? `\nProposal: work may have ended at ${hhmm(a.proposals[0].at, tz)} (last input)` : ""}`
              }
            />
          ))}
        </div>
      </div>
      {day.review && (
        <div className="row">
          <div className="label secondary">{t("reviewedLane", { rev: day.review.rev })}</div>
          <div className="track">
            <Grid range={range} marks={marks} />
            {day.review.snapshot.intervals.map((a, i) => (
              <div
                key={i}
                className={`human reviewed ${a.status}`}
                style={{ left: pct(range, Math.max(a.start, range.start)), width: width(range, Math.max(a.start, range.start), Math.min(a.end, range.end)) }}
                title={`${hhmm(a.start, tz)}–${hhmm(a.end, tz)} · ${duration(a.end - a.start)} ${a.status === "work" ? "counted" : a.status === "unknown" ? "unknown" : "break"} (reviewed)${a.target_labels.length ? `\n→ ${a.target_labels.join(", ")}` : ""}`}
              />
            ))}
          </div>
        </div>
      )}
      <div className="row">
        <div className="label secondary">{t("device")}</div>
        <div className="track">
          <Grid range={range} marks={marks} />
          {day.device.map((d, i) => (
            <div
              key={i}
              className={`lane device-${d.state}`}
              style={{ left: pct(range, Math.max(d.start, range.start)), width: width(range, Math.max(d.start, range.start), Math.min(d.end, range.end)) }}
              title={`${hhmm(d.start, tz)}–${hhmm(d.end, tz)} · ${d.state}`}
            />
          ))}
        </div>
      </div>
      <div className="row">
        <div className="label secondary">{t("coverage")}</div>
        <div className="track">
          <Grid range={range} marks={marks} />
          {day.quality.stream_extents.map((e, i) => (
            <div
              key={`x${i}`}
              className={`lane coverage-${e.state === "gap" ? "missing" : "degraded"} stream-extent`}
              style={{ left: pct(range, Math.max(e.start, range.start)), width: width(range, Math.max(e.start, range.start), Math.min(e.end, range.end)) }}
              title={`${hhmm(e.start, tz)}–${hhmm(e.end, tz)} · ${e.state === "gap" ? "ingestion gap" : "degraded (unreadable record)"}${e.reason ? `\n${e.reason.replaceAll("_", " ")}` : ""}`}
            />
          ))}
          {coverage.map((c, i) => (
            <div
              key={i}
              className={`lane coverage-${c.state}`}
              style={{ left: pct(range, Math.max(c.start, range.start)), width: width(range, Math.max(c.start, range.start), Math.min(c.end, range.end)) }}
              title={`${hhmm(c.start, tz)}–${hhmm(c.end, tz)} · ${c.state}${c.reason ? `\n${c.reason}` : ""}`}
            />
          ))}
        </div>
      </div>

      <div className="group-title">{t("agentSessionsTitle")}</div>
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
          {t("legendPrompt")}
        </span>
        <span>
          <span className="swatch bar-sw" />
          {t("legendMain")}
        </span>
        <span>
          <span className="swatch bar-sw sub" />
          {t("legendSub")}
        </span>
        <span>
          <span className="swatch human-sw" />
          {t("legendCounted")}
        </span>
        <span>
          <span className="swatch unknown-sw" />
          {t("legendUnknown")}
        </span>
        <span>
          <span className="swatch nonwork-sw" />
          {t("legendBreak")}
        </span>
        <span>{t("legendBlank")}</span>
      </div>
      <QualityFooter day={day} />
    </div>
  );
}

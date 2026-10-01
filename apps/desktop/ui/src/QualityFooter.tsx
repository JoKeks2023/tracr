import { duration, reasonText } from "./format";
import type { DayAgents } from "./types";

const NOTICES: Record<string, string> = {
  producer_version_not_fixture_validated: "records from producer versions not fixture-validated",
  unlinked_subagent: "subagent threads kept apart (parent link not validated)",
  prompt_from_non_interactive_source: "prompts from non-interactive sources (not engagement)",
  prompt_from_non_interactive_entrypoint: "prompts from non-interactive entrypoints (not engagement)",
  cumulative_counter_reset: "usage counter resets",
};

/** The day's quality footer (architecture §13.4, reporting §4). Calm text, no alarms. */
export function QualityFooter({ day }: { day: DayAgents }) {
  const tz = day.timezone;
  const q = day.quality;
  const coverage = Object.entries(day.coverage).map(([kind, lanes]) => {
    const by: Record<string, number> = {};
    for (const l of lanes) by[l.state] = (by[l.state] ?? 0) + (l.end - l.start);
    return `${kind.replaceAll("_", " ")}: ${Object.entries(by)
      .map(([s, ms]) => `${s} ${duration(ms)}`)
      .join(", ")}`;
  });
  const diags: Record<string, number> = {};
  for (const s of day.sessions) for (const [k, n] of Object.entries(s.diagnostics)) diags[k] = (diags[k] ?? 0) + n;
  const unknownActorRecords = day.sessions
    .flatMap((s) =>
      s.usage.filter((u) => s.actors.some((a) => a.actor_id === u.actor_id && a.actor_kind === "unknown")),
    )
    .reduce((n, u) => n + u.records, 0);
  const c = day.cost.identified;
  const tokens = c.priced_tokens + c.unpriced_tokens;
  const unpriced = tokens > 0 ? Math.round((c.unpriced_tokens / tokens) * 100) : 0;
  const reported = day.sessions.some((s) => s.reported.length > 0);
  const gaps = q.stream_extents.filter((e) => e.state === "gap").length;

  const rows: [string, string][] = [
    ["Coverage", coverage.length ? coverage.join(" · ") : "none recorded"],
    [
      "Ingestion",
      (q.ingestion_complete
        ? "complete"
        : q.ingestion_verified_through
          ? `verified through ${new Date(q.ingestion_verified_through).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: tz })}`
          : "not verified") +
        (q.recovered_records > 0 ? ` · ${q.recovered_records} records recovered late` : "") +
        (gaps > 0 ? ` · ${gaps} stream gaps` : ""),
    ],
    ["Your records", `${day.corrections.length} corrections · ${day.manual_entries.length} manual entries`],
    [
      "Records",
      `${day.ledger_counts.quarantined} quarantined · ${day.ledger_counts.key_conflicts} key-content conflicts (recorded this day)`,
    ],
    ...Object.entries(NOTICES)
      .filter(([k]) => diags[k])
      .map(([k, text]): [string, string] => ["Notice", `${diags[k]} ${text}`]),
    ["Usage", `${unknownActorRecords} records with actor unknown: the main agent is not documented, never assumed`],
    [
      "Cost",
      `estimated API cost${reported ? ", provider-reported" : ""} · ${unpriced}% of tokens unpriced · catalog ${day.cost.pricing.catalog_version} as of ${day.cost.pricing.oldest_fetched_at ?? "unknown"}` +
        (c.assumptions.length ? ` · assumed: ${c.assumptions.map(reasonText).join("; ")}` : ""),
    ],
    [
      "Content",
      "never stored: prompt, message and tool text never entered tracr. Search, prompt-based suggestions and re-extraction are unavailable.",
    ],
  ];

  return (
    <section className="quality" aria-label="Quality">
      <h3>Quality</h3>
      <dl>
        {rows.map(([k, v], i) => (
          <div key={i} className="quality-row">
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

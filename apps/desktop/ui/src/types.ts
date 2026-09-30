// Mirrors of the tracr-app DTOs (crates/tracr-app/src/day.rs). Times are
// epoch milliseconds (UTC); the day's timezone is part of the payload.

export interface DayList {
  timezone: string;
  timezone_from_setting: boolean;
  days: string[]; // newest first, YYYY-MM-DD
  derived_as_of: number | null;
}

export interface Segment {
  start: number;
  end: number;
  start_basis: "observed" | "bounded" | string;
  end_basis: "observed" | "bounded" | string;
  clipped_start: boolean;
  clipped_end: boolean;
}

export interface ActorDay {
  actor_id: string;
  actor_kind: "known" | "main" | "unknown";
  native_agent_id: string | null;
  segments: Segment[];
  execution_ms: number;
  turns: number;
  tool_calls: number;
}

/** Sum over records that state the category, and records that do not (unknown ≠ 0). */
export interface TokenCount {
  sum: number;
  unknown_records: number;
}

export interface UsageLine {
  actor_id: string | null;
  model: string | null;
  identified: boolean;
  records: number;
  input_fresh: TokenCount;
  cache_creation: TokenCount;
  cache_read: TokenCount;
  output: TokenCount;
}

export interface SessionDay {
  session_id: string;
  provider: string;
  native_session_id: string;
  label: string | null;
  cwd: string | null;
  git_branch: string | null;
  first_evidence_at: number | null;
  last_evidence_at: number | null;
  diagnostics: Record<string, number>;
  engagement_instants: number[];
  actors: ActorDay[];
  usage: UsageLine[];
}

export interface PendingLane {
  lane: "human" | "device" | "coverage" | "cost" | string;
  reason: string;
}

export interface DayAgents {
  day: string;
  timezone: string;
  start: number;
  end: number;
  derived_as_of: number | null;
  sessions: SessionDay[];
  pending: PendingLane[];
}

export interface RuntimeStatus {
  collecting: boolean;
  run_id: string | null;
  keys: Record<string, string>;
  last_heartbeat: number | null;
  last_rebuild: number | null;
  last_error: string | null;
  sensor: string;
}

export interface Status {
  data_dir: string | null;
  data_dir_from_env: boolean;
  ledger_present: boolean;
  approved_roots: string[];
  proposed_roots: { path: string; approved: boolean }[];
  collector: "collecting" | "elsewhere" | "stopped" | "not_configured";
  collector_detail: string | null;
  runtime: RuntimeStatus | null;
}

export interface Refreshed {
  inserted: number;
  sessions: number;
  seconds: number;
}

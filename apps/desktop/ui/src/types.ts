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
  cost: CostView;
}

/** Estimates of usage records under one pricing revision. Not spend.
 * `amount_nano` covers the priced parts only; null means unpriced, never zero. */
export interface CostView {
  amount_nano: number | null;
  priced_records: number;
  partially_priced_records: number;
  unpriced_records: number;
  not_estimated_records: number;
  priced_tokens: number;
  unpriced_tokens: number;
  reasons: string[];
  assumptions: string[];
}

/** The provider's own running cost figure: never summed, not spend. */
export interface ReportedView {
  source_field: string;
  value_as_stated: string;
  value_nano: number | null;
  currency: string;
  semantics: string;
  scope: string;
  recorded_at: number;
  label: string;
}

export interface PricingInfo {
  revision: string;
  catalog_version: string;
  currency: string;
  source_urls: string[];
  oldest_fetched_at: string | null;
  label: string;
}

export interface DayCost {
  pricing: PricingInfo;
  identified: CostView;
  /** Usage without a dedup identity: its own line (AD-17). */
  unidentified: CostView;
  /** Usage whose extent crosses a day boundary: belongs to no single day (AD-16). */
  spanning_records: number;
  spanning: CostView;
}

export interface SessionDay {
  /** Work the provider runs by itself (e.g. Codex's auto-review). */
  automated: boolean;
  session_id: string;
  provider: string;
  native_session_id: string;
  label: string | null;
  cwd: string | null;
  git_branch: string | null;
  repository: string | null;
  repository_label: string | null;
  first_evidence_at: number | null;
  last_evidence_at: number | null;
  diagnostics: Record<string, number>;
  engagement_instants: number[];
  actors: ActorDay[];
  usage: UsageLine[];
  cost: CostView;
  reported: ReportedView[];
}

export interface PendingLane {
  lane: "human" | "device" | "coverage" | "cost" | string;
  reason: string;
}

export interface HumanAtom {
  start: number;
  end: number;
  status: "work" | "unknown" | "non_work";
  unknown_reason: "coverage_gap" | "conflicting_evidence" | "unresolved_window" | null;
  duration_basis: string;
  engaged_session: string | null;
  start_boundary: string | null;
  end_boundary: string | null;
  reasons: string[];
  proposals: { at: number; kind: string }[];
  /** Report-target attribution; work only. */
  attribution: {
    level: "explicit" | "repository" | "worktree" | "none";
    state: "confirmed" | "conflicting" | "unattributed";
    targets: string[];
    labels: string[];
  } | null;
  /** The correction that decided the status (else inference). */
  decided_by: string | null;
  /** The derivation underneath: work | unknown | non_work | blank. */
  candidate_status: string | null;
  candidate_reason: string | null;
  /** The manual entry that counts this time, if any. */
  manual_entry: string | null;
}

export interface ManualEntryInfo {
  id: string;
  rev: number;
  kind: "timer" | "retroactive";
  start: number;
  end: number | null;
  running: boolean;
  target: string | null;
  target_label: string | null;
  note: string | null;
}

export interface ManualRequest {
  op: "start_timer" | "stop_timer" | "add" | "edit" | "delete";
  id?: string;
  start?: number;
  end?: number;
  target?: string;
  note?: string;
}

export interface CorrectionInfo {
  id: string;
  op: "confirm_work" | "mark_non_work" | "set_target" | "reject_target" | "revoke";
  start: number | null;
  end: number | null;
  target: string | null;
  target_label: string | null;
  revokes: string | null;
  at: number;
  status: "effective" | "partly superseded" | "superseded" | "revoked";
  reason: string | null;
}

export interface TargetOption {
  key: string;
  label: string;
  group: "project" | "repository" | "category" | "other";
}

export interface CorrectionRequest {
  op: CorrectionInfo["op"];
  start?: number;
  end?: number;
  target?: string;
  revokes?: string;
  ui_op?: string;
  reason?: string;
}

/** Counted and unknown are separate figures and are never added together. */
export interface HumanDay {
  atoms: HumanAtom[];
  counted_ms: number;
  unknown_ms: number;
  unknown_by_reason: Record<string, number>;
  /** Counted time per report target label (repository, "conflicting", "unattributed"). */
  counted_by_target: Record<string, number>;
  /** First to last human prompt: an upper bound, never a total. */
  evidence_span: [number, number] | null;
}

export interface LaneInterval {
  start: number;
  end: number;
  state: string;
  reason: string | null;
}

export interface ReviewedInterval {
  start: number;
  end: number;
  status: "work" | "unknown" | "non_work";
  duration_basis: string;
  unknown_reason: string | null;
  attribution_state: string | null;
  target_keys: string[];
  target_labels: string[];
  engaged_session: string | null;
}

export interface Snapshot {
  day: string;
  timezone: string;
  intervals: ReviewedInterval[];
  counted_ms: number;
  unknown_ms: number;
  non_work_ms: number;
  counted_by_target: Record<string, number>;
  counted_by_basis: Record<string, number>;
  derived_as_of: number | null;
}

export interface DayAgents {
  /** counted minutes grouped by project; empty until a project exists */
  counted_by_project: ProjectTotal[];
  day: string;
  timezone: string;
  start: number;
  end: number;
  derived_as_of: number | null;
  sessions: SessionDay[];
  human: HumanDay;
  device: LaneInterval[];
  coverage: Record<string, LaneInterval[]>;
  corrections: CorrectionInfo[];
  manual_entries: ManualEntryInfo[];
  review: { revision_id: string; rev: number; at: number; reason: string | null; snapshot: Snapshot } | null;
  discrepancy: {
    fingerprint: string;
    counted_delta_ms: number;
    unknown_delta_ms: number;
    non_work_delta_ms: number;
    draft: Snapshot;
  } | null;
  quality: {
    provisional: boolean;
    provisional_reasons: string[];
    ingestion_complete: boolean;
    ingestion_verified_through: number | null;
    recovered_records: number;
    stream_extents: LaneInterval[];
  };
  cost: DayCost;
  ledger_counts: { quarantined: number; key_conflicts: number };
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
  approved_repository_roots: string[];
  proposed_roots: { path: string; approved: boolean }[];
  home: string | null;
  app_version: string;
  timezone: string | null;
  timezone_chosen: boolean;
  system_timezone: string | null;
  reviewed_days: number;
  collector: "collecting" | "stalled" | "elsewhere" | "stopped" | "not_configured";
  collector_detail: string | null;
  runtime: RuntimeStatus | null;
  running_timer: ManualEntryInfo | null;
}

export interface Refreshed {
  inserted: number;
  sessions: number;
  seconds: number;
}

export interface Project {
  id: string;
  name: string;
}

export interface ProjectsView {
  projects: Project[];
  /** repository key → project id */
  repository_projects: Record<string, string>;
  repositories: { key: string; label: string }[];
}

export interface ProjectTotal {
  kind: "project" | "other";
  project_id: string | null;
  label: string;
  ms: number;
  parts: { label: string; ms: number }[];
}

export type ProjectOp =
  | { op: "create"; name: string }
  | { op: "rename"; id: string; name: string }
  | { op: "delete"; id: string }
  | { op: "map"; repository: string; project: string | null };

export interface SessionList {
  timezone: string;
  from: string;
  to: string;
  sessions: SessionDay[];
  cost: CostView;
  pricing: PricingInfo;
}

export interface TraceActor {
  actor_id: string;
  actor_kind: string;
  native_agent_id: string | null;
  segments: [number, number][];
}
export interface TraceTool {
  actor_id: string | null;
  name: string;
  kind: string;
  at: number;
  finished_at: number | null;
}
export interface ToolSummary {
  name: string;
  kind: string;
  calls: number;
}
export interface TracePoint {
  at: number;
  actor_id: string | null;
  model: string | null;
  context: number | null;
  cache_read: number | null;
  output: number | null;
}
export interface SessionTrace {
  session_id: string;
  timezone: string;
  start: number;
  end: number;
  actors: TraceActor[];
  prompts: number[];
  tools: TraceTool[];
  tool_summary: ToolSummary[];
  points: TracePoint[];
}

export interface LimitWindow {
  provider: "codex" | "claude_code" | string;
  /** `five_hour`, `weekly` or `minutes:<n>`. */
  window: string;
  window_minutes: number | null;
  used_percent: number;
  resets_at: number | null;
  observed_at: number;
  plan: string | null;
  /** The reset time has passed since the figure was stated. */
  expired: boolean;
}
export interface TapState {
  installed: boolean;
  chained: boolean;
  settings_path: string;
}
export interface Credits {
  provider: string;
  has_credits: boolean;
  unlimited: boolean;
  balance: string | null;
  observed_at: number;
}
export interface Limits {
  windows: LimitWindow[];
  credits: Credits[];
  claude_tap: TapState;
}

export interface RepoPart {
  key: string;
  label: string;
  sessions: number;
  agent_ms: number;
}
export interface ProjectEntry {
  key: string;
  kind: "project" | "repository" | "folder" | "none";
  project_id: string | null;
  label: string;
  repositories: RepoPart[];
  session_ids: string[];
  sessions: number;
  agent_ms: number;
  cost: CostView;
  first_at: number | null;
  last_at: number | null;
  branches: string[];
  providers: string[];
}
export interface ProjectOverview {
  from: string;
  to: string;
  timezone: string;
  entries: ProjectEntry[];
  pricing: PricingInfo;
}

/** Stored beside the data: read by the window and by the menu bar alike. */
export interface QuotaPrefs {
  remaining: boolean;
  pace_aware: boolean;
  menu_bar: boolean;
  notify: boolean;
  rings: boolean;
  notify_waiting: boolean;
  notify_finished: boolean;
  show_automated: boolean;
}

export interface LiveSession {
  /** Claude's own session id (`native_session_id` of a derived session). */
  session: string;
  state: "working" | "waiting" | "your_turn" | "started";
  since: number;
  kind: string | null;
}
export interface HooksView {
  installed: boolean;
  live: LiveSession[];
}

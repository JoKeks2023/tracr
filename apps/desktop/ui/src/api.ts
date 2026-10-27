import { invoke } from "@tauri-apps/api/core";
export type RootKind = "provider" | "repository" | "config";
import type { ProjectOp, ProjectsView } from "./types";
import type { CorrectionRequest, ManualRequest, DayAgents, DayList, Refreshed, SessionList, SessionTrace, Limits, ProjectOverview, QuotaPrefs, HooksView, Analytics, CostAlertPrefs, Fleet, Status, TargetOption } from "./types";

// The only way the window reaches tracr: its own commands (AD-12).
export const api = {
  status: () => invoke<Status>("status"),
  days: () => invoke<DayList>("days"),
  dayAgents: (day: string) => invoke<DayAgents>("day_agents", { day }),
  /** Sessions active between two days (inclusive), newest first. */
  sessions: (from: string, to: string) => invoke<SessionList>("sessions", { from, to }),
  /** One session in time: actors, prompts, tool calls (names), request sizes. */
  sessionTrace: (sessionId: string) => invoke<SessionTrace>("session_trace", { sessionId }),
  /** Percent of each limit window as the providers report it. */
  limits: () => invoke<Limits>("limits"),
  /** Opt in (true) or out (false) of routing Claude Code's statusLine through tracr. */
  limitsTap: (install: boolean) => invoke<void>("limits_tap", { install }),
  /** Sessions of a range grouped by project, repository or none. */
  projectOverview: (from: string, to: string) => invoke<ProjectOverview>("project_overview", { from, to }),
  /** Running Claude Code sessions (registry of the approved home), spend of the last hour, cache life. */
  fleetState: () => invoke<Fleet>("fleet_state"),
  hooksState: () => invoke<HooksView>("hooks_state"),
  /** Opt in or out of Claude Code's hooks (needs you, your turn); a copy of the settings is made first. */
  hooksSet: (install: boolean) => invoke<void>("hooks_set", { install }),
  /** Cost per day and model, cache health and a what-if over a range of days (estimates). */
  analytics: (from: string, to: string) => invoke<Analytics>("analytics", { from, to }),
  getCostAlerts: () => invoke<CostAlertPrefs>("get_cost_alerts"),
  setCostAlerts: (prefs: CostAlertPrefs) => invoke<void>("set_cost_alerts", { prefs }),
  getQuotaPrefs: () => invoke<QuotaPrefs>("get_quota_prefs"),
  setQuotaPrefs: (prefs: QuotaPrefs) => invoke<void>("set_quota_prefs", { prefs }),
  refresh: () => invoke<Refreshed>("refresh"),
  /** Several corrections as one step: all are recorded or none. */
  correctBatch: (reqs: CorrectionRequest[]) => invoke<string[]>("correct_batch", { reqs }),
  correct: (req: CorrectionRequest) => invoke<string>("correct", { req }),
  manual: (req: ManualRequest) => invoke<string>("manual", { req }),
  review: (day: string, reason?: string) => invoke<string>("review", { day, reason }),
  dismiss: (day: string, fingerprint: string) => invoke<string>("dismiss", { day, fingerprint }),
  targets: () => invoke<TargetOption[]>("targets"),
  /** Approves `~/.claude` (Claude Code's home: the session registry, later its configuration). */
  approveConfigRoot: () => invoke<void>("approve_config_root"),
  approveRoots: (paths: string[]) => invoke<void>("approve_roots", { paths }),
  /** Opens the native folder panel and approves the pick. Resolves to the path, or null if cancelled. */
  chooseRoot: (kind: RootKind) => invoke<string | null>("choose_root", { kind }),
  /** Chooses the reporting time zone (null: follow the system) and rebuilds. Refused once a day is reviewed. */
  setTimezone: (name: string | null) => invoke<void>("set_timezone", { name }),
  setMenuLanguage: (german: boolean) => invoke<void>("set_menu_language", { german }),
  projects: () => invoke<ProjectsView>("projects"),
  /** Creates, renames or deletes a project, or maps a repository; resolves to the project id. */
  projectOp: (op: ProjectOp) => invoke<string>("project_op", { op }),
  /** Stops reading a root; what was imported stays in the ledger. */
  removeRoot: (kind: RootKind, path: string) => invoke<void>("remove_root", { kind, path }),
  /** Resolves to the written path, or null if the save panel was cancelled. */
  exportCsv: (kind: "timesheet" | "agent_sessions", from: string, to: string) =>
    invoke<string | null>("export_csv", { kind, from, to }),
  /** Resolves to the written folder, or null if cancelled. */
  exportEverything: () => invoke<string | null>("export_everything"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

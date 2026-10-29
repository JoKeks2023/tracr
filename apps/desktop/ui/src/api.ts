import { invoke } from "@tauri-apps/api/core";
export type RootKind = "provider" | "repository";
import type { ProjectOp, ProjectsView } from "./types";
import type { CorrectionRequest, ManualRequest, DayAgents, DayList, Refreshed, Status, TargetOption } from "./types";

// The only way the window reaches tracr: its own commands (AD-12).
export const api = {
  status: () => invoke<Status>("status"),
  days: () => invoke<DayList>("days"),
  dayAgents: (day: string) => invoke<DayAgents>("day_agents", { day }),
  refresh: () => invoke<Refreshed>("refresh"),
  correct: (req: CorrectionRequest) => invoke<string>("correct", { req }),
  manual: (req: ManualRequest) => invoke<string>("manual", { req }),
  review: (day: string, reason?: string) => invoke<string>("review", { day, reason }),
  dismiss: (day: string, fingerprint: string) => invoke<string>("dismiss", { day, fingerprint }),
  targets: () => invoke<TargetOption[]>("targets"),
  approveRoots: (paths: string[]) => invoke<void>("approve_roots", { paths }),
  /** Opens the native folder panel and approves the pick. Resolves to the path, or null if cancelled. */
  chooseRoot: (kind: RootKind) => invoke<string | null>("choose_root", { kind }),
  /** Chooses the reporting time zone (null: follow the system) and rebuilds. Refused once a day is reviewed. */
  setTimezone: (name: string | null) => invoke<void>("set_timezone", { name }),
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

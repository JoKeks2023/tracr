import { invoke } from "@tauri-apps/api/core";
import type { DayAgents, DayList, Refreshed, Status } from "./types";

// The only way the window reaches tracr: its own commands (AD-12).
export const api = {
  status: () => invoke<Status>("status"),
  days: () => invoke<DayList>("days"),
  dayAgents: (day: string) => invoke<DayAgents>("day_agents", { day }),
  refresh: () => invoke<Refreshed>("refresh"),
  approveRoot: (path: string) => invoke<void>("approve_root", { path }),
  /** Resolves to the written path, or null if the save panel was cancelled. */
  exportCsv: (kind: "timesheet" | "agent_sessions", from: string, to: string) =>
    invoke<string | null>("export_csv", { kind, from, to }),
  /** Resolves to the written folder, or null if cancelled. */
  exportEverything: () => invoke<string | null>("export_everything"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

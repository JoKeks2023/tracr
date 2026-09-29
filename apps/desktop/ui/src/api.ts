import { invoke } from "@tauri-apps/api/core";
import type { DayAgents, DayList, Refreshed, Status } from "./types";

// The only way the window reaches tracr: its own commands (AD-12).
export const api = {
  status: () => invoke<Status>("status"),
  days: () => invoke<DayList>("days"),
  dayAgents: (day: string) => invoke<DayAgents>("day_agents", { day }),
  refresh: () => invoke<Refreshed>("refresh"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

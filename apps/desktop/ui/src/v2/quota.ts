import { createContext, createElement, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { LimitWindow } from "../types";

/** Display preferences for quota windows: per viewer, kept in the browser storage of the window. */
export interface QuotaPrefs {
  /** Show what is left instead of what is used. */
  remaining: boolean;
  /** Judge a window by its pace (will it last until it resets?) rather than by the level alone. */
  paceAware: boolean;
}

const KEY = "tracr.quota.prefs";
const DEFAULTS: QuotaPrefs = { remaining: false, paceAware: true };

function load(): QuotaPrefs {
  try {
    const raw = window.localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<QuotaPrefs>) } : DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

const Ctx = createContext<{ prefs: QuotaPrefs; set: (p: Partial<QuotaPrefs>) => void }>({ prefs: DEFAULTS, set: () => {} });

export function QuotaPrefsProvider({ children }: { children: ReactNode }) {
  const [prefs, setPrefs] = useState<QuotaPrefs>(load);
  const value = useMemo(
    () => ({
      prefs,
      set: (p: Partial<QuotaPrefs>) =>
        setPrefs((cur) => {
          const next = { ...cur, ...p };
          try {
            window.localStorage.setItem(KEY, JSON.stringify(next));
          } catch {
            /* the preference just does not persist */
          }
          return next;
        }),
    }),
    [prefs],
  );
  return createElement(Ctx.Provider, { value }, children);
}

export const useQuotaPrefs = () => useContext(Ctx);

/** The current time, refreshed so countdowns and paces stay true. */
export function useNow(every = 30_000): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), every);
    return () => window.clearInterval(id);
  }, [every]);
  return now;
}

export type Pace = "hot" | "ontrack" | "room";

export interface Forecast {
  pace: Pace;
  /** Share of the window that had passed when the figure was stated (0..1). */
  elapsed: number;
  /** When the quota would be used up at the pace so far (epoch ms), if before the reset. */
  runsOutAt: number | null;
}

/** Under this share of the window gone, the pace is noise, not a rate. */
const MIN_ELAPSED = 0.03;
/** Percentage points ahead of the time gone that count as running hot. */
const HOT_POINTS = 5;

/**
 * Pace of a window: how fast it has been used against how fast its window
 * passed, as of the moment the provider stated it. `null` when there is no
 * window to measure, nothing used yet, or the window has already reset.
 */
export function forecastOf(w: LimitWindow): Forecast | null {
  if (w.expired || w.resets_at === null || w.window_minutes === null || w.window_minutes <= 0) return null;
  const length = w.window_minutes * 60_000;
  const start = w.resets_at - length;
  const elapsed = (w.observed_at - start) / length;
  if (elapsed < MIN_ELAPSED || elapsed > 1) return null;
  const used = Math.max(0, Math.min(100, w.used_percent));
  if (used >= 100) return null;
  const ahead = used - elapsed * 100;
  const pace: Pace = ahead > HOT_POINTS ? "hot" : ahead < -HOT_POINTS ? "room" : "ontrack";
  let runsOutAt: number | null = null;
  // Only a window that is running hot is said to run out: forecast and pace never contradict each other.
  if (pace === "hot" && used > 0) {
    const out = w.observed_at + ((100 - used) / used) * (w.observed_at - start);
    if (out < w.resets_at) runsOutAt = out;
  }
  return { pace, elapsed, runsOutAt };
}

export type Level = "ok" | "warn" | "crit" | "depleted";

/** Healthy from 50 % left, warning from 20 %, critical below, depleted at none; optionally judged by pace above critical. */
export function levelOf(w: LimitWindow, paceAware: boolean): Level {
  if (w.expired) return "ok";
  const left = 100 - w.used_percent;
  if (left <= 0) return "depleted";
  if (left < 20) return "crit";
  if (paceAware) {
    const f = forecastOf(w);
    if (f) return f.pace === "hot" ? "warn" : "ok";
  }
  return left < 50 ? "warn" : "ok";
}

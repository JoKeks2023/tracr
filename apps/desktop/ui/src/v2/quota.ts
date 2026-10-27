import { createContext, createElement, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api } from "../api";
import type { LimitWindow, QuotaPrefs } from "../types";

/** Display preferences for quota windows: kept beside the data, so the menu bar reads the same ones. */
export interface Prefs {
  remaining: boolean;
  paceAware: boolean;
  menuBar: boolean;
  notify: boolean;
  rings: boolean;
  notifyWaiting: boolean;
  notifyFinished: boolean;
}

const DEFAULTS: Prefs = { remaining: false, paceAware: true, menuBar: true, notify: true, rings: false, notifyWaiting: true, notifyFinished: true };

const fromBackend = (p: QuotaPrefs): Prefs => ({ remaining: p.remaining, paceAware: p.pace_aware, menuBar: p.menu_bar, notify: p.notify, rings: p.rings, notifyWaiting: p.notify_waiting, notifyFinished: p.notify_finished });
const toBackend = (p: Prefs): QuotaPrefs => ({ remaining: p.remaining, pace_aware: p.paceAware, menu_bar: p.menuBar, notify: p.notify, rings: p.rings, notify_waiting: p.notifyWaiting, notify_finished: p.notifyFinished });

const Ctx = createContext<{ prefs: Prefs; set: (p: Partial<Prefs>) => void }>({ prefs: DEFAULTS, set: () => {} });

export function QuotaPrefsProvider({ children }: { children: ReactNode }) {
  const [prefs, setPrefs] = useState<Prefs>(DEFAULTS);
  useEffect(() => {
    void api.getQuotaPrefs().then((p) => setPrefs(fromBackend(p))).catch(() => {});
  }, []);
  const value = useMemo(
    () => ({
      prefs,
      set: (p: Partial<Prefs>) => {
        const next = { ...prefs, ...p };
        setPrefs(next);
        void api.setQuotaPrefs(toBackend(next)).catch(() => {});
      },
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

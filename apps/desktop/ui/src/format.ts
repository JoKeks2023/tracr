import type { TokenCount } from "./types";

const clock = new Map<string, Intl.DateTimeFormat>();

/** 24-hour clock time in the reporting timezone. */
export function hhmm(ms: number, timeZone: string): string {
  let f = clock.get(timeZone);
  if (!f) {
    f = new Intl.DateTimeFormat("en-GB", { hour: "2-digit", minute: "2-digit", hour12: false, timeZone });
    clock.set(timeZone, f);
  }
  return f.format(ms);
}

/** "2026-09-28" → "Mon 28 Sep". */
export function dayLabel(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  const date = new Date(Date.UTC(y, m - 1, d));
  return new Intl.DateTimeFormat("en-GB", { weekday: "short", day: "numeric", month: "short", timeZone: "UTC" }).format(date);
}

export function duration(ms: number): string {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

export function compact(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(n >= 1e8 ? 0 : n >= 1e7 ? 1 : 2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(n >= 1e5 ? 0 : 1)}k`;
  return String(n);
}

/** A token sum; records that state no value are marked, never counted as zero. */
export function tokens(t: TokenCount): { text: string; unknown: number } {
  return { text: compact(t.sum), unknown: t.unknown_records };
}

export function shortId(id: string, n = 8): string {
  return id.slice(0, n);
}

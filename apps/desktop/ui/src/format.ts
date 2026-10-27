import { locale, t } from "./i18n";
import type { CostView, TokenCount } from "./types";

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
  return new Intl.DateTimeFormat(locale, { weekday: "short", day: "numeric", month: "short", timeZone: "UTC" }).format(date);
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

/** An amount in nano-units, e.g. "$12.34"; amounts under a cent as "<$0.01". */
export function money(nano: number, currency: string): string {
  const units = nano / 1e9;
  const sign = currency === "USD" ? "$" : "";
  const suffix = currency === "USD" ? "" : ` ${currency}`;
  if (units > 0 && units < 0.01) return `<${sign}0.01${suffix}`;
  const digits = units >= 1000 ? 0 : 2;
  return `${sign}${units.toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits })}${suffix}`;
}

/** An estimate as text: unpriced is never shown as zero. */
export function estimate(c: CostView, currency: string): string {
  if (c.amount_nano === null) {
    return c.unpriced_records + c.not_estimated_records > 0 ? t("unpricedWord") : "—";
  }
  const partial = c.partially_priced_records + c.unpriced_records + c.not_estimated_records > 0;
  return `${partial ? "≥ " : ""}${money(c.amount_nano, currency)}`;
}

/** Why part of an estimate is missing, in words. */
export function reasonText(r: string): string {
  const fixed: Record<string, string> = {
    cache_write_tier_not_reported: t("rCacheWriteTier"),
    service_tier_unknown: t("rServiceTier"),
    model_unknown: t("rModel"),
  };
  if (fixed[r]) return fixed[r];
  const [kind, arg] = r.split(/:(.*)/s);
  if (kind === "no_price_rule") return t("rNoPriceFor", { x: arg });
  if (kind === "category_not_reported") return t("rNotReported", { x: arg.replaceAll("_", " ") });
  if (kind === "no_price_for" || kind === "no_price_for_tool") return t("rNoPriceFor", { x: arg.replaceAll("_", " ") });
  if (kind === "cache_tier_unknown") return t("rCacheTier", { x: arg });
  if (kind === "speed_not_recorded") return t("rSpeed", { x: arg });
  if (kind === "inference_geo_not_recorded") return t("rGeo", { x: arg });
  return r.replaceAll("_", " ");
}

/** "/Users/ada/.claude/projects" → "~/.claude/projects" (the window never needs a user name). */
export function tildify(path: string, home: string | null): string {
  if (!home) return path;
  const h = home.replace(/\/+$/, "");
  return path === h ? "~" : path.startsWith(`${h}/`) ? `~${path.slice(h.length)}` : path;
}

/** A provider as a person names it. */
export function providerName(p: string): string {
  return ({ claude_code: "Claude Code", codex: "Codex" } as Record<string, string>)[p] ?? p.replaceAll("_", " ");
}

/** "1 interval" / "3 intervals" (the unclear windows of a day). */
export function intervals(n: number): string {
  return n === 1 ? t("intervalOne") : t("unclearHint", { n });
}

/** "09:12–10:40", or with the first day named when the session crossed midnight. */
export function spanLabel(first: number, last: number, tz: string): string {
  const day = (ms: number) => new Intl.DateTimeFormat("en-CA", { timeZone: tz }).format(ms);
  const range = `${hhmm(first, tz)}–${hhmm(last, tz)}`;
  if (day(first) === day(last)) return range;
  const short = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", timeZone: tz }).format(first);
  return `${short} ${range}`;
}

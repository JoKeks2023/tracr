import { locale } from "./i18n";

/** The places in the sidebar. Order is the sidebar's order. */
export type Area = "today" | "projects" | "worktime" | "limits";
export const AREAS: Area[] = ["today", "projects", "worktime", "limits"];

/** How wide a period the toolbar shows. Work time is always one day. */
export type Scope = "day" | "week" | "month";

const parse = (day: string): Date => {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d));
};
const fmt = (d: Date): string => d.toISOString().slice(0, 10);
const addDays = (d: Date, n: number): Date => new Date(d.getTime() + n * 86_400_000);

/** Today in the window's local calendar (the same notion the sidebar used for "Today"). */
export function isoToday(): string {
  return new Intl.DateTimeFormat("en-CA").format(new Date());
}

/** `[from, to]` (inclusive) of the period that contains `anchor`; weeks start on Monday. */
export function rangeOf(scope: Scope, anchor: string): [string, string] {
  const d = parse(anchor);
  if (scope === "day") return [anchor, anchor];
  if (scope === "week") {
    const monday = addDays(d, -((d.getUTCDay() + 6) % 7));
    return [fmt(monday), fmt(addDays(monday, 6))];
  }
  const first = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1));
  const last = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 0));
  return [fmt(first), fmt(last)];
}

/** The anchor one period earlier (`-1`) or later (`+1`). */
export function shiftAnchor(scope: Scope, anchor: string, dir: -1 | 1): string {
  const d = parse(anchor);
  if (scope === "day") return fmt(addDays(d, dir));
  if (scope === "week") return fmt(addDays(d, 7 * dir));
  return fmt(new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + dir, 1)));
}

/** "Mon 28 Sep", "28 Sep – 4 Oct" or "September 2026". */
export function periodLabel(scope: Scope, anchor: string): string {
  const f = (o: Intl.DateTimeFormatOptions, d: Date) => new Intl.DateTimeFormat(locale, { ...o, timeZone: "UTC" }).format(d);
  const [from, to] = rangeOf(scope, anchor);
  if (scope === "day") return f({ weekday: "short", day: "numeric", month: "short" }, parse(from));
  if (scope === "month") return f({ month: "long", year: "numeric" }, parse(from));
  const o = { day: "numeric", month: "short" } as const;
  return `${f(o, parse(from))} – ${f(o, parse(to))}`;
}

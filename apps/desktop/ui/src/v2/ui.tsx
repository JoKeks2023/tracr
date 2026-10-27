import type { ReactNode } from "react";
import { locale, t } from "../i18n";
import { providerName } from "../format";
import type { LimitWindow } from "../types";

export function Stat({ label, value, note, tone }: { label: string; value: string; note?: string; tone?: "warn" | "muted" }) {
  return (
    <div className="stat">
      <div className="lab">{label}</div>
      <div className={`val mono ${tone === "warn" ? "w" : tone === "muted" ? "m" : ""}`}>{value}</div>
      {note && <div className="vnote">{note}</div>}
    </div>
  );
}

export function Panel({ title, aside, children, label }: { title: string; aside?: ReactNode; children: ReactNode; label?: string }) {
  return (
    <section className="panel" aria-label={label ?? title}>
      <div className="ph">
        <h2>{title}</h2>
        {aside && <span>{aside}</span>}
      </div>
      {children}
    </section>
  );
}

/** "17:42" today, else "Thu 21:40". */
export function clockLabel(ms: number, tz?: string): string {
  const day = (x: number) => new Intl.DateTimeFormat("en-CA", { timeZone: tz }).format(x);
  const time = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", hour12: false, timeZone: tz }).format(ms);
  if (day(ms) === day(Date.now())) return time;
  return `${new Intl.DateTimeFormat(locale, { weekday: "short", timeZone: tz }).format(ms)} ${time}`;
}

export function windowName(w: LimitWindow): string {
  const span = w.window === "five_hour" ? t("wFive") : w.window === "weekly" ? t("wWeek") : t("wMinutes", { n: w.window_minutes ?? "?" });
  return `${providerName(w.provider)} · ${span}`;
}

export function toneOf(pct: number): "" | "warn" | "bad" {
  return pct >= 100 ? "bad" : pct >= 80 ? "warn" : "";
}

/** One provider-reported window: the percent, a bar, when it resets and when it was stated. */
export function QuotaBar({ w, big }: { w: LimitWindow; big?: boolean }) {
  const pct = Math.max(0, Math.min(100, w.used_percent));
  const tone = w.expired ? "" : toneOf(w.used_percent);
  const reached = !w.expired && w.used_percent >= 100;
  return (
    <div className="q">
      <div className="top">
        <span className="nm">{windowName(w)}</span>
        <span className={`mono ${big ? "qbig" : ""}`}>{Math.round(w.used_percent)} %</span>
      </div>
      <div className={`meter ${tone}`} role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100} aria-label={windowName(w)}>
        <i style={{ width: `${pct}%`, opacity: w.expired ? 0.4 : 1 }} />
      </div>
      <div className="vnote">
        {w.expired
          ? t("windowExpired")
          : reached
            ? `${t("limitReached")}${w.resets_at ? ` · ${t("resetsAt", { time: clockLabel(w.resets_at) })}` : ""}`
            : w.resets_at
              ? t("resetsAt", { time: clockLabel(w.resets_at) })
              : ""}{" "}
        · {t("statedAt", { time: clockLabel(w.observed_at) })}
      </div>
    </div>
  );
}

import type { ReactNode } from "react";
import { locale, t } from "../i18n";
import { providerName } from "../format";
import type { LimitWindow } from "../types";
import { forecastOf, levelOf, useNow, useQuotaPrefs } from "./quota";

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

/** "1h 12m" / "38m" for a span of milliseconds. */
export function spanText(ms: number): string {
  const m = Math.max(0, Math.round(ms / 60_000));
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  return h >= 48 ? `${Math.round(h / 24)}d` : `${h}h ${m % 60}m`;
}

/** One provider-reported window: the figure, a bar with a pace tick, when it resets, whether it lasts. */
export function QuotaBar({ w, big }: { w: LimitWindow; big?: boolean }) {
  const { prefs } = useQuotaPrefs();
  const now = useNow();
  const used = Math.max(0, Math.min(100, w.used_percent));
  const shown = prefs.remaining ? 100 - used : used;
  const level = levelOf(w, prefs.paceAware);
  const fc = forecastOf(w);
  const reached = !w.expired && used >= 100;
  const tone = level === "ok" ? "" : level === "warn" ? "warn" : "bad";
  const resets = w.resets_at && !w.expired ? t("resetsIn", { time: spanText(w.resets_at - now), at: clockLabel(w.resets_at) }) : "";
  const forecast = w.expired
    ? t("windowExpired")
    : reached
      ? t("limitReached")
      : fc
        ? fc.runsOutAt !== null
          ? t("runsOut", { in: spanText(fc.runsOutAt - now), before: spanText(w.resets_at! - fc.runsOutAt) })
          : t("lastsUntilReset")
        : "";
  const paceText = fc ? t(fc.pace === "hot" ? "paceHot" : fc.pace === "room" ? "paceRoom" : "paceOk") : null;
  return (
    <div className="q">
      <div className="top">
        <span className="nm">{windowName(w)}</span>
        <span className={`mono ${big ? "qbig" : ""}`} title={prefs.remaining ? t("quotaLeft") : t("quotaUsed")}>
          {w.expired ? "—" : `${Math.round(shown)} %`}
          {big && !w.expired && <small className="qwhat">{prefs.remaining ? t("quotaLeft") : t("quotaUsed")}</small>}
        </span>
      </div>
      <div className={`meter ${tone}`} role="progressbar" aria-valuenow={Math.round(used)} aria-valuemin={0} aria-valuemax={100} aria-label={`${windowName(w)}: ${Math.round(used)} % ${t("quotaUsed")}`}>
        <i style={{ width: w.expired ? "0%" : `${used}%` }} />
        {fc && <b className="tick" style={{ left: `${Math.min(100, fc.elapsed * 100)}%` }} title={t("paceTick")} />}
      </div>
      <div className="vnote qline">
        {resets && <span>{resets}</span>}
        {paceText && <span className={`pace ${fc!.pace}`}>{paceText}</span>}
      </div>
      {forecast && <div className="vnote">{forecast}</div>}
      <div className="vnote">{t("statedAt", { time: clockLabel(w.observed_at) })}</div>
    </div>
  );
}

/** One ring: the share used, coloured by level; the tightest window of a provider. */
function Ring({ w, others }: { w: LimitWindow; others: LimitWindow[] }) {
  const { prefs } = useQuotaPrefs();
  const used = Math.max(0, Math.min(100, w.used_percent));
  const shown = prefs.remaining ? 100 - used : used;
  const level = levelOf(w, prefs.paceAware);
  const color = level === "ok" ? "var(--acc)" : level === "warn" ? "var(--warn)" : "var(--bad)";
  const r = 34;
  const c = 2 * Math.PI * r;
  return (
    <div className="ringcard">
      <svg viewBox="0 0 80 80" width="88" height="88" role="img" aria-label={`${windowName(w)}: ${Math.round(used)} % ${t("quotaUsed")}`}>
        <circle cx="40" cy="40" r={r} fill="none" stroke="var(--track)" strokeWidth="7" />
        <circle cx="40" cy="40" r={r} fill="none" stroke={color} strokeWidth="7" strokeLinecap="round" strokeDasharray={`${(used / 100) * c} ${c}`} transform="rotate(-90 40 40)" opacity={w.expired ? 0.4 : 1} />
        <text x="40" y="45" textAnchor="middle" className="ringtext">{Math.round(shown)}%</text>
      </svg>
      <div className="ringname">{windowName(w)}</div>
      {w.resets_at && !w.expired && <div className="vnote">{t("resetsAt", { time: clockLabel(w.resets_at) })}</div>}
      {others.map((o) => (
        <div key={o.window} className="vnote">{windowName(o)}: {Math.round(prefs.remaining ? 100 - o.used_percent : o.used_percent)} %</div>
      ))}
    </div>
  );
}

/** Every provider as a ring on its tightest window, the other windows named under it. */
export function QuotaRings({ windows }: { windows: LimitWindow[] }) {
  const byProvider = new Map<string, LimitWindow[]>();
  for (const w of windows) byProvider.set(w.provider, [...(byProvider.get(w.provider) ?? []), w]);
  return (
    <div className="rings">
      {[...byProvider.values()].map((ws) => {
        const sorted = [...ws].sort((a, b) => Number(a.expired) - Number(b.expired) || b.used_percent - a.used_percent);
        return <Ring key={ws[0].provider} w={sorted[0]} others={sorted.slice(1)} />;
      })}
    </div>
  );
}

/** The view switch for quota: one bar per window, or one ring per provider. */
export function QuotaViewSwitch() {
  const { prefs, set } = useQuotaPrefs();
  return (
    <div className="seg" role="group" aria-label={t("quotaView")}>
      <button aria-pressed={!prefs.rings} onClick={() => set({ rings: false })}>{t("viewBars")}</button>
      <button aria-pressed={prefs.rings} onClick={() => set({ rings: true })}>{t("viewRings")}</button>
    </div>
  );
}

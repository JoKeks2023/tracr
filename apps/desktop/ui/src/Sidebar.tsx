import { dayLabel, tildify } from "./format";
import { t } from "./i18n";
import type { Status } from "./types";

function isoDay(offset: number): string {
  const d = new Date();
  d.setDate(d.getDate() + offset);
  return new Intl.DateTimeFormat("en-CA").format(d);
}

function label(day: string): string {
  if (day === isoDay(0)) return t("today");
  if (day === isoDay(-1)) return t("yesterday");
  return dayLabel(day);
}

function collectorLabel(c: Status["collector"]): string {
  return {
    collecting: t("collecting"),
    stalled: t("collectorStalled"),
    elsewhere: t("collectorElsewhere"),
    stopped: t("collectorStopped"),
    not_configured: t("collectorNone"),
  }[c];
}

/** Durable navigation: the days, then the recording state and settings at the foot. */
export function Sidebar({
  days,
  day,
  status,
  onSelect,
  onSettings,
}: {
  days: string[];
  day: string | null;
  status: Status | null;
  onSelect: (d: string) => void;
  onSettings: () => void;
}) {
  const rt = status?.runtime;
  const detail = status
    ? [
        status.collector_detail,
        rt?.last_error && `Last error: ${rt.last_error}`,
        status.data_dir && `Data: ${tildify(status.data_dir, status.home)}`,
      ]
        .filter(Boolean)
        .join("\n")
    : "";
  return (
    <nav className="sidebar" aria-label={t("daysTitle")}>
      <div className="sidebar-drag" data-tauri-drag-region />
      {days.length > 0 && <h2 className="sidebar-title">{t("daysTitle")}</h2>}
      <ul className="sidebar-days">
        {days.map((d) => (
          <li key={d}>
            <button className="side-item" aria-current={d === day ? "page" : undefined} onClick={() => onSelect(d)}>
              {label(d)}
            </button>
          </li>
        ))}
      </ul>
      <div className="sidebar-foot">
        {status && (
          <div className="side-status" title={detail}>
            <span className={`dot ${status.collector}`} aria-hidden />
            {collectorLabel(status.collector)}
          </div>
        )}
        <button className="side-item" onClick={onSettings} title={t("settingsHint")}>
          {t("settings")}
        </button>
      </div>
    </nav>
  );
}

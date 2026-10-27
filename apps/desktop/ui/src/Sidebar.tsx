import { tildify } from "./format";
import { AREAS, type Area } from "./nav";
import { t } from "./i18n";
import type { Status } from "./types";

function collectorLabel(c: Status["collector"]): string {
  return {
    collecting: t("collecting"),
    stalled: t("collectorStalled"),
    elsewhere: t("collectorElsewhere"),
    stopped: t("collectorStopped"),
    not_configured: t("collectorNone"),
  }[c];
}

const AREA_LABEL: Record<Area, "today" | "areaSessions" | "areaWorktime"> = {
  today: "today",
  sessions: "areaSessions",
  worktime: "areaWorktime",
};

/** Durable navigation: where to go, then the recording state and settings at the foot. */
export function Sidebar({
  area,
  status,
  onArea,
  onSettings,
}: {
  area: Area;
  status: Status | null;
  onArea: (a: Area) => void;
  onSettings: () => void;
}) {
  const rt = status?.runtime;
  const detail = status
    ? [
        status.collector_detail,
        rt?.last_error && t("lastError", { text: rt.last_error }),
        status.data_dir && t("dataDirectory", { path: tildify(status.data_dir, status.home) }),
      ]
        .filter(Boolean)
        .join("\n")
    : "";
  return (
    <nav className="sidebar" aria-label={t("navTitle")}>
      <div className="sidebar-drag" data-tauri-drag-region />
      <ul className="sidebar-days">
        {AREAS.map((a) => (
          <li key={a}>
            <button className="side-item" aria-current={a === area ? "page" : undefined} onClick={() => onArea(a)}>
              {t(AREA_LABEL[a])}
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

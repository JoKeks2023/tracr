import { t } from "../i18n";
import type { Area } from "../nav";
import { icons } from "./icons";

const ITEMS: { area: Area; icon: keyof typeof icons; label: "today" | "areaProjects" | "railWorktime" | "railLimits" }[] = [
  { area: "today", icon: "today", label: "today" },
  { area: "projects", icon: "projects", label: "areaProjects" },
  { area: "worktime", icon: "worktime", label: "railWorktime" },
  { area: "limits", icon: "limits", label: "railLimits" },
];

/** The areas, as an icon rail: durable navigation, always the same places. */
export function Rail({ area, alert, onArea, onSettings }: { area: Area; alert: Area | null; onArea: (a: Area) => void; onSettings: () => void }) {
  return (
    <nav className="rail" aria-label={t("railAria")}>
      <div className="rail-drag" data-tauri-drag-region />
      {ITEMS.map((i) => (
        <button key={i.area} className="nav" aria-current={area === i.area ? "page" : undefined} onClick={() => onArea(i.area)}>
          {icons[i.icon]}
          {t(i.label)}
          {alert === i.area && <span className="badge" aria-label="!" />}
        </button>
      ))}
      <div className="grow" />
      <button className="nav" onClick={onSettings} title={t("settingsHint")}>
        {icons.settings}
        {t("railSettings")}
      </button>
    </nav>
  );
}

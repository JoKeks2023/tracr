import type { TargetOption } from "./types";
import { t } from "./i18n";

/** Target dropdown (UI-01): repositories, categories, used before, "Other…". */
export function TargetSelect({ targets, value, onChange, other, onOther, allowNone, extras }: {
  targets: TargetOption[];
  value: string;
  onChange: (v: string) => void;
  other: string;
  onOther: (v: string) => void;
  allowNone?: boolean;
  /** Extra choices before the targets (e.g. "Break"). */
  extras?: { value: string; label: string }[];
}) {
  const groups: [TargetOption["group"], string][] = [
    ["project", t("targetProjects")],
    ["repository", t("targetRepos")],
    ["category", t("targetCategories")],
    ["other", t("targetUsed")],
  ];
  return (
    <>
      <select className="select" value={value} onChange={(e) => onChange(e.target.value)} aria-label={t("target")}>
        <option value="">{allowNone ? t("noTarget") : t("chooseTarget")}</option>
        {extras?.map((x) => (
          <option key={x.value} value={x.value}>
            {x.label}
          </option>
        ))}
        {groups.map(([g, label]) => {
          const items = targets.filter((o) => o.group === g);
          return items.length === 0 ? null : (
            <optgroup key={g} label={label}>
              {items.map((o) => (
                <option key={o.key} value={o.key}>
                  {o.label}
                </option>
              ))}
            </optgroup>
          );
        })}
        <option value="__other">{t("otherEllipsis")}</option>
      </select>
      {value === "__other" && (
        <input className="input" placeholder={t("shortLabel")} maxLength={80} value={other} onChange={(e) => onOther(e.target.value)} />
      )}
    </>
  );
}

/** The target key for a dropdown value ("" = none). */
export function targetKey(value: string, other: string): string {
  if (value === "__other") return other.trim() ? `other:${other.trim()}` : "";
  return value;
}

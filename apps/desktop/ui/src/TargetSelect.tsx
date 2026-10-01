import type { TargetOption } from "./types";

/** Target dropdown (UI-01): repositories, categories, used before, "Other…". */
export function TargetSelect({ targets, value, onChange, other, onOther, allowNone }: {
  targets: TargetOption[];
  value: string;
  onChange: (v: string) => void;
  other: string;
  onOther: (v: string) => void;
  allowNone?: boolean;
}) {
  const groups: [TargetOption["group"], string][] = [
    ["repository", "Repositories"],
    ["category", "Categories"],
    ["other", "Used before"],
  ];
  return (
    <>
      <select className="select" value={value} onChange={(e) => onChange(e.target.value)} aria-label="Target">
        <option value="">{allowNone ? "No target" : "Choose a target…"}</option>
        {groups.map(([g, label]) => {
          const items = targets.filter((t) => t.group === g);
          return items.length === 0 ? null : (
            <optgroup key={g} label={label}>
              {items.map((t) => (
                <option key={t.key} value={t.key}>
                  {t.label}
                </option>
              ))}
            </optgroup>
          );
        })}
        <option value="__other">Other…</option>
      </select>
      {value === "__other" && (
        <input className="input" placeholder="Short label" maxLength={80} value={other} onChange={(e) => onOther(e.target.value)} />
      )}
    </>
  );
}

/** The target key for a dropdown value ("" = none). */
export function targetKey(value: string, other: string): string {
  if (value === "__other") return other.trim() ? `other:${other.trim()}` : "";
  return value;
}

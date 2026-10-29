import { useEffect, useMemo, useRef, useState } from "react";
import { t } from "./i18n";
import { useSheet } from "./useSheet";

export interface Command {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

/** ⌘K: type to go somewhere or do something. Arrow keys choose, Return runs, Escape closes. */
export function Palette({ commands, onClose }: { commands: Command[]; onClose: () => void }) {
  const ref = useSheet<HTMLDivElement>();
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return commands.filter((c) => !needle || c.label.toLowerCase().includes(needle));
  }, [commands, q]);
  useEffect(() => setAt(0), [q]);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") return onClose();
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (shown.length) setAt((i) => (i + (e.key === "ArrowDown" ? 1 : shown.length - 1)) % shown.length);
    }
    if (e.key === "Enter" && shown[at]) {
      onClose();
      shown[at].run();
    }
  };
  return (
    <>
      <div className="scrim" onClick={onClose} />
      <div ref={ref} className="palette" role="dialog" aria-modal="true" aria-label={t("paletteTitle")} onKeyDown={onKey}>
        <input
          ref={input}
          className="palette-input"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={shown[at] ? `cmd-${shown[at].id}` : undefined}
          placeholder={t("palettePlaceholder")}
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <ul id="palette-list" role="listbox" className="palette-list">
          {shown.length === 0 && <li className="palette-none muted">{t("paletteNone")}</li>}
          {shown.map((c, i) => (
            <li
              key={c.id}
              id={`cmd-${c.id}`}
              role="option"
              aria-selected={i === at}
              className="palette-item"
              onMouseMove={() => setAt(i)}
              onClick={() => {
                onClose();
                c.run();
              }}
            >
              <span>{c.label}</span>
              {c.hint && <span className="muted mono">{c.hint}</span>}
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}

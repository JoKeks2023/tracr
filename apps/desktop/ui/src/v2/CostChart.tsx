import { useEffect, useMemo, useRef, useState } from "react";
import { dayLabel } from "../format";
import { t } from "../i18n";
import type { CostDay } from "../types";

/** Model → family and colour: the chart reads by family (a handful of colours), the table by model. */
export function familyOf(model: string): { key: string; label: string; color: string } {
  if (model.includes("opus")) return { key: "opus", label: "Opus", color: "var(--purple)" };
  if (model.includes("sonnet")) return { key: "sonnet", label: "Sonnet", color: "var(--info)" };
  if (model.includes("haiku")) return { key: "haiku", label: "Haiku", color: "var(--teal)" };
  if (model.startsWith("gpt") || model.includes("codex")) return { key: "gpt", label: "GPT", color: "var(--warn)" };
  return { key: "other", label: t("catOther"), color: "var(--dim)" };
}

const FAMILIES = ["opus", "sonnet", "haiku", "gpt", "other"];

/** Stacked cost per day by model family (estimates; unpriced time has no bar, never a zero-cost claim). */
export function CostChart({ days, currency }: { days: CostDay[]; currency: string }) {
  const sign = currency === "USD" ? "$" : "";
  const data = useMemo(
    () =>
      days.map((d) => {
        const by = new Map<string, number>();
        let partial = false;
        for (const m of d.models) {
          const f = familyOf(m.model);
          by.set(f.key, (by.get(f.key) ?? 0) + (m.amount_nano ?? 0) / 1e9);
          partial ||= m.partial;
        }
        return { day: d.day, by, total: [...by.values()].reduce((a, b) => a + b, 0), partial };
      }),
    [days],
  );
  const max = Math.max(1, ...data.map((d) => d.total));
  const nice = (() => {
    const mag = 10 ** Math.floor(Math.log10(max));
    const n = max / mag;
    return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * mag;
  })();
  // The chart is drawn at the width it has, so text and bars keep their size.
  const box = useRef<HTMLElement>(null);
  const [w, setW] = useState(800);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setW(Math.max(320, Math.round(el.clientWidth))));
    ro.observe(el);
    setW(Math.max(320, Math.round(el.clientWidth)));
    return () => ro.disconnect();
  }, []);
  const h = 220;
  const left = 44;
  const bottom = 22;
  const plotH = h - bottom - 6;
  const band = (w - left) / Math.max(1, data.length);
  const bw = Math.min(40, band * 0.7);
  const peak = data.reduce((a, b) => (b.total > a.total ? b : a), data[0] ?? { day: "", total: 0 });
  const labelEvery = Math.ceil(data.length / 14);
  return (
    <figure ref={box} className="costchart" aria-label={t("costChartAria", { days: data.length, peak: `${sign}${peak.total.toFixed(0)}`, day: peak.day ? dayLabel(peak.day) : "–" })}>
      <svg viewBox={`0 0 ${w} ${h}`} width={w} height={h} aria-hidden="true">
        {[0, 0.5, 1].map((f) => {
          const y = 6 + plotH - f * plotH;
          return (
            <g key={f}>
              <line x1={left} x2={w} y1={y} y2={y} stroke="var(--line)" />
              <text x={0} y={y + 4} className="chartlab">{sign}{Math.round(nice * f)}</text>
            </g>
          );
        })}
        {data.map((d, i) => {
          let y = 6 + plotH;
          const x = left + i * band + (band - bw) / 2;
          return (
            <g key={d.day}>
              {FAMILIES.filter((f) => (d.by.get(f) ?? 0) > 0).map((f) => {
                const hh = ((d.by.get(f) ?? 0) / nice) * plotH;
                y -= hh;
                const fam = familyOf(f === "gpt" ? "gpt" : f === "other" ? "x" : f);
                return (
                  <rect key={f} x={x} y={y} width={bw} height={Math.max(1, hh)} fill={fam.color}>
                    <title>{`${dayLabel(d.day)} · ${fam.label} · ${sign}${(d.by.get(f) ?? 0).toFixed(2)}${d.partial ? " (≥)" : ""}`}</title>
                  </rect>
                );
              })}
              {i % labelEvery === 0 && (
                <text x={x + bw / 2} y={h - 6} textAnchor="middle" className="chartlab">{dayLabel(d.day).split(" ").slice(0, 2).join(" ")}</text>
              )}
            </g>
          );
        })}
      </svg>
      <figcaption className="vlegend">
        {FAMILIES.map((f) => {
          const fam = familyOf(f === "gpt" ? "gpt" : f === "other" ? "x" : f);
          return (
            <span key={f}>
              <i className="sw" style={{ background: fam.color }} />
              {fam.label}
            </span>
          );
        })}
      </figcaption>
    </figure>
  );
}

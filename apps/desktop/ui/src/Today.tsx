import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import { t } from "./i18n";
import { Attention, BigNumber, CostLine } from "./Overview";
import { SessionRow } from "./SessionRow";
import { Timeline } from "./Timeline";
import type { DayAgents } from "./types";

/** What is going on today: the number, what needs you, the latest sessions. */
export function Today({
  day,
  hasData,
  reloadKey,
  onOpenSession,
  onOpenWorktime,
  onOpenSessions,
  onError,
}: {
  day: string;
  hasData: boolean;
  reloadKey: unknown;
  onOpenSession: (id: string) => void;
  onOpenWorktime: () => void;
  onOpenSessions: () => void;
  onError: (text: string) => void;
}) {
  const [view, setView] = useState<DayAgents | null>(null);
  useEffect(() => {
    if (!hasData) {
      setView(null);
      return;
    }
    let live = true;
    api
      .dayAgents(day)
      .then((v) => live && setView(v))
      .catch((e) => live && onError(errorText(e)));
    return () => {
      live = false;
    };
  }, [day, hasData, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!hasData || !view) {
    return (
      <div className="empty">
        <h2>{t("nothingToday")}</h2>
        <p className="muted">{t("nothingTodayHint")}</p>
      </div>
    );
  }
  const human = view.review?.snapshot
    ? { ...view.human, counted_ms: view.review.snapshot.counted_ms, unknown_ms: view.review.snapshot.unknown_ms, unknown_by_reason: {} }
    : view.human;
  const unknownWindows = view.human.atoms.filter((a) => a.status === "unknown").length;
  const latest = [...view.sessions].sort((a, b) => (b.last_evidence_at ?? 0) - (a.last_evidence_at ?? 0)).slice(0, 5);
  return (
    <div className="main-card">
      <BigNumber human={human} canReview={false} onReview={() => {}} state={view.review ? t("reviewed") : t("provisional")} />
      <Attention human={human} windows={unknownWindows} onResolve={onOpenWorktime} />
      <section>
        <div className="section-head">
          <h2 className="section-title">{t("recentSessions")}</h2>
          <button className="button ghost small" onClick={onOpenSessions}>
            {t("openSessions")}
          </button>
        </div>
        <div className="session-group">
          {latest.map((s) => (
            <SessionRow key={s.session_id} s={s} pricing={view.cost.pricing} selected={false} onOpen={() => onOpenSession(s.session_id)} />
          ))}
        </div>
      </section>
      <Timeline day={view} selected={null} onSelect={onOpenSession} onAtom={onOpenWorktime} />
      <CostLine cost={view.cost} />
    </div>
  );
}

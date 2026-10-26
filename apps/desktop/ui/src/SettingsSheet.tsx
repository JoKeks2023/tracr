import { useEffect, useMemo, useState } from "react";
import { useSheet } from "./useSheet";
import { api, errorText, type RootKind } from "./api";
import { tildify } from "./format";
import { t } from "./i18n";
import { ProjectsSection } from "./ProjectsSection";
import { RootsPicker } from "./RootsPicker";
import type { Status } from "./types";

function collectorText(c: Status["collector"]): string {
  return {
    collecting: t("collecting"),
    stalled: t("collectorStalled"),
    elsewhere: t("collectorElsewhere"),
    stopped: t("collectorStopped"),
    not_configured: t("collectorNone"),
  }[c];
}

function RootList({
  kind,
  roots,
  home,
  busy,
  empty,
  confirm,
  onAsk,
  onCancel,
  onRemove,
}: {
  kind: RootKind;
  roots: string[];
  home: string | null;
  busy: boolean;
  empty: string;
  confirm: { kind: RootKind; path: string } | null;
  onAsk: (kind: RootKind, path: string) => void;
  onCancel: () => void;
  onRemove: (kind: RootKind, path: string) => void;
}) {
  if (roots.length === 0) return <p className="muted">{empty}</p>;
  return (
    <ul className="root-list">
      {roots.map((r) => {
        const asking = confirm?.kind === kind && confirm.path === r;
        return (
          <li key={r} className={asking ? "asking" : undefined}>
            <span className="mono" title={r}>
              {tildify(r, home)}
            </span>
            {asking ? (
              <div className="ask">
                <p>{t("stopReadingAsk")}</p>
                <div className="actions left">
                  <button className="button primary small" disabled={busy} onClick={() => onRemove(kind, r)}>
                    {t("stopReading")}
                  </button>
                  <button className="button ghost small" disabled={busy} onClick={onCancel}>
                    {t("cancel")}
                  </button>
                </div>
              </div>
            ) : (
              <button className="button ghost small" disabled={busy} onClick={() => onAsk(kind, r)} aria-label={`${t("stopReading")}: ${r}`}>
                {t("remove")}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** Settings: what tracr may read, projects, time zone, where its data lives, and which version this is. */
export function SettingsSheet({
  status,
  onClose,
  onChanged,
}: {
  status: Status;
  onClose: () => void;
  onChanged: (text: string, error?: boolean) => void;
}) {
  const sheetRef = useSheet<HTMLElement>();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<{ kind: RootKind; path: string } | null>(null);
  const proposed = status.proposed_roots.filter((r) => !r.approved).map((r) => r.path);

  const run = async (work: () => Promise<string | null>) => {
    setBusy(true);
    try {
      const text = await work();
      if (text) onChanged(text);
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  const choose = (kind: RootKind) =>
    run(async () => {
      const path = await api.chooseRoot(kind);
      return path ? t("approvedFolder", { path: tildify(path, status.home) }) : null;
    });
  const approve = (paths: string[]) =>
    run(async () => {
      await api.approveRoots(paths);
      return t("approvedFolders", { n: paths.length });
    });
  const zones = useMemo(() => {
    try {
      return Intl.supportedValuesOf("timeZone");
    } catch {
      return status.timezone ? [status.timezone] : [];
    }
  }, [status.timezone]);
  const chooseZone = (value: string) =>
    run(async () => {
      await api.setTimezone(value === "" ? null : value);
      return value === "" ? t("zoneSystem", { zone: status.system_timezone ?? "" }) : t("zoneSet", { zone: value });
    });
  const remove = (kind: RootKind, path: string) =>
    run(async () => {
      await api.removeRoot(kind, path);
      setConfirm(null);
      return t("noLongerReading", { path: tildify(path, status.home) });
    });
  const list = (kind: RootKind, roots: string[], empty: string) => (
    <RootList kind={kind} roots={roots} home={status.home} busy={busy} empty={empty} confirm={confirm} onAsk={(k, p) => setConfirm({ kind: k, path: p })} onCancel={() => setConfirm(null)} onRemove={remove} />
  );

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside ref={sheetRef} aria-modal="true" className="sheet settings-sheet" role="dialog" aria-label={t("settings")}>
        <header>
          <div style={{ flex: 1 }}>
            <h2>{t("settings")}</h2>
            <div className="muted">tracr {status.app_version}</div>
          </div>
          <button className="button ghost" onClick={onClose}>
            {t("done")}
          </button>
        </header>
        <div className="body">
          <section>
            <h3>{t("agentHistory")}</h3>
            <p className="note">{t("agentHistoryNote")}</p>
            {list("provider", status.approved_roots, t("noFolder"))}
            {proposed.length > 0 && (
              <>
                <h3 className="sub">{t("foundOnMac")}</h3>
                <RootsPicker paths={proposed} home={status.home} busy={busy} onApprove={approve} />
              </>
            )}
            <div className="actions left">
              <button className="button secondary" disabled={busy} onClick={() => choose("provider")}>
                {t("addFolder")}
              </button>
            </div>
          </section>

          <section>
            <h3>{t("repositories")}</h3>
            <p className="note">{t("repositoriesNote")}</p>
            {list("repository", status.approved_repository_roots, t("noRepoFolder"))}
            <div className="actions left">
              <button className="button secondary" disabled={busy} onClick={() => choose("repository")}>
                {t("addFolder")}
              </button>
            </div>
          </section>

          <ProjectsSection onChanged={onChanged} />

          <section>
            <h3>{t("timeZone")}</h3>
            <p className="note">{t("timeZoneNote")}</p>
            <label className="field">
              <span>{t("reportingZone")}</span>
              <select
                className="select"
                value={status.timezone_chosen ? (status.timezone ?? "") : ""}
                disabled={busy || status.reviewed_days > 0}
                onChange={(e) => chooseZone(e.target.value)}
              >
                <option value="">
                  {t("system")}
                  {status.system_timezone ? ` (${status.system_timezone})` : ""}
                </option>
                {zones.map((z) => (
                  <option key={z} value={z}>
                    {z}
                  </option>
                ))}
              </select>
            </label>
            <p className="note">
              {status.reviewed_days === 1 ? t("timeZoneLockedOne") : status.reviewed_days > 1 ? t("timeZoneLockedMany", { n: status.reviewed_days }) : t("timeZoneChange")}
            </p>
          </section>

          <section>
            <h3>{t("collector")}</h3>
            <dl className="kv">
              <dt>{t("state")}</dt>
              <dd>{collectorText(status.collector)}</dd>
              {status.collector_detail && (
                <>
                  <dt>{t("detail")}</dt>
                  <dd>{status.collector_detail}</dd>
                </>
              )}
              {status.runtime && (
                <>
                  <dt>{t("deviceSensor")}</dt>
                  <dd>{status.runtime.sensor}</dd>
                </>
              )}
            </dl>
          </section>

          <section>
            <h3>{t("data")}</h3>
            <dl className="kv">
              <dt>{t("location")}</dt>
              <dd className="mono">{status.data_dir ? tildify(status.data_dir, status.home) : "–"}</dd>
              <dt>{t("ledger")}</dt>
              <dd>{status.ledger_present ? t("ledgerPresent") : t("ledgerMissing")}</dd>
              {status.data_dir_from_env && (
                <>
                  <dt>{t("note")}</dt>
                  <dd>{t("setByEnv")}</dd>
                </>
              )}
            </dl>
          </section>

          <section>
            <h3>{t("about")}</h3>
            <dl className="kv">
              <dt>{t("version")}</dt>
              <dd>{status.app_version}</dd>
              <dt>{t("license")}</dt>
              <dd>AGPL-3.0-or-later</dd>
              <dt>{t("website")}</dt>
              <dd className="mono">tracr.jorisconrad.com</dd>
              <dt>{t("privacy")}</dt>
              <dd>{t("privacyText")}</dd>
            </dl>
          </section>
        </div>
      </aside>
    </>
  );
}

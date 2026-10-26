import { useMemo, useState } from "react";
import { api, errorText, type RootKind } from "./api";
import { tildify } from "./format";
import { RootsPicker } from "./RootsPicker";
import type { Status } from "./types";

const COLLECTOR_TEXT: Record<Status["collector"], string> = {
  collecting: "Collecting",
  stalled: "Not responding",
  elsewhere: "Collecting in another tracr process",
  stopped: "Not collecting",
  not_configured: "No approved history yet",
};

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
                <p>Stop reading this folder? What was imported stays in your ledger; new sessions are no longer picked up.</p>
                <div className="actions left">
                  <button className="button primary small" disabled={busy} onClick={() => onRemove(kind, r)}>
                    Stop reading
                  </button>
                  <button className="button ghost small" disabled={busy} onClick={onCancel}>
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <button className="button ghost small" disabled={busy} onClick={() => onAsk(kind, r)} aria-label={`Stop reading ${r}`}>
                Remove
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** Settings: what tracr may read, where its data lives, and which version this is. */
export function SettingsSheet({
  status,
  onClose,
  onChanged,
}: {
  status: Status;
  onClose: () => void;
  onChanged: (text: string, error?: boolean) => void;
}) {
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
      return path ? `Approved ${tildify(path, status.home)}` : null;
    });
  const approve = (paths: string[]) =>
    run(async () => {
      await api.approveRoots(paths);
      return `Approved ${paths.length} folder${paths.length === 1 ? "" : "s"}`;
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
      return value === "" ? `Reporting zone follows the system${status.system_timezone ? ` (${status.system_timezone})` : ""}` : `Reporting zone set to ${value}`;
    });
  const remove = (kind: RootKind, path: string) =>
    run(async () => {
      await api.removeRoot(kind, path);
      setConfirm(null);
      return `No longer reading ${tildify(path, status.home)}`;
    });

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet settings-sheet" role="dialog" aria-label="Settings">
        <header>
          <div style={{ flex: 1 }}>
            <h2>Settings</h2>
            <div className="muted">tracr {status.app_version}</div>
          </div>
          <button className="button ghost" onClick={onClose}>
            Done
          </button>
        </header>
        <div className="body">
          <section>
            <h3>Agent history</h3>
            <p className="note">
              tracr reads session files from these folders and keeps usage, timing and structure only, never prompts or tool
              output. Nothing is read from a folder you have not approved.
            </p>
            <RootList kind="provider" roots={status.approved_roots} home={status.home} busy={busy} empty="No folder approved yet." confirm={confirm} onAsk={(k, p) => setConfirm({ kind: k, path: p })} onCancel={() => setConfirm(null)} onRemove={remove} />
            {proposed.length > 0 && (
              <>
                <h3 className="sub">Found on this Mac</h3>
                <RootsPicker paths={proposed} home={status.home} busy={busy} onApprove={approve} />
              </>
            )}
            <div className="actions left">
              <button className="button secondary" disabled={busy} onClick={() => choose("provider")}>
                Add folder…
              </button>
            </div>
          </section>

          <section>
            <h3>Repositories</h3>
            <p className="note">
              tracr looks at git (branch and remote, read-only) below these folders to say which project your time belongs to.
            </p>
            <RootList kind="repository" roots={status.approved_repository_roots} home={status.home} busy={busy} empty="No repository folder approved yet." confirm={confirm} onAsk={(k, p) => setConfirm({ kind: k, path: p })} onCancel={() => setConfirm(null)} onRemove={remove} />
            <div className="actions left">
              <button className="button secondary" disabled={busy} onClick={() => choose("repository")}>
                Add folder…
              </button>
            </div>
          </section>

          <section>
            <h3>Time zone</h3>
            <p className="note">
              Your days are cut at midnight in this zone. It stays the same wherever you are, so a day always means the same
              hours.
            </p>
            <label className="field">
              <span>Reporting zone</span>
              <select
                className="select"
                value={status.timezone_chosen ? (status.timezone ?? "") : ""}
                disabled={busy || status.reviewed_days > 0}
                onChange={(e) => chooseZone(e.target.value)}
              >
                <option value="">System{status.system_timezone ? ` (${status.system_timezone})` : ""}</option>
                {zones.map((z) => (
                  <option key={z} value={z}>
                    {z}
                  </option>
                ))}
              </select>
            </label>
            {status.reviewed_days > 0 ? (
              <p className="note">
                {status.reviewed_days} reviewed day{status.reviewed_days === 1 ? "" : "s"} keep their zone, so it cannot be changed
                while any day is reviewed.
              </p>
            ) : (
              <p className="note">Changing it moves the day boundaries and rebuilds the days; nothing imported is lost.</p>
            )}
          </section>

          <section>
            <h3>Collector</h3>
            <dl className="kv">
              <dt>State</dt>
              <dd>{COLLECTOR_TEXT[status.collector]}</dd>
              {status.collector_detail && (
                <>
                  <dt>Detail</dt>
                  <dd>{status.collector_detail}</dd>
                </>
              )}
              {status.runtime && (
                <>
                  <dt>Device sensor</dt>
                  <dd>{status.runtime.sensor}</dd>
                </>
              )}
            </dl>
          </section>

          <section>
            <h3>Data</h3>
            <dl className="kv">
              <dt>Location</dt>
              <dd className="mono">{status.data_dir ? tildify(status.data_dir, status.home) : "none"}</dd>
              <dt>Ledger</dt>
              <dd>{status.ledger_present ? "present, append-only" : "not created yet"}</dd>
              {status.data_dir_from_env && (
                <>
                  <dt>Note</dt>
                  <dd>Set by TRACR_DATA_DIR</dd>
                </>
              )}
            </dl>
          </section>

          <section>
            <h3>About</h3>
            <dl className="kv">
              <dt>Version</dt>
              <dd>{status.app_version}</dd>
              <dt>License</dt>
              <dd>AGPL-3.0-or-later</dd>
              <dt>Website</dt>
              <dd className="mono">tracr.jorisconrad.com</dd>
              <dt>Privacy</dt>
              <dd>Local only. Nothing leaves this Mac.</dd>
            </dl>
          </section>
        </div>
      </aside>
    </>
  );
}

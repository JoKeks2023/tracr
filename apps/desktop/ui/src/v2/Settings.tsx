import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, errorText, type RootKind } from "../api";
import { tildify } from "../format";
import { t } from "../i18n";
import { RootsPicker } from "../RootsPicker";
import type { CostAlertPrefs, HooksView, Limits, ProjectOp, ProjectsView, Status } from "../types";
import { useQuotaPrefs } from "./quota";
import { Panel } from "./ui";

type Pane = "sources" | "projects" | "time" | "quota" | "notices" | "recording" | "about";
const PANES: { id: Pane; label: "stSources" | "projects" | "timeZone" | "quotaTitle" | "stNotices" | "stRecording" | "about" }[] = [
  { id: "sources", label: "stSources" },
  { id: "projects", label: "projects" },
  { id: "time", label: "timeZone" },
  { id: "quota", label: "quotaTitle" },
  { id: "notices", label: "stNotices" },
  { id: "recording", label: "stRecording" },
  { id: "about", label: "about" },
];

function collectorText(c: Status["collector"]): string {
  return {
    collecting: t("collecting"),
    stalled: t("collectorStalled"),
    elsewhere: t("collectorElsewhere"),
    stopped: t("collectorStopped"),
    not_configured: t("collectorNone"),
  }[c];
}

/** A labelled row: what it is on the left, the control on the right. */
function Row({ label, note, children }: { label: string; note?: string; children?: ReactNode }) {
  return (
    <div className="srow">
      <div className="lbl">
        <div>{label}</div>
        {note && <div className="vnote">{note}</div>}
      </div>
      <div className="ctl">{children}</div>
    </div>
  );
}

/** Folders tracr may read. Stopping needs a second click that says what it means. */
function Roots({
  kind,
  roots,
  home,
  busy,
  empty,
  asking,
  onAsk,
  onRemove,
}: {
  kind: RootKind;
  roots: string[];
  home: string | null;
  busy: boolean;
  empty: string;
  asking: { kind: RootKind; path: string } | null;
  onAsk: (a: { kind: RootKind; path: string } | null) => void;
  onRemove: (kind: RootKind, path: string) => void;
}) {
  if (roots.length === 0) return <p className="vnote">{empty}</p>;
  return (
    <ul className="rlist">
      {roots.map((r) => {
        const open = asking?.kind === kind && asking.path === r;
        return (
          <li key={r} className={open ? "asking" : undefined}>
            <span className="mono path" title={r}>{tildify(r, home)}</span>
            {open ? (
              <div className="ask">
                <p>{t("stopReadingAsk")}</p>
                <div className="acts">
                  <button className="btn s" disabled={busy} onClick={() => onRemove(kind, r)}>{t("stopReading")}</button>
                  <button className="btn s" disabled={busy} onClick={() => onAsk(null)}>{t("cancel")}</button>
                </div>
              </div>
            ) : (
              <button className="btn s" disabled={busy} onClick={() => onAsk({ kind, path: r })} aria-label={`${t("stopReading")}: ${r}`}>
                {t("remove")}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** Projects: a name you choose, and which repository belongs to which. Nothing here moves a tracked minute. */
function ProjectsPane({ onChanged }: { onChanged: (text: string, error?: boolean) => void }) {
  const [view, setView] = useState<ProjectsView | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const notify = useRef(onChanged);
  notify.current = onChanged;
  const load = useCallback(() => {
    void api.projects().then(setView).catch((e) => notify.current(errorText(e), true));
  }, []);
  useEffect(load, [load]);

  const run = async (op: ProjectOp, done: string) => {
    setBusy(true);
    try {
      await api.projectOp(op);
      setRenaming(null);
      setDeleting(null);
      load();
      onChanged(done);
    } catch (e) {
      onChanged(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  if (!view) return null;
  const nameOf = (id: string) => view.projects.find((p) => p.id === id)?.name ?? "";
  return (
    <>
      <Panel title={t("projects")}>
        <p className="vnote">{t("projectsNote")}</p>
        {view.projects.length > 0 && (
          <ul className="rlist">
            {view.projects.map((p) => (
              <li key={p.id} className={deleting === p.id ? "asking" : undefined}>
                {renaming?.id === p.id ? (
                  <>
                    <input className="sinput" value={renaming.name} maxLength={80} aria-label="Project name" onChange={(e) => setRenaming({ id: p.id, name: e.target.value })} />
                    <span className="acts">
                      <button className="btn s" disabled={busy || renaming.name.trim() === ""} onClick={() => run({ op: "rename", id: p.id, name: renaming.name }, `${t("rename")}: ${renaming.name.trim()}`)}>{t("save")}</button>
                      <button className="btn s" disabled={busy} onClick={() => setRenaming(null)}>{t("cancel")}</button>
                    </span>
                  </>
                ) : deleting === p.id ? (
                  <div className="ask">
                    <b>{p.name}</b>
                    <p>{t("deleteProjectAsk")}</p>
                    <div className="acts">
                      <button className="btn s" disabled={busy} onClick={() => run({ op: "delete", id: p.id }, `${t("delete")}: ${p.name}`)}>{t("deleteProject")}</button>
                      <button className="btn s" disabled={busy} onClick={() => setDeleting(null)}>{t("cancel")}</button>
                    </div>
                  </div>
                ) : (
                  <>
                    <span className="path">{p.name}</span>
                    <span className="acts">
                      <button className="btn s" disabled={busy} onClick={() => setRenaming({ id: p.id, name: p.name })}>{t("rename")}</button>
                      <button className="btn s" disabled={busy} onClick={() => setDeleting(p.id)}>{t("delete")}</button>
                    </span>
                  </>
                )}
              </li>
            ))}
          </ul>
        )}
        <form
          className="sform"
          onSubmit={(e) => {
            e.preventDefault();
            const n = name.trim();
            if (n) void run({ op: "create", name: n }, `${t("create")}: ${n}`).then(() => setName(""));
          }}
        >
          <input className="sinput" placeholder={t("newProjectName")} maxLength={80} value={name} onChange={(e) => setName(e.target.value)} aria-label={t("newProjectName")} />
          <button className="btn" type="submit" disabled={busy || name.trim() === ""}>{t("create")}</button>
        </form>
      </Panel>
      {view.projects.length > 0 && (
        <Panel title={t("repositories")}>
          {view.repositories.length === 0 ? (
            <p className="vnote">{t("noRepoSeen")}</p>
          ) : (
            view.repositories.map((r) => {
              const current = view.repository_projects[r.key] ?? "";
              return (
                <Row key={r.key} label={r.label}>
                  <select
                    className="ssel"
                    value={nameOf(current) ? current : ""}
                    disabled={busy}
                    aria-label={`Project of ${r.label}`}
                    onChange={(e) =>
                      void run(
                        { op: "map", repository: r.key, project: e.target.value === "" ? null : e.target.value },
                        e.target.value === "" ? `${r.label}: ${t("noProject")}` : `${r.label}: ${nameOf(e.target.value)}`,
                      )
                    }
                  >
                    <option value="">{t("noProject")}</option>
                    {view.projects.map((p) => (
                      <option key={p.id} value={p.id}>{p.name}</option>
                    ))}
                  </select>
                </Row>
              );
            })
          )}
        </Panel>
      )}
    </>
  );
}

const SESSION_STEPS = [0, 5, 10, 25, 50, 100, 250];
const WINDOW_STEPS = [0, 5, 10, 15, 25, 50, 100];
const WINDOW_MINUTES = [5, 15, 30, 60, 120, 240];
const DAY_STEPS = [0, 10, 25, 50, 100, 250, 500];
const MONTH_STEPS = [0, 100, 250, 500, 1000, 2500];

/** Cost alerts: one dropdown per rule, thresholds as presets. An alert fires when a figure crosses its threshold and at each doubling. */
function CostAlerts({ onChanged }: { onChanged: (text: string, error?: boolean) => void }) {
  const [p, setP] = useState<CostAlertPrefs | null>(null);
  useEffect(() => {
    void api.getCostAlerts().then(setP).catch((e) => onChanged(errorText(e), true));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  if (!p) return null;
  const set = (patch: Partial<CostAlertPrefs>) => {
    const next = { ...p, ...patch };
    setP(next);
    void api.setCostAlerts(next).catch((e) => onChanged(errorText(e), true));
  };
  const pick = (label: string, note: string | undefined, value: number, steps: number[], key: keyof CostAlertPrefs) => (
    <Row label={label} note={note}>
      <select className="ssel" value={value} disabled={!p.enabled} aria-label={label} onChange={(e) => set({ [key]: Number(e.target.value) } as Partial<CostAlertPrefs>)}>
        {steps.map((v) => (
          <option key={v} value={v}>{v === 0 ? t("off") : `$${v}`}</option>
        ))}
      </select>
    </Row>
  );
  return (
    <Panel title={t("costAlertsTitle")}>
      <Row label={t("costAlertsOn")} note={t("costAlertsNote")}>
        <div className="seg" role="group" aria-label={t("costAlertsOn")}>
          <button aria-pressed={p.enabled} onClick={() => set({ enabled: true })}>{t("on")}</button>
          <button aria-pressed={!p.enabled} onClick={() => set({ enabled: false })}>{t("off")}</button>
        </div>
      </Row>
      {pick(t("costAlertSession"), t("costAlertSessionNote"), p.session, SESSION_STEPS, "session")}
      {pick(t("costAlertWindow"), t("costAlertWindowNote"), p.window, WINDOW_STEPS, "window")}
      <Row label={t("costAlertWindowLen")}>
        <select className="ssel" value={p.window_minutes} disabled={!p.enabled} aria-label={t("costAlertWindowLen")} onChange={(e) => set({ window_minutes: Number(e.target.value) })}>
          {WINDOW_MINUTES.map((m) => (
            <option key={m} value={m}>{m < 60 ? `${m} min` : `${m / 60} h`}</option>
          ))}
        </select>
      </Row>
      {pick(t("costAlertDay"), undefined, p.day, DAY_STEPS, "day")}
      {pick(t("costAlertMonth"), undefined, p.month, MONTH_STEPS, "month")}
    </Panel>
  );
}

/** Settings: what tracr may read, projects, time zone, the Claude quota switch, recording and data, about. */
export function Settings({
  status,
  limits,
  onTap,
  onChanged,
}: {
  status: Status;
  limits: Limits | null;
  onTap: (install: boolean) => Promise<void>;
  onChanged: (text: string, error?: boolean) => void;
}) {
  const [pane, setPane] = useState<Pane>("sources");
  const [busy, setBusy] = useState(false);
  const [asking, setAsking] = useState<{ kind: RootKind; path: string } | null>(null);
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
  const remove = (kind: RootKind, path: string) =>
    run(async () => {
      await api.removeRoot(kind, path);
      setAsking(null);
      return t("noLongerReading", { path: tildify(path, status.home) });
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
  const tap = limits?.claude_tap;
  const { prefs, set: setPrefs } = useQuotaPrefs();
  const [hooks, setHooks] = useState<HooksView | null>(null);
  useEffect(() => {
    void api.hooksState().then(setHooks).catch(() => setHooks(null));
  }, []);

  return (
    <div className="st">
      <nav className="st-nav" aria-label={t("settings")}>
        <h1>{t("settings")}</h1>
        {PANES.map((p) => (
          <button key={p.id} aria-current={pane === p.id ? "page" : undefined} onClick={() => setPane(p.id)}>
            {t(p.label)}
          </button>
        ))}
      </nav>
      <div className="st-pane">
        <h2 className="st-title">{t(PANES.find((p) => p.id === pane)!.label)}</h2>

        {pane === "sources" && (
          <>
            <Panel title={t("agentHistory")}>
              <p className="vnote">{t("agentHistoryNote")}</p>
              <Roots kind="provider" roots={status.approved_roots} home={status.home} busy={busy} empty={t("noFolder")} asking={asking} onAsk={setAsking} onRemove={remove} />
              {proposed.length > 0 && (
                <div className="found">
                  <div className="vnote">{t("foundOnMac")}</div>
                  <RootsPicker paths={proposed} home={status.home} busy={busy} onApprove={approve} />
                </div>
              )}
              <div className="acts left">
                <button className="btn" disabled={busy} onClick={() => choose("provider")}>{t("addFolder")}</button>
              </div>
            </Panel>
            <Panel title={t("configRootTitle")}>
              <p className="vnote">{t("configRootNote")}</p>
              <Roots kind="config" roots={status.approved_config_roots} home={status.home} busy={busy} empty={t("configRootNone")} asking={asking} onAsk={setAsking} onRemove={remove} />
              {status.proposed_config_root && (
                <div className="found">
                  <div className="vnote">{t("foundOnMac")}</div>
                  <div className="srow">
                    <div className="lbl mono">{tildify(status.proposed_config_root, status.home)}</div>
                    <div className="ctl">
                      <button className="btn p" disabled={busy} onClick={() => run(async () => { await api.approveConfigRoot(); return t("configRootApproved"); })}>
                        {t("setupApprove")}
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </Panel>
            <Panel title={t("automatedTitle")}>
              <Row label={t("automatedShow")} note={t("automatedNote")}>
                <div className="seg" role="group" aria-label={t("automatedShow")}>
                  <button aria-pressed={!prefs.showAutomated} onClick={() => setPrefs({ showAutomated: false })}>{t("off")}</button>
                  <button aria-pressed={prefs.showAutomated} onClick={() => setPrefs({ showAutomated: true })}>{t("on")}</button>
                </div>
              </Row>
            </Panel>
            <Panel title={t("repositories")}>
              <p className="vnote">{t("repositoriesNote")}</p>
              <Roots kind="repository" roots={status.approved_repository_roots} home={status.home} busy={busy} empty={t("noRepoFolder")} asking={asking} onAsk={setAsking} onRemove={remove} />
              <div className="acts left">
                <button className="btn" disabled={busy} onClick={() => choose("repository")}>{t("addFolder")}</button>
              </div>
            </Panel>
          </>
        )}

        {pane === "projects" && <ProjectsPane onChanged={onChanged} />}

        {pane === "time" && (
          <Panel title={t("reportingZone")}>
            <p className="vnote">{t("timeZoneNote")}</p>
            <Row label={t("reportingZone")}>
              <select className="ssel" value={status.timezone_chosen ? (status.timezone ?? "") : ""} disabled={busy || status.reviewed_days > 0} onChange={(e) => chooseZone(e.target.value)}>
                <option value="">{t("system")}{status.system_timezone ? ` (${status.system_timezone})` : ""}</option>
                {zones.map((z) => (
                  <option key={z} value={z}>{z}</option>
                ))}
              </select>
            </Row>
            <p className="vnote">
              {status.reviewed_days === 1 ? t("timeZoneLockedOne") : status.reviewed_days > 1 ? t("timeZoneLockedMany", { n: status.reviewed_days }) : t("timeZoneChange")}
            </p>
          </Panel>
        )}

        {pane === "quota" && (
          <>
            <Panel title="Claude Code">
              <Row label={tap?.installed ? t("tapStatusOn") : t("tapStatusOff")} note={t("quotaClaudeHint")}>
                <button className="btn" disabled={!tap || busy} onClick={() => run(async () => (await onTap(!tap?.installed), null))}>
                  {tap?.installed ? t("quotaClaudeOff") : t("quotaClaudeOn")}
                </button>
              </Row>
              {tap && <Row label={t("tapFile")}><span className="mono path">{tildify(tap.settings_path, status.home)}</span></Row>}
            </Panel>
            <Panel title={t("quotaDisplay")}>
              <Row label={t("quotaDisplay")} note={t("quotaDisplayNote")}>
                <div className="seg" role="group" aria-label={t("quotaDisplay")}>
                  <button aria-pressed={!prefs.remaining} onClick={() => setPrefs({ remaining: false })}>{t("quotaUsed")}</button>
                  <button aria-pressed={prefs.remaining} onClick={() => setPrefs({ remaining: true })}>{t("quotaLeft")}</button>
                </div>
              </Row>
              <Row label={t("quotaPaceAware")} note={t("quotaPaceAwareNote")}>
                <div className="seg" role="group" aria-label={t("quotaPaceAware")}>
                  <button aria-pressed={prefs.paceAware} onClick={() => setPrefs({ paceAware: true })}>{t("on")}</button>
                  <button aria-pressed={!prefs.paceAware} onClick={() => setPrefs({ paceAware: false })}>{t("off")}</button>
                </div>
              </Row>
            </Panel>
            <Panel title={t("quotaMenuBar")}>
              <Row label={t("quotaMenuBar")} note={t("quotaMenuBarNote")}>
                <div className="seg" role="group" aria-label={t("quotaMenuBar")}>
                  <button aria-pressed={prefs.menuBar} onClick={() => setPrefs({ menuBar: true })}>{t("on")}</button>
                  <button aria-pressed={!prefs.menuBar} onClick={() => setPrefs({ menuBar: false })}>{t("off")}</button>
                </div>
              </Row>
            </Panel>
            <Panel title="Codex">
              <p className="vnote">{t("codexQuotaNote")}</p>
            </Panel>
          </>
        )}

        {pane === "notices" && (
          <>
            <Panel title={t("hooksTitle")}>
              <Row label={hooks?.installed ? t("hooksOn") : t("hooksOff")} note={t("hooksHint")}>
                <button className="btn" disabled={!hooks || busy} onClick={() => run(async () => { await api.hooksSet(!hooks?.installed); setHooks(await api.hooksState()); return hooks?.installed ? t("hooksDoneOff") : t("hooksDoneOn"); })}>
                  {hooks?.installed ? t("hooksTurnOff") : t("hooksTurnOn")}
                </button>
              </Row>
              <Row label={t("notifyWaiting")}>
                <div className="seg" role="group" aria-label={t("notifyWaiting")}>
                  <button aria-pressed={prefs.notifyWaiting} onClick={() => setPrefs({ notifyWaiting: true })}>{t("on")}</button>
                  <button aria-pressed={!prefs.notifyWaiting} onClick={() => setPrefs({ notifyWaiting: false })}>{t("off")}</button>
                </div>
              </Row>
              <Row label={t("notifyFinished")}>
                <div className="seg" role="group" aria-label={t("notifyFinished")}>
                  <button aria-pressed={prefs.notifyFinished} onClick={() => setPrefs({ notifyFinished: true })}>{t("on")}</button>
                  <button aria-pressed={!prefs.notifyFinished} onClick={() => setPrefs({ notifyFinished: false })}>{t("off")}</button>
                </div>
              </Row>
            </Panel>
            <CostAlerts onChanged={onChanged} />
            <Panel title={t("quotaTitle")}>
              <Row label={t("quotaNotify")} note={t("quotaNotifyNote")}>
                <div className="seg" role="group" aria-label={t("quotaNotify")}>
                  <button aria-pressed={prefs.notify} onClick={() => setPrefs({ notify: true })}>{t("on")}</button>
                  <button aria-pressed={!prefs.notify} onClick={() => setPrefs({ notify: false })}>{t("off")}</button>
                </div>
              </Row>
            </Panel>
          </>
        )}

        {pane === "recording" && (
          <>
            <Panel title={t("collector")}>
              <Row label={t("state")}><span className="live"><i className={`sdot ${status.collector}`} />{collectorText(status.collector)}</span></Row>
              {status.collector_detail && <Row label={t("detail")}><span>{status.collector_detail}</span></Row>}
              {status.runtime && <Row label={t("deviceSensor")}><span>{status.runtime.sensor}</span></Row>}
            </Panel>
            <Panel title={t("data")}>
              <Row label={t("location")}><span className="mono path">{status.data_dir ? tildify(status.data_dir, status.home) : "–"}</span></Row>
              <Row label={t("ledger")}><span>{status.ledger_present ? t("ledgerPresent") : t("ledgerMissing")}</span></Row>
              {status.data_dir_from_env && <Row label={t("note")}><span>{t("setByEnv")}</span></Row>}
            </Panel>
          </>
        )}

        {pane === "about" && (
          <Panel title={`tracr ${status.app_version}`}>
            <Row label={t("version")}><span>{status.app_version}</span></Row>
            <Row label={t("license")}><span>AGPL-3.0-or-later</span></Row>
            <Row label={t("website")}><span className="mono">tracr.jorisconrad.com</span></Row>
            <Row label={t("privacy")}><span>{t("privacyText")}</span></Row>
          </Panel>
        )}
      </div>
    </div>
  );
}

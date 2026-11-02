import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, errorText } from "../api";
import { tildify } from "../format";
import { lang, locale, t } from "../i18n";
import type { EnvFinding, EnvItem, EnvOrigin, EnvSeverity, Environment, Status } from "../types";
import { Panel, Stat } from "./ui";

type Pane = "check" | "skills" | "commands" | "agents" | "plugins" | "mcp" | "hooks" | "memory" | "permissions";
const PANES: { id: Pane; label: Parameters<typeof t>[0] }[] = [
  { id: "check", label: "envCheck" },
  { id: "skills", label: "envSkills" },
  { id: "commands", label: "envCommands" },
  { id: "agents", label: "envAgents" },
  { id: "plugins", label: "envPlugins" },
  { id: "mcp", label: "envMcp" },
  { id: "hooks", label: "envHooks" },
  { id: "memory", label: "envMemory" },
  { id: "permissions", label: "envPermissions" },
];

/** Title, advice and how to read the detail, per rule. The backend sends ids and facts only. */
const RULES: Record<string, { en: [string, string, string?]; de: [string, string, string?] }> = {
  "SKL-01": {
    en: ["Skill without a description", "Claude picks skills by their description. Without one the skill is never chosen."],
    de: ["Skill ohne Beschreibung", "Claude wählt Skills anhand ihrer Beschreibung aus. Ohne Beschreibung wird der Skill nie gewählt."],
  },
  "SKL-02": {
    en: ["Skill name differs from its folder", "Claude Code shows both names. Matching them avoids confusion.", "folder {d}"],
    de: ["Skill-Name weicht vom Ordner ab", "Claude Code zeigt beide Namen. Gleiche Namen vermeiden Verwechslungen.", "Ordner {d}"],
  },
  "SKL-03": {
    en: ["Very long skill description", "Descriptions over 1,024 characters may be cut off.", "{d} characters"],
    de: ["Sehr lange Skill-Beschreibung", "Beschreibungen über 1.024 Zeichen werden womöglich abgeschnitten.", "{d} Zeichen"],
  },
  "SKL-04": {
    en: ["Skill name used more than once", "With the same name in several places it is unclear which one runs.", "{d} times"],
    de: ["Skill-Name mehrfach vergeben", "Bei gleichem Namen an mehreren Stellen ist unklar, welcher Skill läuft.", "{d}-mal"],
  },
  "CMD-01": {
    en: ["Large command file", "A command is loaded completely each time it runs. Over 20 KB costs context on every call.", "{d}"],
    de: ["Große Befehlsdatei", "Ein Befehl wird bei jedem Aufruf vollständig geladen. Über 20 KB kostet das bei jedem Aufruf Kontext.", "{d}"],
  },
  "AGT-01": {
    en: ["Agent without a name or description", "Claude needs both to choose and start the agent."],
    de: ["Agent ohne Name oder Beschreibung", "Claude braucht beides, um den Agenten zu wählen und zu starten."],
  },
  "HOOK-01": {
    en: ["Hook program not found", "The hook points to a file that does not exist, so it fails every time it fires."],
    de: ["Hook-Programm nicht gefunden", "Der Hook zeigt auf eine Datei, die es nicht gibt, und schlägt deshalb jedes Mal fehl."],
  },
  "HOOK-02": {
    en: ["Same hook registered twice", "It runs twice for every event.", "{d} times"],
    de: ["Derselbe Hook doppelt eingetragen", "Er läuft bei jedem Ereignis zweimal.", "{d}-mal"],
  },
  "PLG-01": {
    en: ["Plugin switched on but not installed", "Your settings name a plugin that is not installed. Install it again or remove it from enabledPlugins."],
    de: ["Plugin eingeschaltet, aber nicht installiert", "Deine Einstellungen nennen ein Plugin, das nicht installiert ist. Installiere es neu oder entferne es aus enabledPlugins."],
  },
  "PLG-02": {
    en: ["Plugin folder missing", "The plugin is listed as installed, but its folder is gone."],
    de: ["Plugin-Ordner fehlt", "Das Plugin ist als installiert eingetragen, sein Ordner fehlt aber."],
  },
  "PLG-03": {
    en: ["Plugin installed but switched off", "It is not loaded. Uninstall it if you no longer need it."],
    de: ["Plugin installiert, aber ausgeschaltet", "Es wird nicht geladen. Deinstalliere es, wenn du es nicht mehr brauchst."],
  },
  "MCP-01": {
    en: ["MCP server name used more than once", "Two plugins bring a server with the same name; only one of them can be reached under it.", "{d} times"],
    de: ["MCP-Server-Name mehrfach vergeben", "Zwei Plugins bringen einen Server mit gleichem Namen mit; nur einer ist darunter erreichbar.", "{d}-mal"],
  },
  "CFG-01": {
    en: ["Settings file is not valid JSON", "Claude Code ignores a settings file it cannot read."],
    de: ["Einstellungsdatei ist kein gültiges JSON", "Claude Code ignoriert eine Einstellungsdatei, die es nicht lesen kann."],
  },
  "CFG-02": {
    en: ["Rule both allowed and restricted", "The stricter list wins, so the allow entry has no effect.", "also in {d}"],
    de: ["Regel erlaubt und zugleich eingeschränkt", "Die strengere Liste gewinnt, der Eintrag unter allow wirkt also nicht.", "auch in {d}"],
  },
  "CFG-03": {
    en: ["Many settings backups", "Old backup copies next to settings.json can be deleted.", "{d} copies"],
    de: ["Viele Einstellungs-Sicherungen", "Alte Sicherungskopien neben settings.json kannst du löschen.", "{d} Kopien"],
  },
  "MEM-01": {
    en: ["Large memory file", "It is loaded into every session. Over 40,000 characters costs context in each one.", "{d} characters"],
    de: ["Große Memory-Datei", "Sie wird in jede Sitzung geladen. Über 40.000 Zeichen kostet das in jeder Sitzung Kontext.", "{d} Zeichen"],
  },
  "MEM-02": {
    en: ["Auto-memory index over 200 lines", "Only the first 200 lines are loaded; the rest is never seen.", "{d} lines"],
    de: ["Auto-Memory-Index über 200 Zeilen", "Nur die ersten 200 Zeilen werden geladen, der Rest wird nie gesehen.", "{d} Zeilen"],
  },
};

const SEVERITY_COLOR: Record<EnvSeverity, string> = { error: "var(--bad)", warning: "var(--warn)", info: "var(--info)" };
const num = new Intl.NumberFormat(locale);

function rule(id: string): [string, string, string?] {
  const r = RULES[id];
  return r ? r[lang] : [id, ""];
}

function kb(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${num.format(Math.round(bytes / 1024))} KB`;
}

function detailText(f: EnvFinding): string | null {
  if (!f.detail) return null;
  const tpl = rule(f.rule)[2];
  const d = f.rule === "CMD-01" ? kb(Number(f.detail)) : /^\d+$/.test(f.detail) ? num.format(Number(f.detail)) : f.detail;
  return tpl ? tpl.replace("{d}", d) : d;
}

function originLabel(o: EnvOrigin): string {
  return o.kind === "user" ? t("envYours") : o.plugin.split("@")[0];
}

function dateLabel(ms: number | null): string | null {
  return ms === null ? null : new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(new Date(ms));
}

/** One row that opens in place: the name and a line of description, then the facts. */
function Row({ title, sub, chips, children }: { title: string; sub?: string | null; chips?: ReactNode; children?: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <li className={`erow ${open ? "open" : ""}`}>
      <button className="ehead" aria-expanded={children ? open : undefined} disabled={!children} onClick={() => setOpen((o) => !o)}>
        <span className="et">
          <b>{title}</b>
          {sub && <span className="vnote">{sub}</span>}
        </span>
        <span className="chips">{chips}</span>
      </button>
      {open && children && <div className="ebody">{children}</div>}
    </li>
  );
}

function Facts({ rows, home }: { rows: [string, ReactNode | null | undefined][]; home: string | null }) {
  return (
    <dl className="efacts">
      {rows
        .filter(([, v]) => v !== null && v !== undefined && v !== "")
        .map(([k, v], i) => (
          <div key={i}>
            <dt>{k}</dt>
            <dd className={typeof v === "string" && v.startsWith("/") ? "mono" : undefined}>{typeof v === "string" && v.startsWith("/") ? tildify(v, home) : v}</dd>
          </div>
        ))}
    </dl>
  );
}

function Items({ items, home, query }: { items: EnvItem[]; home: string | null; query: string }) {
  const q = query.trim().toLowerCase();
  const shown = items.filter((i) => !q || i.name.toLowerCase().includes(q) || (i.description ?? "").toLowerCase().includes(q));
  if (shown.length === 0) return <p className="vnote">{t("envNone")}</p>;
  return (
    <ul className="elist">
      {shown.map((i) => (
        <Row
          key={i.path}
          title={i.name}
          sub={i.description}
          chips={
            <>
              {!i.active && <span className="chip">{t("envOff")}</span>}
              <span className="chip">{originLabel(i.origin)}</span>
            </>
          }
        >
          <Facts
            home={home}
            rows={[
              ...Object.entries(i.fields)
                .filter(([k]) => k !== "name" && k !== "description")
                .map(([k, v]) => [k, v] as [string, string]),
              ["", `${t("envLines", { n: num.format(i.lines) })} · ${kb(i.bytes)}`],
              ["", i.modified_ms === null ? null : t("envModified", { date: dateLabel(i.modified_ms) ?? "" })],
              ["", i.path],
            ]}
          />
        </Row>
      ))}
    </ul>
  );
}

function Check({ env, home }: { env: Environment; home: string | null }) {
  const [only, setOnly] = useState<EnvSeverity | "all">("all");
  const count = (s: EnvSeverity) => env.findings.filter((f) => f.severity === s).length;
  const groups = useMemo(() => {
    const by = new Map<string, EnvFinding[]>();
    for (const f of env.findings) if (only === "all" || f.severity === only) by.set(f.rule, [...(by.get(f.rule) ?? []), f]);
    return [...by.entries()];
  }, [env.findings, only]);
  return (
    <>
      <div className="stats">
        <Stat label={t("envScore")} value={String(env.score)} note={t("envScoreNote")} tone={env.score < 90 ? "warn" : undefined} />
        <Stat label={t("envErrors")} value={String(count("error"))} tone={count("error") === 0 ? "muted" : undefined} />
        <Stat label={t("envWarnings")} value={String(count("warning"))} tone={count("warning") === 0 ? "muted" : undefined} />
        <Stat label={t("envInfos")} value={String(count("info"))} tone="muted" />
      </div>
      <div className="seg" role="group" aria-label={t("envCheck")}>
        {(["all", "error", "warning", "info"] as const).map((s) => (
          <button key={s} aria-pressed={only === s} onClick={() => setOnly(s)}>
            {t(s === "all" ? "envAll" : s === "error" ? "envErrors" : s === "warning" ? "envWarnings" : "envInfos")}
          </button>
        ))}
      </div>
      {groups.length === 0 && <p className="vnote">{t("envNoFindings")}</p>}
      {groups.map(([id, list]) => {
        const [title, advice] = rule(id);
        return (
          <Panel
            key={id}
            title={title}
            aside={
              <span className="live mono vnote">
                <i className="sdot" style={{ background: SEVERITY_COLOR[list[0].severity] }} />
                {id}
              </span>
            }
          >
            <p className="vnote">{advice}</p>
            <ul className="elist">
              {list.map((f, i) => (
                <li key={`${f.subject}-${i}`} className="erow flat">
                  <span className="et">
                    <b>{f.subject}</b>
                    {f.file && <span className="vnote mono">{tildify(f.file, home)}</span>}
                  </span>
                  <span className="chips">{detailText(f) && <span className="chip">{detailText(f)}</span>}</span>
                </li>
              ))}
            </ul>
          </Panel>
        );
      })}
    </>
  );
}

/** How Claude Code is set up on this machine, and what looks wrong (T-0016). Read only. */
export function EnvironmentPage({ status, onApproved, onError }: { status: Status; onApproved: () => void; onError: (text: string) => void }) {
  const [env, setEnv] = useState<Environment | null>(null);
  const [readAt, setReadAt] = useState<number | null>(null);
  const [pane, setPane] = useState<Pane>("check");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const home = status.home ?? null;
  const approved = status.approved_config_roots.length > 0;

  // The parent's handler changes on every render; reading it through a ref keeps `load` stable.
  const errorRef = useRef(onError);
  errorRef.current = onError;
  const load = useCallback(() => {
    setBusy(true);
    void api
      .environment()
      .then((e) => (setEnv(e), setReadAt(Date.now())))
      .catch((e) => errorRef.current(errorText(e)))
      .finally(() => setBusy(false));
  }, []);
  useEffect(() => {
    if (approved) load();
  }, [approved, load]);

  if (!approved) {
    const proposed = status.proposed_config_root;
    return (
      <div className="body">
        <div className="vempty">
          <h2>{t("envNoRoot")}</h2>
          <p>{t("envNoRootHint")}</p>
          {proposed ? (
            <button className="btn p" disabled={busy} onClick={() => (setBusy(true), void api.approveConfigRoot().then(onApproved).catch((e) => onError(errorText(e))).finally(() => setBusy(false)))}>
              {t("envApprove", { path: tildify(proposed, home) })}
            </button>
          ) : (
            <p className="vnote">{t("envNoRootSettings")}</p>
          )}
        </div>
      </div>
    );
  }

  const counts: Record<Pane, number | null> = env
    ? {
        check: env.findings.filter((f) => f.severity !== "info").length,
        skills: env.skills.length,
        commands: env.commands.length,
        agents: env.agents.length,
        plugins: env.plugins.length,
        mcp: env.mcp_servers.length,
        hooks: env.hooks.length,
        memory: env.memory.length,
        permissions: env.permissions.length,
      }
    : { check: null, skills: null, commands: null, agents: null, plugins: null, mcp: null, hooks: null, memory: null, permissions: null };
  const q = query.trim().toLowerCase();
  const match = (...texts: (string | null | undefined)[]) => !q || texts.some((x) => (x ?? "").toLowerCase().includes(q));
  const searchable = pane !== "check" && pane !== "permissions";

  return (
    <div className="st">
      <nav className="st-nav" aria-label={t("envTitle")}>
        <h1>{t("envTitle")}</h1>
        {PANES.map((p) => (
          <button key={p.id} aria-current={pane === p.id ? "page" : undefined} onClick={() => (setPane(p.id), setQuery(""))}>
            <span>{t(p.label)}</span>
            {counts[p.id] !== null && <em className="ecount">{counts[p.id]}</em>}
          </button>
        ))}
      </nav>
      <div className="st-pane" aria-busy={busy || !env}>
        <div className="ebar">
          <h2 className="st-title">{t(PANES.find((p) => p.id === pane)!.label)}</h2>
          <button className="btn s" disabled={busy} onClick={load}>{t("envRefresh")}</button>
        </div>
        {env && readAt && (
          <p className="vnote">
            {t("envReadFrom", { path: env.roots.map((r) => tildify(r, home)).join(", "), time: new Intl.DateTimeFormat(locale, { timeStyle: "short" }).format(new Date(readAt)) })} {t("envSecrets")}
          </p>
        )}
        {searchable && (
          <input className="sinput" type="search" placeholder={t("envSearch")} aria-label={t("envSearch")} value={query} onChange={(e) => setQuery(e.target.value)} />
        )}
        {env && pane === "check" && <Check env={env} home={home} />}
        {env && pane === "skills" && <Items items={env.skills} home={home} query={query} />}
        {env && pane === "commands" && <Items items={env.commands} home={home} query={query} />}
        {env && pane === "agents" && <Items items={env.agents} home={home} query={query} />}
        {env && pane === "plugins" && (
          <ul className="elist">
            {env.plugins.filter((p) => match(p.id, p.description)).map((p) => (
              <Row
                key={p.id}
                title={p.name}
                sub={p.description}
                chips={
                  <>
                    {!p.installed && <span className="chip hot">{t("envNotInstalled")}</span>}
                    {p.installed && !p.enabled && <span className="chip">{t("envOff")}</span>}
                    <span className="chip">{p.marketplace}</span>
                  </>
                }
              >
                <Facts home={home} rows={[["", t("envComponents", { s: p.skills, c: p.commands, a: p.agents, h: p.hooks, m: p.mcp_servers })], ["Version", p.version], ["", p.path]]} />
              </Row>
            ))}
          </ul>
        )}
        {env && pane === "mcp" && (
          <>
            <p className="vnote">{t("envMcpNote")}</p>
            <ul className="elist">
              {env.mcp_servers.filter((s) => match(s.name, s.target)).map((s) => (
                <li key={`${originLabel(s.origin)}-${s.name}`} className="erow flat">
                  <span className="et">
                    <b>{s.name}</b>
                    <span className="vnote mono">{[s.transport, s.target, s.env_keys.length ? t("envEnvKeys", { keys: s.env_keys.join(", ") }) : null].filter(Boolean).join(" · ")}</span>
                  </span>
                  <span className="chips">
                    {!s.active && <span className="chip">{t("envOff")}</span>}
                    <span className="chip">{originLabel(s.origin)}</span>
                  </span>
                </li>
              ))}
            </ul>
            {env.mcp_servers.length === 0 && <p className="vnote">{t("envNone")}</p>}
          </>
        )}
        {env && pane === "hooks" && (
          <ul className="elist">
            {env.hooks.filter((h) => match(h.event, h.matcher, h.program)).map((h, i) => (
              <li key={i} className="erow flat">
                <span className="et">
                  <b>{h.event}{h.matcher ? ` · ${h.matcher}` : ""}</b>
                  <span className="vnote mono">{[h.program, h.timeout_s !== null ? t("envTimeout", { s: h.timeout_s }) : null, tildify(h.file, home)].filter(Boolean).join(" · ")}</span>
                </span>
                <span className="chips">
                  {h.missing && <span className="chip hot">{t("envMissing")}</span>}
                  {!h.active && <span className="chip">{t("envOff")}</span>}
                  <span className="chip">{originLabel(h.origin)}</span>
                </span>
              </li>
            ))}
          </ul>
        )}
        {env && pane === "memory" && (
          <ul className="elist">
            {env.memory.filter((m) => match(m.name)).map((m) => (
              <li key={m.path} className="erow flat">
                <span className="et">
                  <b>{m.name}</b>
                  <span className="vnote mono">{tildify(m.path, home)}</span>
                </span>
                <span className="chips">
                  <span className="chip">{t(m.kind === "global" ? "envMemGlobal" : m.kind === "rule" ? "envMemRule" : "envMemAuto")}</span>
                  <span className="chip">{t("envLines", { n: num.format(m.lines) })}</span>
                  {m.topics > 0 && <span className="chip">{t("envTopics", { n: m.topics })}</span>}
                </span>
              </li>
            ))}
          </ul>
        )}
        {env && pane === "permissions" &&
          (["allow", "ask", "deny"] as const).map((list) => {
            const rules = env.permissions.filter((p) => p.list === list);
            return (
              <Panel key={list} title={t(list === "allow" ? "envPermAllow" : list === "ask" ? "envPermAsk" : "envPermDeny")} aside={String(rules.length)}>
                {rules.length === 0 ? (
                  <p className="vnote">{t("envNone")}</p>
                ) : (
                  <ul className="elist">
                    {rules.map((p, i) => (
                      <li key={i} className="erow flat">
                        <span className="et">
                          <b className="mono">{p.rule}</b>
                          <span className="vnote mono">{tildify(p.file, home)}</span>
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </Panel>
            );
          })}
      </div>
    </div>
  );
}

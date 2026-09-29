import { useEffect } from "react";
import { duration, hhmm, shortId, tokens } from "./format";
import type { SessionDay, TokenCount, UsageLine } from "./types";

function Tok({ t }: { t: TokenCount }) {
  const { text, unknown } = tokens(t);
  return (
    <>
      {text}
      {unknown > 0 && (
        <span className="unknown-mark" title={`${unknown} records do not state this category (unknown, not zero)`}>
          +{unknown}?
        </span>
      )}
    </>
  );
}

function actorName(s: SessionDay, actorId: string | null): string {
  const a = s.actors.find((x) => x.actor_id === actorId);
  if (!a) return "unattributed actor";
  return a.actor_kind === "known" && a.native_agent_id ? `agent ${shortId(a.native_agent_id, 10)}` : "main chain";
}

export function SessionSheet({ s, tz, onClose }: { s: SessionDay; tz: string; onClose: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const usage: UsageLine[] = [...s.usage].sort((a, b) => b.output.sum - a.output.sum);
  const diagnostics = Object.entries(s.diagnostics);

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="sheet" role="dialog" aria-label={`Session ${s.label ?? s.native_session_id}`}>
        <header>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2>{s.label ?? "No working directory"}</h2>
            <div className="mono muted">{s.native_session_id}</div>
          </div>
          <button className="button ghost" onClick={onClose} style={{ minWidth: 0 }}>
            Close
          </button>
        </header>
        <div className="body">
          <dl className="kv">
            <dt>Provider</dt>
            <dd>{s.provider}</dd>
            <dt>Working dir</dt>
            <dd className="mono">{s.cwd ?? "—"}</dd>
            <dt>Branch</dt>
            <dd className="mono">{s.git_branch ?? "—"}</dd>
            <dt>Evidence span</dt>
            <dd>
              {s.first_evidence_at && s.last_evidence_at
                ? `${new Date(s.first_evidence_at).toLocaleString("en-GB", { timeZone: tz })} – ${new Date(
                    s.last_evidence_at,
                  ).toLocaleString("en-GB", { timeZone: tz })}`
                : "—"}
            </dd>
            <dt>Human prompts</dt>
            <dd>
              {s.engagement_instants.length === 0
                ? "none this day"
                : s.engagement_instants.map((t) => hhmm(t, tz)).join("  ")}
            </dd>
          </dl>

          <section>
            <h3>Agents this day · execution is agent time, never human time</h3>
            <table className="data">
              <thead>
                <tr>
                  <th>Actor</th>
                  <th>Execution</th>
                  <th>Segments</th>
                  <th>Turns</th>
                  <th>Tool calls</th>
                </tr>
              </thead>
              <tbody>
                {s.actors.map((a) => (
                  <tr key={a.actor_id}>
                    <td>{actorName(s, a.actor_id)}</td>
                    <td>{duration(a.execution_ms)}</td>
                    <td>{a.segments.length}</td>
                    <td>{a.turns}</td>
                    <td>{a.tool_calls}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>

          <section>
            <h3>Tokens this day · per actor and model, each request counted once</h3>
            <table className="data">
              <thead>
                <tr>
                  <th>Actor · model</th>
                  <th>Input</th>
                  <th>Cache write</th>
                  <th>Cache read</th>
                  <th>Output</th>
                </tr>
              </thead>
              <tbody>
                {usage.map((u, i) => (
                  <tr key={i}>
                    <td title={`${u.records} records${u.identified ? "" : ", unidentified"}`}>
                      {actorName(s, u.actor_id)} · <span className="mono">{u.model ?? "model unknown"}</span>
                      {!u.identified && <span className="muted"> · unidentified</span>}
                    </td>
                    <td>
                      <Tok t={u.input_fresh} />
                    </td>
                    <td>
                      <Tok t={u.cache_creation} />
                    </td>
                    <td>
                      <Tok t={u.cache_read} />
                    </td>
                    <td>
                      <Tok t={u.output} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="note">
              Estimated API cost is not derived yet (S1.5). It will be labeled as an API-equivalent estimate, not spend.
            </p>
          </section>

          {diagnostics.length > 0 && (
            <section>
              <h3>Quality</h3>
              <dl className="kv">
                {diagnostics.map(([k, n]) => (
                  <div key={k} style={{ display: "contents" }}>
                    <dt className="mono">{n}</dt>
                    <dd>{k.replaceAll("_", " ")}</dd>
                  </div>
                ))}
              </dl>
            </section>
          )}
        </div>
      </aside>
    </>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorText } from "./api";
import type { ProjectOp, ProjectsView } from "./types";

/**
 * Projects and which repository belongs to which (S3.1). A project is a name
 * you choose; mapping a repository only regroups the day, no minute moves.
 */
export function ProjectsSection({ onChanged }: { onChanged: (text: string, error?: boolean) => void }) {
  const [view, setView] = useState<ProjectsView | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);

  // The parent passes a fresh callback on every render; keep the latest one without reloading for it.
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
    <section>
      <h3>Projects</h3>
      <p className="note">
        A project is a name you choose, for example a client or a product. Map your repositories to it and the day also shows
        your counted time per project. Nothing is moved: the time stays on its repositories.
      </p>
      {view.projects.length > 0 && (
        <ul className="root-list">
          {view.projects.map((p) => (
            <li key={p.id} className={deleting === p.id ? "asking" : undefined}>
              {renaming?.id === p.id ? (
                <>
                  <input
                    className="input"
                    value={renaming.name}
                    maxLength={80}
                    aria-label="Project name"
                    onChange={(e) => setRenaming({ id: p.id, name: e.target.value })}
                  />
                  <span className="row-actions">
                    <button
                      className="button primary small"
                      disabled={busy || renaming.name.trim() === ""}
                      onClick={() => run({ op: "rename", id: p.id, name: renaming.name }, `Renamed to ${renaming.name.trim()}`)}
                    >
                      Save
                    </button>
                    <button className="button ghost small" disabled={busy} onClick={() => setRenaming(null)}>
                      Cancel
                    </button>
                  </span>
                </>
              ) : deleting === p.id ? (
                <div className="ask">
                  <span>{p.name}</span>
                  <p>Delete this project? Its repositories become unmapped; your counted time is not touched.</p>
                  <div className="actions left">
                    <button className="button primary small" disabled={busy} onClick={() => run({ op: "delete", id: p.id }, `Deleted ${p.name}`)}>
                      Delete project
                    </button>
                    <button className="button ghost small" disabled={busy} onClick={() => setDeleting(null)}>
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <>
                  <span>{p.name}</span>
                  <span className="row-actions">
                    <button className="button ghost small" disabled={busy} onClick={() => setRenaming({ id: p.id, name: p.name })}>
                      Rename
                    </button>
                    <button className="button ghost small" disabled={busy} onClick={() => setDeleting(p.id)}>
                      Delete
                    </button>
                  </span>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
      <form
        className="new-project"
        onSubmit={(e) => {
          e.preventDefault();
          const n = name.trim();
          if (n) void run({ op: "create", name: n }, `Created ${n}`).then(() => setName(""));
        }}
      >
        <input className="input" placeholder="New project name" maxLength={80} value={name} onChange={(e) => setName(e.target.value)} aria-label="New project name" />
        <button className="button secondary" type="submit" disabled={busy || name.trim() === ""}>
          Create
        </button>
      </form>

      {view.projects.length > 0 && (
        <>
          <h3 className="sub">Repositories</h3>
          {view.repositories.length === 0 ? (
            <p className="muted">No repository seen in your agent sessions yet.</p>
          ) : (
            <ul className="root-list">
              {view.repositories.map((r) => {
                const current = view.repository_projects[r.key] ?? "";
                return (
                  <li key={r.key}>
                    <span className="mono" title={r.key}>
                      {r.label}
                    </span>
                    <select
                      className="select"
                      value={nameOf(current) ? current : ""}
                      disabled={busy}
                      aria-label={`Project of ${r.label}`}
                      onChange={(e) =>
                        void run(
                          { op: "map", repository: r.key, project: e.target.value === "" ? null : e.target.value },
                          e.target.value === "" ? `${r.label} is in no project` : `${r.label} is in ${nameOf(e.target.value)}`,
                        )
                      }
                    >
                      <option value="">No project</option>
                      {view.projects.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.name}
                        </option>
                      ))}
                    </select>
                  </li>
                );
              })}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

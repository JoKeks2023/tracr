# T-0009 Projects, bindings and issue suggestions (S3.1 + S3.2, TRACR-16)

Spec: `docs/mvp.md` M6 and M7, `docs/attribution.md` §2 (context targets), §4 (evidence ladder), §6 (rejection memory). Local only: GitHub and Huly context is S3.3 and S3.4.

## Decisions (user, 2026-10-26)
- **Three small steps, each usable on its own:**
  - **A. Projects and repository→project mapping** (settings, the day grouped by project).
  - **B. Bindings:** a standing assignment of a session or repository to a project or an issue key, with valid-from/to.
  - **C. Suggestions** from the branch key and the transcript's PR link, with rejection memory.
- **Day view:** "Counted by project" first, the repository line stays below it (smaller, as before). Repositories without a project keep their own name.
- **Suggestions do not count until accepted** (attribution §4: a branch key is a candidate, not ownership; OQ-P-05 is still open).

## Principles carried over
- A project is a name the user chooses. Mapping a repository only regroups the day: no minute moves, no history is edited, a reviewed day keeps the targets it was reviewed with.
- Everything the user decides is an append-only user action in the ledger (`setting.projects.create|rename|delete|map`).
- Enumerable choices are dropdowns (repository → project is a select), names are free text.

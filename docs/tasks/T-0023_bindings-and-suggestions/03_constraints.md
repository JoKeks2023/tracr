# Constraints and decisions

From the spec (not reopened):
- A binding never creates or removes human time (TW-09). It acts on work only.
- Precedence: correction > manual entry > binding > structural (repository/worktree) > unattributed (attribution §5).
- Bindings that disagree are `conflicting`, not last-write-wins (OQ-P-20 default).
- Suggestions never count until accepted. A rejected (subject, target) is never suggested again (AT-05).

Decided here (conservative, documented in `bindings.rs`):
- **Subjects:** session (native ID) and repository (key). Timer bindings are not needed: a timer entry already carries its own target.
- **Targets:** project, category or label. Not a repository (that is observed, not bound).
- **Issue keys become free labels** (`other:TRACR-12`). Scoped Huly/GitHub references need a workspace or repository (attribution §2), which arrives with S3.3/S3.4.
- **Validity:** a binding applies to a piece of work when it is valid at the piece's start; work pieces are cut at every binding start/end (`binding_boundary`), so it applies from its exact time.
- **Branch keys:** `ABC-123` at word boundaries, case-insensitive, 2–10 letter prefix, not followed by `-<digit>` (dates), minus a short denylist (UTF, SHA, ISO, RFC, …).
- **Storage:** two settings (`bindings`, `rejected_suggestions`) written by user actions `setting.bindings.bind|end|reject`; no ledger migration.
- **Agent records:** bindings are not yet applied to sessions' usage/cost grouping (projects page). Human time only, plus `bound_to` on each session.

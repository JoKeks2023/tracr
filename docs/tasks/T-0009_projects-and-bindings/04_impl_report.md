# T-0009 implementation report

## Step A: projects and repository mapping (this branch)
- `tracr-app::projects`: `Project`, `ProjectConfig`, `ProjectOp` (create, rename, delete, map), `apply`, `project_totals`, `clean_name`. Stored as two settings (`projects`, `repository_projects`); each change is one user action `setting.projects.<op>`.
- `App::project_op`, `Reader::project_config`, `Reader::known_repositories`; `Reader::day_agents` fills `counted_by_project` from the numbers the day shows (the reviewed snapshot for a reviewed day). The collector gets a `Job::Projects`, so edits go through the writer it holds.
- Desktop: commands `projects` and `project_op`; Settings gets a Projects section (create, rename, delete with an inline confirmation, a dropdown per known repository); the day view shows "Counted by project" with the repository line below.
- Rules worth knowing: names are trimmed, 1 to 80 characters, unique ignoring case; deleting a project drops its mappings; two repositories that share a label (`owner/name`) and sit in different projects cannot be told apart by label and stay unmapped.

## Evidence
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `pnpm typecheck`, `cargo test --workspace`: green (34 suites).
- New tests: four unit tests in `projects.rs` (names, delete drops mappings, grouping and nothing added or lost, shared label stays unmapped) and `tests/projects.rs`: a real collected day (git repo `example/repo`, 10 counted minutes) grouped under a project, `counted_by_target` and `counted_ms` unchanged by mapping, rename shows at once, delete restores the plain view, the four user actions are in the ledger in order.
- In the real window: the Settings sheet renders the new Projects section (explanation, name field, Create) against a demo ledger.

## Not verified
- Creating a project, mapping a repository and the "Counted by project" line **in the real window**. The attempt was cut short: another program on the machine repeatedly took focus and the window disappeared, and synthetic input is not safe to send then (it once landed in the terminal instead). Everything behind the UI is tested; the UI path is typechecked, not clicked.
- Reviewed days: the grouping comes from the reviewed snapshot by label; covered by reasoning and the unit test of the grouping, not by a reviewed-day test.

## Next
Step B (bindings) and step C (suggestions, rejection memory).

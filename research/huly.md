# Huly: deployed MCP versus upstream platform

Research date: **2026-09-28**. This chapter uses the actual custom `mcp__huly_joris` catalog and correct workspace calls, not capabilities borrowed from an unrelated upstream MCP. [Live evidence](SOURCES.md#huly-live)

## Verified workspace context

| Read | Result |
|---|---|
| list_workspaces | Jokeks2023 (`jokeks2023`) and separate AVMS workspace |
| list_projects in jokeks2023 | TRACR — tracr; TSK — Default |
| get_project TRACR | Zero issues; Backlog, Todo, In Progress, Done, Canceled |
| list_issues TRACR | Empty |
| list_teamspaces | Empty |

The connector was initially attached to a different workspace, then briefly required reconnection. Those errors were resolved. Explicit workspace selection now exists; every change requires a workspace argument. No project was created, no test time was logged, and no real issue was edited. Imported project documentation is still not located in the available repo/teamspace context. [CONTEXT](SOURCES.md#context)

The returned TRACR status list does not yet include `Review in progress` or `Merged` from the supplied workflow convention. This is an observed setup difference, not something silently normalized during product research. No issue currently exists whose status needs research-progress maintenance.

## Exact advertised capability inventory

Every name below belongs to `mcp__huly_joris__`. **L-call** means successfully called in TRACR context; **L-schema** means available schema only, without a mutation/runtime test.

| Area | Read tools | Change tools | Verification |
|---|---|---|---|
| Workspace/project | list_workspaces, list_projects, get_project | create_project | Reads L-call; create forbidden by user convention |
| Issues | list_issues, get_issue, search_issues | create_issue, update_issue, delete_issue | list L-call; others L-schema |
| Workflow | list_issue_statuses | create_issue_status | L-schema; project states also returned by get_project |
| Relations | get_issue may expose details; exact fields U | set_parent, add_relation, add_blocked_by, link_document | L-schema; no separate general relation-list tool |
| Classification | list_labels, list_components, list_milestones | create_label, add_label, remove_label, create_component, create_milestone | L-schema |
| Issue discussion | list_comments | add_comment, delete_comment | L-schema |
| Attachments | list_attachments | attach_file, delete_attachment | L-schema |
| Documents | list_teamspaces, list_documents, get_document | create_teamspace, create_document, update_document, delete_document | Teamspace read L-call; rest L-schema |
| People/organizations | list_members, list_organizations, get_organization | create_organization, update_organization | L-schema |
| Communication | list_channels, list_messages | create_channel, start_direct_message, send_message | L-schema; no messages sent |
| Time | No dedicated time-entry read/report tool advertised | log_time | L-schema; user confirms existing availability |

Exact useful request fields:

- `get_project`: workspace, identifier.
- `get_issue`: workspace, identifier such as `TRACR-12`.
- `list_issues`: workspace, projectIdentifier, optional status/priority/limit. No pagination cursor is advertised in this schema.
- `search_issues`: workspace, query, optional limit. Search-result shape and completeness still require a populated test fixture.
- `log_time`: **workspace, identifier, hours**, optional **description**.
- `set_parent`: workspace, child identifier, optional/null parentIdentifier.
- `get_document`: workspace, documentId; tool description says content needs the deployment's HULY_FRONT_URL configuration.

Identifiers and units above come from schemas. The schemas do not expose time-entry start/end, date, employee override, currency, external source ID or idempotency key. They also do not expose a generic database query, GitHub-mapping query, webhook subscription, time-entry update/delete, or report endpoint. **Not advertised** is narrower than **backend incapable**. [HULY-LIVE](SOURCES.md#huly-live)

## Upstream implementation evidence

Official Huly docs describe repository mapping and two-way issue/comment synchronization; PRs appear through the integration. This requires configured mapping and permissions. SERV/PLAY confirmation cannot be transferred automatically to TRACR. [Huly GitHub docs](https://docs.huly.io/integrations/github/)

**C:** `plugins/tracker/src/index.ts` defines `Issue.reportedTime` as a trigger-updated aggregate and `reports` as a collection of `TimeSpendReport`. A report attaches to an issue and contains employee, date, value in person-hours and description. It is therefore possible in the platform model to distinguish individual worklogs from an issue total. [Tracker source](https://github.com/hcengineering/platform/blob/f4f3d30bf1b62d766a6d6d2b481bdcdf6f24c37c/plugins/tracker/src/index.ts)

**C:** `issueBase.ts` looks up sync metadata by space/URL and writes a GitHub mixin containing number, URL and repository reference. This offers an identity bridge if accessible through the deployed integration. It does not mean `get_issue` serializes it. [Sync source](https://github.com/hcengineering/platform/blob/f4f3d30bf1b62d766a6d6d2b481bdcdf6f24c37c/services/github/pod-github/src/sync/issueBase.ts)

Huly's API entry point links TypeScript platform examples. They establish a separate integration route, not stable REST coverage equivalent to GitHub. The HR docs describe preliminary attendance/leave functionality; their caveat must not be applied indiscriminately to issue worklog storage. [API](https://docs.huly.io/getting-started/api-tools/), [HR](https://docs.huly.io/team-resources/human-resources/)

## Reading and reporting: what is still unknown

There are no issues in TRACR on which to verify `get_issue`'s real worklog or external-reference response. The custom MCP source/deployment revision was not found in imported docs because those docs are absent. Therefore this package does not claim:

- that `log_time` creates a TimeSpendReport rather than another representation;
- that issue detail includes individual entries, aggregate hours, author or external URL;
- that retries are deduplicated, negative adjustments are supported, or entries can be backdated;
- that project-wide queries paginate completely or expose deletion history;
- that this deployment exports reports or supports upstream client features unchanged.

Next evidence required: fork repository/revision; handler code for `log_time`, `get_issue`, list/search; sanitized existing worklog response; verified TRACR GitHub mapping. A dedicated test issue could later validate write/read/update semantics, but no production write is needed to pretend the research has that evidence.

## Candidate integration plan, not implementation

1. Read project/issue catalogs with explicit workspace and cache stable IDs.
2. Store user-confirmed tracr↔Huly mappings and source URLs separately from labels.
3. Keep a local authoritative interval/revision ledger; Huly's hours-only input cannot preserve all detail.
4. Export only reviewed amounts with a local receipt and external entry ID if returned.
5. On response loss, mark outcome unknown. Without remote lookup/idempotency, automatic retry risks duplicate hours.
6. Read remote worklogs before attempting bidirectional time sync. If unavailable, expose one-way export as a limited capability, not full synchronization.

A candidate MCP extension would list entries with stable IDs, issue, author, date, duration, modified time and source key; support idempotent create and revision-aware update; and expose GitHub relations. These are requested future capabilities, not currently callable tools.

Follow existing user workflow: code issues through GitHub, architecture/product issues through Huly; no manual cross-posting; link auto-created PR tracking issues to their parent when a PR is eventually created. This research created no PR or remote issue. [CONTEXT](SOURCES.md#context)

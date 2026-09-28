# Candidate entities and boundaries

Research date: **2026-09-28**. **All entities and retention periods here are proposals, not a finalized schema.** Precedents: normalized connector contract [T4](SOURCES.md#t4--claudescope-multi-agent-explorer), derived caches [T2](SOURCES.md#t2--claudoscope-native-macos), event/usage tables [T5](SOURCES.md#t5--claude-usage), worklog semantics [M1](SOURCES.md#m1--professional-time-entry-semantics), Huly report model [huly.md](huly.md).

## Candidate processing patterns

```text
Provider evidence → normalized events → candidate activity → resolved worklog
       │                    │                 │                   │
       └── token usage ─────┴── cost estimates ┴── reports/diary ──┘
Git / GitHub / Huly → external identities and evidence-backed relations
Manual timers/corrections → authoritative user intent → resolved worklog
```

Compare two storage choices: immutable evidence envelopes plus derived tables, or normalized authoritative facts with bounded raw payload retention. The former improves replay after parser changes; the latter reduces sensitive storage. A hybrid may retain metadata long-term and payload briefly. Do not claim full replay once source content is deleted.

Agent providers should describe observation capabilities; integrations should describe read/write/sync capabilities. Correlation should operate on normalized evidence instead of calling agent-specific parsers. UI, pricing and streak policies should not be baked into raw events. This is a candidate separation of responsibilities, not production architecture.

## Retention vocabulary

- **Payload:** candidate default off or 30 days with opt-in longer retention; prompts/tool outputs can contain secrets.
- **Evidence metadata:** candidate 90 days minimum for troubleshooting, configurable longer for replay; compact durable facts may outlive payload.
- **History:** retain until user deletion/export policy; user corrections and diary entries are not disposable caches.
- **Remote cache:** refresh/expire according to sync needs; retain identity/tombstones while referenced.
- **Derived:** rebuildable; delete when underlying data/policy is removed, unless explicitly finalized.

These are product options, not legal retention advice. Final retention must also account for backups, search indexes and exported copies.

## Entity candidates

| Entity | Why / producer | Raw, derived or authoritative | User edits | Candidate retention | Relationships |
|---|---|---|---|---|---|
| Project | User-facing work grouping; user/integration mapping | Authoritative mapping | Name, membership, defaults | History | Repositories, issues, worklogs |
| Repository | Stable codebase identity; Git/integration | Observed + mapping | Aliases/merge mappings | History while referenced | Projects, checkouts, commits |
| Checkout / Worktree | Distinguish concurrent paths/HEADs; local Git | Observed | Override association | Metadata; tombstone after removal | Repository, sessions, branch observations |
| BranchObservation | Time-specific branch/HEAD; Git sampler | Raw fact | No; correction annotation | Evidence/history as needed | Checkout, event, commit |
| Commit | Delivered code identity; Git/GitHub | Observed external fact | Notes/links only | Remote cache + referenced history | Repository, parents, PRs, segments |
| GitHubIssue | External task; GitHub | Cached external fact | Via explicit remote workflow only | Remote cache | Project, external identity, PRs |
| PullRequest | Review/delivery relation; GitHub | Cached external fact | Local notes, not silent remote edit | Remote cache/history | Commits, issues, Huly mapping |
| HulyProject | Workspace-specific task container; Huly | Cached external fact | Mapping only | Remote cache | Workspace, Huly issues, Project |
| HulyIssue | Huly task identity/state; MCP/API | Cached external fact | Mapping/notes; explicit remote changes | Remote cache/history | HulyProject, external refs, worklogs |
| ExternalIdentity / Link | Prevent title-based/number-only joins; integration/user | Observed or confirmed | Confirm/reject | History while referenced | Host/workspace/repository/native ID |
| AgentProvider | Capabilities and parser version; adapter | Configuration | Enable/source scope | While configured + version history | Sessions, imports |
| AgentSession | Provider-native session/thread; agent | Observed | Display label/mapping only | History metadata; payload separate | Provider, parent/child, checkout, events |
| Agent / SubagentRun | Distinguish actor from conversation; provider | Observed | Notes only | History metadata | Parent session, events, usage |
| SourceObservation | Original event envelope and optional payload; collector | Raw | Redact/delete, not overwrite | Payload/evidence policy | Source cursor, provider, normalized events |
| AgentEvent | Portable semantic event; normalizer | Derived from raw | No; reparse/version | Evidence/history | Session, turn/item, observation |
| CollectorHealth | Missing data versus inactivity; collector | Raw operational fact | Acknowledge only | Evidence metadata | Source/machine, coverage intervals |
| ActivityCandidate | Possible interval/task; inference | Derived | Accept/reject via correction | Derived | Evidence, competing candidates |
| ActivitySegment | Reconciled inferred human/agent interval; resolver | Derived | Override via revision | Derived/history when referenced | Session(s), issue, evidence |
| ManualTimeEntry | Timer/retroactive user assertion | Authoritative | Start/end/task/notes | History | Project/issue, revisions |
| WorklogEntry | Counted effort used by reports/export; resolution | Derived + confirmed authority | Revisioned corrections | History | Source segments/manual entries, exports |
| WorklogRevision | Audit split/merge/override; user/resolver | Authoritative history | Append/undo | History with entry | Prior revision, actor, reason |
| TokenUsage | Deduplicated request or counter delta; provider adapter | Observed normalized fact | No; source correction tracked | History aggregate; detail configurable | Request/session/model, source |
| PriceRule | Rates and effective period; official source/user override | Versioned configuration | Explicit scenario override | History while referenced | Model/provider/tier, estimates |
| CostRecord | Monetary kind/value; billing import or estimator | Observed bill / derived estimate | Annotation or adjustment | History | Usage, price rule, subscription |
| Subscription | Paid period and plan; user/billing import | Authoritative input | Amount, period, currency | History | Allocation policy, cost records |
| AllocationRun | Reproducible expense split; calculator | Derived/finalizable | Policy/revision | History when finalized | Subscription, project weights, unallocated amount |
| DiaryEntry | User-authored/generated narrative; user/report process | Authoritative edit + generated draft | Yes | History | Facts, report revision, projects |
| DailySnapshot | Cached or finalized day's report; reporting | Derived or immutable finalized revision | Regenerate/new revision | Derived/history if finalized | Worklog revision, timezone, pricing, watermark |
| QualifyingDay | Day-level streak evidence; policy | Derived | Underlying facts or explicit override | Derived/history policy record | Calendar date, activity, rule version |
| Streak | Runs of qualifying days; calculator | Derived | Policy only | Derived | QualifyingDay, timezone/policy |
| Contribution | Metric-specific daily count/value; report | Derived | Underlying facts only | Derived | Date, metric, source references |
| SyncReceipt / OutboxItem | External write intent/outcome; exporter | Authoritative transaction history | Retry/reconcile, not erase outcome | History while exported | Worklog revision, remote ID, source key |

## Candidate keys and invariants

- Provider-native IDs are scoped by provider, installation/machine and account as needed. Huly identifiers are workspace-scoped; GitHub issue numbers are repository-scoped.
- Occurred-at, observed-at and source sequence/cursor are separate. Nullable means unknown; zero means observed zero.
- Session parentage, issue relations and segment attribution can be many-to-many. A foreign key to one “current issue” cannot represent all history.
- Monetary values include currency, scale, kind and price revision. Usage counters include cumulative/delta semantics.
- Human work totals use interval union or explicit allocation. Agent-time totals may overlap; labels must keep the distinction.
- Derived results carry algorithm/policy versions and source references. User overrides survive parser upgrades.
- Remote deletions/revoked permissions must not silently delete locally authored diary history. User-directed deletion must cover raw payload, indexes and derived sensitive content.

## Decisions intentionally deferred

No ORM, database engine, migration layout, event-bus technology, UI framework or synchronization transport is chosen. Whether WorklogEntry stores exact intervals or an allocation across interval atoms needs prototype validation. Whether payload retention defaults to zero or 30 days needs user research. A separate immutable event store may be unnecessary if normalized facts plus provenance support the required corrections.

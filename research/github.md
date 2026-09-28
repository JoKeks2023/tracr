# Git and GitHub correlation

Research date: **2026-09-28**. Official references: [G1](SOURCES.md#g1--git-and-github); implementation precedents: [T1](SOURCES.md#t1--claude-code-timelog), [T4](SOURCES.md#t4--claudescope-multi-agent-explorer), [H1](SOURCES.md#h1--huly-upstream).

## Local repository facts versus inferred work

Git worktrees share repository data but retain worktree-specific state. A single project can therefore have several simultaneous CWDs/branches. Treat checkout identity separately from repository identity. [Git worktree](https://git-scm.com/docs/git-worktree)

**P:** Capture Git root, common repository directory, worktree path, current HEAD/ref and normalized remote identity at the observation time. Cache expensive queries, refresh on relevant changes, and tolerate detached HEAD, no remote, submodules and non-Git projects. A directory basename is a display label, not a key.

Commit identity, author/committer timestamps, parentage and changed paths provide evidence of output. A commit time does not reveal the time spent preparing it. Generated files and agent edits are not proof of human attention. Avoid interpreting file count, line count or commit count as a scalar productivity metric.

## Attribution ladder to evaluate

| Evidence | Suggested confidence | Caveat |
|---|---|---|
| User binds session/segment to Huly issue | Highest user intent | Mapping may later change |
| Verified Huly↔GitHub external relation | Strong identity | Requires access to the actual relation |
| PR closing relationship to issue | Strong relationship | A PR can address several issues |
| Branch contains scoped issue key | Candidate | Convention, not authoritative ownership |
| Commit/PR description mentions issue | Supporting | Mention is not necessarily a closing relationship |
| Prompt/tool text mentions issue | Weak unless confirmed | May discuss an unrelated dependency |
| Changed-file similarity | Suggestion only | Shared files and monorepos are ambiguous |

Timelog verifies that branch regex matching is feasible; multi-agent Claudescope's optional PR projection demonstrates a normalized relation channel. Neither alone proves reliable issue attribution. [T1](SOURCES.md#t1--claude-code-timelog), [T4](SOURCES.md#t4--claudescope-multi-agent-explorer)

**P:** Store candidate links with evidence, confidence, source scope and valid-from/to. Keep manual rejection so a rejected guess does not reappear after every reindex. Do not rewrite branches to force a research-time convention. Existing global no-squash instructions remain in force. [CONTEXT](SOURCES.md#context)

## GitHub data acquisition

| Candidate API use | Benefit | Tradeoff |
|---|---|---|
| REST repository/issues/PRs/commits/files | Straightforward incremental resource reads | Pagination and separate relationship requests |
| GraphQL relationship queries | Nested joins and contribution calendar | Point/node limits, partial responses, query complexity |
| Webhooks | Timely updates | Needs reachable receiver/relay; sleeping local app cannot be assumed reachable |
| Scheduled incremental pulls | Simpler local-only deployment | Delay, rate budget and missed-window reconciliation |

REST's ordinary authenticated-user budget is 5,000 requests/hour, with exceptions and stricter resource/secondary limits. GraphQL has a separate point budget; 5,000 points is not 5,000 arbitrary queries. Respect response headers, pagination and backoff; do not hard-code one account-wide number. [REST](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api), [GraphQL](https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api)

GitHub documents webhook signature protection, prompt acknowledgement and delivery identity handling. **P:** a relay, if later chosen, should queue and deduplicate notifications; API reconciliation still establishes current state. Local-first does not require public inbound ports on the desktop. [Webhooks](https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks)

GitHub supports issue-closing keywords and manual PR links. Closing behavior depends on the merge/default-branch context. A plain `#12` must be interpreted within a repository; Huly `TRACR-12` is a different namespace. [Linking](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue)

## Candidate synchronization contract

Use provider node/resource ID plus host/repository/workspace as identity; retain display numbers and URLs separately. Preserve created/updated/closed/merged times with their meanings. Cache permission failures as unavailable, not deleted. Incremental scans need overlap windows and stable cursors where available; changed files may be paginated or unavailable and should carry coverage status.

A work segment can link to several commits, a commit to several segments, and a PR to several issues. Represent those relations explicitly instead of nesting one issue under one session forever. Do not split time equally merely because two issues are mentioned.

## Huly relationship

Upstream's inspected GitHub sync code stores `githubNumber`, URL and repository references in a GitHub issue mixin; sync metadata is looked up using project space and external URL. This is stronger correlation evidence than comparing titles. Whether the custom MCP returns these fields is still unverified. [Huly source](https://github.com/hcengineering/platform/blob/f4f3d30bf1b62d766a6d6d2b481bdcdf6f24c37c/services/github/pod-github/src/sync/issueBase.ts)

The user confirms bidirectional synchronization for SERV/PLAY. Correct-workspace discovery does **not** prove TRACR's repository mapping. Do not duplicate issue/comment writes across both systems. Verify mapping before eventual export, preserve explicit workspace selection and follow the supplied PR-parent convention. [Huly](huly.md)

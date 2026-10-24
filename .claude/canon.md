# Canon

Settled engineering decisions for this repository. The protocol lives in
`.claude/rules/canon.md`. Append new records at the end; supersede, never
rewrite. Managed with Claudoscope, safe to edit by hand.

## Codex subagent_history_start_ordinal is not a copy marker
kind: gotcha | date: 2026-10-24 | status: canon
In all 17 local child threads the value equals the thread's own line count, so treating it as a copy boundary discards every child record. It is kept as a fact and unused; architecture §8.1 was corrected (TRACR-9).
Because: the first reading of the spec would have silently dropped all subagent activity.

## Codex item IDs are scoped by thread
kind: gotcha | date: 2026-10-24 | status: canon
`item_completed` IDs repeat across unrelated threads (33 repeats in the local corpus), so record keys for items include the thread. Tool records key on call IDs and usage on `response_id`, which do not repeat.
Because: unscoped keys made 16 real prompts look like copies and lost them; a regression test covers it.

## Codex interactive sources are an allowlist, currently vscode only
kind: choice | date: 2026-10-24 | status: canon
Only `vscode` threads count prompts as engagement; `cli` and `exec` start `automated` turns until fixtures cover them. A UserMessage in a non-interactive or child thread is counted as `prompt_from_non_interactive_source`, never engagement.
Because: `vscode` is the only value in the real corpus, and guessing for others would inflate human time.

## Unpriced is never zero
kind: constraint | date: 2026-10-24 | status: canon
The pricing catalog has no OpenAI prices, so Codex usage shows "unpriced" and is excluded from cost totals, with the unpriced token share shown in the quality footer.
Because: a missing price rendered as $0 would make the day's cost look complete when it is not.

## Cost is labelled as estimated API cost, not spend
kind: convention | date: 2026-10-24 | status: canon
Computed figures read "est. API cost · not spend"; the figure from Claude Code's `totalCostUSD` reads "provider's own running total". Money kinds are never merged into one number.
Because: that total's scope is not the transcript session (resumed sessions carry identical values), and our estimate runs 5-15 % below it on large sessions for unexplained reasons.

## Ledger and derived store have fixed roles
kind: constraint | date: 2026-10-24 | status: canon
The ledger is immutable and written through one reconcile path; the derived store is rebuildable from it, and a normalizer version change forces a rebuild. Sessions run in parallel with no "active session".
Because: every later correction, re-import and format migration relies on being able to rebuild derived state from the ledger alone.

## Agent time is not human time, and unknown is visible but uncounted
kind: constraint | date: 2026-10-24 | status: canon
Agent execution minutes and human minutes are reported separately; time that cannot be attributed is shown as unknown and never added to counted time.
Because: walkthrough C only reconciles (52 counted, 13 unknown) if neither is folded into the other.

## tracr is local-first
kind: constraint | date: 2026-10-24 | status: canon
Provider history stays on the machine; dogfooding uses real local Claude and Codex history and nothing is uploaded. Provider roots must be approved by the user before they are read.
Because: transcripts contain private work, and approval is the user's control over what enters the ledger.

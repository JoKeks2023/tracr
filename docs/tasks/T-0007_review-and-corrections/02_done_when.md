# Done when

S2.1 corrections

- [x] Walkthrough B review outcomes: unresolved 87/20/0, break 87/0/20, confirmed 107/0/0 with targets A 7 / B 13, partial 94/0/13 (tested)
- [x] Overlap supersession, undo (revoke), revoked revoke; confirming blank time; targets never change minutes (tested)
- [x] End to end through the collector: confirm + target → counted and attributed; undo restores; history shows `revoked` (tested)
- [x] INV-5: the agent-sessions export is identical with and without human-time corrections (tested)
- [ ] Manual check in the app (the release app could not be swapped yet: the running instance did not quit)

S2.2 manual entries and the timer

- [x] Walkthrough A: timer 75 (`timer_measured`) + meeting 60 (`user_asserted`) = 135; blank elsewhere (tested in core and end to end through the collector)
- [x] Manual entry overrides inferred and unknown time; a correction overrides the entry (tested)
- [x] Overlapping entries count once; different targets → `conflicting` (TW-04) (tested)
- [x] Edit and delete are revisions; at most one running timer (start stops a running one); a running timer counts up to now (tested)
- [ ] Manual check in the app (timer in header and menu bar, Add time, edit/delete)

S2.3 review

- [x] Review stores the resolved day by value as a ledger revision (schema v2: `worklog_revision`, `discrepancy_dismissal`, append-only by trigger, incl. `REPLACE`) (tested)
- [x] Review freezes counting: a later correction leaves r1 unchanged and shows a discrepancy with the exact delta (tested)
- [x] Dismiss acts on exactly one difference; a different later change shows again (tested)
- [x] Accept creates r2 and needs a reason (tested)
- [x] E-2: a full rebuild (renormalization) leaves the reviewed day unchanged, with no false discrepancy (tested)
- [x] The timesheet exports the reviewed revision (`worklog_state = reviewed rN`) (tested)
- [ ] Walkthrough H (changed derivation under a prior correction) as a scenario test; walkthrough C after review (needs Codex, S1.6)
- [ ] Manual check in the app

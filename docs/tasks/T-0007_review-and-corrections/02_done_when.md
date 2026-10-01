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

S2.3: open.

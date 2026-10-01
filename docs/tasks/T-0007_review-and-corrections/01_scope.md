# Scope

- **In S2.1:**
  - range-anchored counting corrections (`confirm_work`, `mark_non_work`) and attribution corrections (`set_target`, `reject_target`);
  - `revoke` for undo;
  - composition per AD-10: the latest correction on the same axis wins for the overlap only;
  - corrections visible with their status;
  - the interval panel and the review list.
- **Not in S2.1:**
  - trim and extend gestures (they are recorded as `mark_non_work` / `confirm_work` on the changed range, see §6.5; the UI gesture comes later);
  - split and merge;
  - allocation and joint shapes;
  - bindings;
  - natural-key anchors for agent subjects (corrections of agent records).
- Corrections of human time never change agent sessions, segments, usage or cost (AT-10, INV-5).

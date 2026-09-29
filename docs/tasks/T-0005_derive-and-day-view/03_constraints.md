# Constraints

- Architecture invariants are unchanged:
  - `derived.sqlite` is disposable and rebuilt only by the lock holder;
  - no global active session;
  - agent time is never human time;
  - unknown is never zero;
  - unidentified usage is never merged;
  - copies are marked, never deleted.
- Real history is used for validation only in scratch data directories. The default data directory stays unused until `tracr run` exists (S1.4). This is the proposed default, not yet confirmed by the user.
- Machine: M1 with 8 GB RAM and about 10 GB free disk. Tauri's first debug build adds several GB to `target/`.
- No push. Commits are scoped per milestone.

# Constraints

- Corrections are user records: `user_action` + `correction` rows, append-only by trigger, never deleted. Undo is a revoke.
- Only the collector holding `tracr.lock` writes them. The app routes corrections through its runtime.
- Disk and live-ledger rules from T-0006 apply.

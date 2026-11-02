# Scope

In scope:

1. **Rule `human-timeline@3`** (`crates/tracr-core/src/human.rs`), new parameter `reply_window_ms` = 5 min. Take two consecutive prompts in any sessions, and the latest agent segment end in any session between them. If the next prompt comes at most 5 min after that end:
   - The whole gap counts as inferred work, with reason `agent_turn_reply`.
   - It is attributed to the session the person waited for.
   - Locked or suspended time stays blank.
   - A degraded or gap extent in the window falls back to the old rule.
   - Every other gap keeps the old rule.
2. **Wiring:** `derive_human.rs` passes the segment ends into `Evidence::agent_ends`.
3. **New record types:** the Claude record types `api-request`, `api-request-blob`, `api-request-shape`, `frame-link`, `fork-context-ref` and `tools-used` are now ignored, the same as `summary` or `mode`. Records of these types already quarantined in the ledger stay as they are (the ledger only grows). They no longer count as degraded evidence (`ledger::degraded_extents(conn, ignored_types)`).

Out of scope:

- Adding UI text for the reason. Reasons are not shown in the UI or CLI today.
- Attributing the new time to projects. It stays "unattributed" the same way as before.

# Done when

- [x] The rule is in place with these unit tests:
  - a reply after 3 min counts, with or without the sensor;
  - a reply after more than 5 min stays unknown, and exactly 5 min counts;
  - a session change counts once, attributed to the session the person waited for;
  - a lock inside the gap is cut out;
  - degraded evidence, or no agent run between the prompts, falls back to the old rule.
- [x] The 6 new record types are ignored (extractor test). Old quarantined records of these types no longer block.
- [x] fmt, clippy `-D warnings` and `cargo test --workspace --exclude tracr-desktop` are green.
- [x] A rebuild of a copy of the real ledger shows more than 0 human minutes on 10-24 to 11-02.
- [ ] Merged and released. The installed app derives the days again (not verified until then).

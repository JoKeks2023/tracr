# Constraints

- **Canon "Agent time is not human time, and unknown is visible but uncounted":** agent minutes stay reported separately and are never added to human time. Human time may now overlap agent time. Replies later than 5 min stay unknown.
- **Canon "tracr is local-first":** no new source is read.
- **Canon "Stored facts are not re-extracted when a record already exists":** old quarantined records stay in the ledger. Only how they are evaluated changes.
- **User decision 2026-11-02**, which differs from architecture §7.3 "backfilled history … is unknown":
  - The whole sequence counts: prompt, then Claude works, then the reply.
  - It applies across sessions.
  - It applies without the device sensor. A prompt shortly after the agent finished is itself evidence that the person was present.

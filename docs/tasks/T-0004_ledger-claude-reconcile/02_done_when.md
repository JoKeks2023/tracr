# Completion criteria (from architecture.md, Implementation Start)

- [x] F main (cursor-rewind fixture: 4 ingested, 1 skipped, non-live trigger) green
- [x] F2 green
- [x] F3 (rotation to a new file identity: 7-minute gap, surviving a crash after batch 1) green
- [x] F4 green
- [x] Trigger tests, including `REPLACE`, green
- [x] A real import of `~/.claude/projects` completes with per-type counts (manually verified, scratch data directory)
- [x] A second import adds 0 provider-source observations (frozen snapshot: +0; live directory: only records appended by running sessions)
- [ ] CI green on Linux, macOS and Windows (`cargo check`): **unverified**, nothing pushed

Status per item: [05_validation_report.md](05_validation_report.md).

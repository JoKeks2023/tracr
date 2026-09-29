# Completion criteria

- `docs/architecture.md` covers all 14 requested topics and ends with "Implementation Start": decisions made, deferred decisions, the exact first task, the first files and modules.
- Stack and persistence are chosen, with a short rationale.
- Parallel sessions, one human timeline, unknown ≠ counted, counting ≠ attribution, durable corrections, immutable evidence and separate money kinds are each enforced by a named mechanism (schema, trigger, type or test), not only by prose.
- Walkthroughs A–H each have an invariant, an owning subsystem, a fixture and the results that must hold, plus the slice in which they pass.
- The first vertical slice uses real provider data, names the first provider, event kinds, stored data, UI, passing tests and deferrals.
- Mermaid diagrams are syntax-checked with the mermaid parser. Relative links and anchors resolve.
- An adversarial review against v0.2 has run, confirmed findings are fixed, and the result is recorded in [05_validation_report.md](05_validation_report.md).
- `research/` is unchanged. Nothing is committed or pushed.

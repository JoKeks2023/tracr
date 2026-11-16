# Validation report

## #32: measured on a copy of the live ledger (2026-11-16, scratch dir, release build)
| | review job |
|---|---|
| before (v3 ledger, derived v6) | 13.4 s, 14.3 s, 24.0 s (debug); 16.9 s, 11.1 s, 11.5 s (release) |
| ledger v4 only | 14.5 s, 7.9 s, 9.7 s |
| ledger v4 + derived v7 | **3.0 s, 4.6 s, 4.4 s** |

Isolated queries on the copy (sqlite3, before → after): copy peers 9.19 s → 0.09–0.32 s, parent links 2.05 s → 0.003 s, degraded 4.10 s → 0.002 s, recovered 1.45 s → 0.001 s.

Not verified: the desktop app itself (only through `Runtime` as the app uses it); the migration on the live ledger.

## #31
Typecheck green. **Not verified in the running UI:** needs a reviewed day plus a later correction, then a look at Arbeitszeit and Heute.

## Checks
`cargo fmt --check`, `cargo clippy --workspace --exclude tracr-desktop --all-targets` (no warnings), `cargo test --workspace --exclude tracr-desktop` (43 suites ok), `pnpm typecheck` ok.

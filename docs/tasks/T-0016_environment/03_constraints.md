# Constraints

- Canon "tracr is local-first": only approved roots are read.
- Read only, never write to the Claude folder. Never open `*.key` files (the sessions folder is not part of this view at all).
- No new dependency (frontmatter is parsed with a small line parser: top-level `key: value`, quoted values, block scalars reduced to their first line).
- Nothing from this view goes into the ledger or the derived store: it is read on demand.
- The window keeps its capability set (AD-12): only tracr's own commands.

# Constraints

- Ledger stays immutable evidence: v3 is additive with a pre-migration backup; `pricing_catalog` is insert-only.
- `derived.sqlite` stays rebuildable: estimates are derived; a catalog change rebuilds.
- Monetary kinds stay distinct: no total across kinds, no `Add` for reported figures, no `total_cost` anywhere.
- Unknown pricing is not zero (CO-02); no substring matching (CO-04).
- Local-first: the catalog is bundled; tracr fetches nothing at runtime.

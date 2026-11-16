# Implementation report

- `crates/tracr-app/src/bindings.rs`: `Subject`, `Binding`, `BindingConfig`, `BindingOp` (bind, end, reject), `apply`, `issue_keys`, `suggestions`; 4 unit tests.
- `App::binding_op`, `Reader::binding_config`, `Collector::binding_op` (applies and rebuilds), `Job::Bindings`, `RuntimeHandle::binding`.
- `derive_human`: new attribution level `binding` between explicit and structural; work pieces cut at binding boundaries (`split_at`); `DerivedWriter::session_native_ids`.
- Day view: `SessionDay.bound_to`, `SessionDay.suggestions`; labels of the `binding` level as for explicit targets (project names resolved when read).
- Desktop: Tauri commands `bindings`, `binding_op`; TS types `Binding`, `BindingOp`, `Suggestion`; `api.bindings`, `api.bindingOp`. **No screens yet.**
- `tests/bindings.rs` (5): suggestion → accept → counts for the target, counted time unchanged; reject survives a rebuild; correction beats binding, end hands back to repository; session vs repository binding → conflicting (counted, no target); a later binding cuts the piece, future start refused.

## Next (needs the Mac)
UI: suggestion chips with Accept/Reject on a session row, a bindings list in Settings or the project page, "bound to X" on the session. Then apply bindings to agent records (usage/cost per project).

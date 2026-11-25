# signal

The pure core of the signal backbone (ADR-0014, ADR-0064): `evaluate` turns one subscription, its unjudged readings and
the time into effects; `apply` moves the subscription by them; `replay` is the dry run; `settle`, `retry`, `redo` and
`skip_firing` are what a failed delivery makes of a firing. Conditions are CEL run by the `cel` crate; `src/check.rs` is
the type check cel-go runs at compile, which the crate lacks.

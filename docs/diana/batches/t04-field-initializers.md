# T04 field initializer presentation

Goal: expose original encoded field values directly at their fields while
retaining original encoded-array ordinals, bytes, ordering and coverage.
Contracts: DESIGN §2.2/§6.5, ALGORITHMS §7, TASKS T04/T05; fixed D046 with
D059–D064 and existing public encoded-value regressions.

D046 exposed an actual observation-shape mismatch: field initializers contained
only ordinal references while the independent field observation contained the
encoded value. The owning native writer will emit the semantic value and keep
its ordinal separately. The public Rust contract and independent comparison
rules remain unchanged; Debug source is an analysis presentation, not a stable
format golden.

`p9_emit/initializers.rs` reads a forward encoded-value cursor across static
fields. It skips any intervening original ordinal exactly once and uses an
explicit container stack for arrays and annotations. Field `initial` now carries
the value; `encoded_static_values_ordinal` stays beside it. The rich original
`static_values` records retain bytes, offsets, value_arg, order and coverage.
Float values retain full-width hexadecimal bits and strings retain UTF16 units.
Failures retain their classification and available prefix; the cursor does not
invent defaults after malformed input. Both metadata passes charge the existing
fallback ledger and check cancellation.

On macOS arm64, the fixed run is
`.cache/e2e/engine/run-pybwguro/report.json` (parent `856ce41` plus this dirty
batch). It selected 12 whole cases and 16 invocations. The 11 regression cases
and 15 invocations passed. D046 now passes its original Debug-fact comparison,
but its whole case still fails because it requires Java and currently returns
Mixed. The owning-writer correction does not count D046 as accepted.

```sh
python3 tools/e2e.py run --case D046 --case D009 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D084
```

Two existing public encoded-value tests also inspect the field value directly,
including scalar extrema, FP bits, literal UTF16 and nested containers. They
retain the original array/ordinal checks; no new test functions were added.

Checks passed: workspace fmt, all-target/all-feature Clippy with warnings denied,
locked release build, 110 existing cargo-nextest core tests with zero skips,
diff checks and sensitive-input audit. The comparator, manifest, independent
expectations and TASKS.md are unchanged. Remaining T acceptance still includes
D046 Java production, wider scalar/control-flow profiles and resource evidence
that depends on executable string lowering.

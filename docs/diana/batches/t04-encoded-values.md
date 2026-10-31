# T04 encoded values

Goal: retain original static encoded values, nested arrays and embedded annotation
values in the public method snapshot. Producers: T02 declaration/type/reference
views and T03 fallback JSON output. Contracts: ALGORITHMS §1.4–§1.5/§7 and DESIGN
§2.3/§3.7. Fixed obligations: F03/M04 and D055/D061/D062/D063. Reuse unchanged
draft-7 B01–B04 preparation and independent expectations in TASKS §3.

Not in scope: annotation directory ownership and Java initializer recovery.

Coverage review: existing readers cover LEB/range failures; snapshot tests cover
JSON output quotas and ordinary cancellation. New core inputs target value bits,
container grammar, local damage and an actual deep container traversal. The
parser uses a fallible explicit stack charged to the existing fallback ledger.
No recursion or numeric default depth limit is introduced.

Source review: AOSP [encoded_value encoding](https://source.android.com/docs/core/runtime/dex-format#encoding)
defines tag/argument widths, signed extension, floating bit padding and annotation
element order. The implementation is independent and copies no reference code.

Changed functions: `EncodedValues::open/next/push` read original values with
borrowed byte ranges, reference checks and a fallible container-depth stack.
Counts are checked against remaining bytes and the available static-field count
before quota charging. Stack capacity is bounded by consumed input bytes, not
a policy limit. `write_values/write_value` retain tags, original encoding bytes,
exact scalar/reference facts and the checked prefix after local damage. Field
ordinals bind to the original static-value array; omitted values are explicit
DEX defaults. These grammar products do not seal initializer types or annotation
ownership.

Red: all four public core tests ran and failed because static values were marked
Unsupported. Green: the same table-driven tests exercise scalar widths, signed
extremes, noncanonical NaN bits, negative zero, UTF16 references, nested annotation
order, omitted defaults, bad counts/tags/indices, partial tails and unsupported
newer tags. A 4096-container input completes without recursive traversal and
rejects an explicit metadata quota and an armed cancellation without publishing
a truncated success.

Independent original-input comparison uses locked Temurin 21.0.8+9, R8 8.3.37 and
dexlib2 2.5.2. The self-made `ValueCase.java` compiles with
`--release 8 -proc:none -g:none`; D8 uses `--debug --min-api 23`. All seven static
field tags and values agree with dexlib2, including NaN, negative zero, signed
extremes, NUL and an isolated surrogate. Evidence under
`.cache/e2e/t04-encoded-values/`: `ValueCase.java`, `classes/`, `dex/`, `logs/`,
`dexlib.json`, `snapshots/` and `comparison.json`. The probe reuses the public
library operation from the previous debug-program batch.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t04_encoded_values)' --status-level fail --final-status-level fail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
git diff --cached --check
python3 tools/audit/desensitize_check.py --range HEAD
python3 tools/audit/desensitize_check.py --staged
```

The closing workspace run passes all 101 core tests without skips, including the
97 existing tests. Format, Clippy, release build, staged diff and sensitive-input
checks pass. Unchanged preparation evidence is reused without fingerprint
verification. T04 annotation-directory ownership, native 041, the complete P1
seal and T05 Engine integration retain their acceptance requirements.

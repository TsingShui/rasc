# T04 normal-path metadata validation

Goal: require the normal method-input producer to validate original debug and
annotation metadata before sealing its raw method product. Contracts: TASKS
T02–T04, DESIGN §3.2–§3.3 and ALGORITHMS §1.3–§1.5. Producers: the existing
checked layout, debug-program and encoded-value readers. Reuse unchanged draft-7
B01–B04 preparation and the existing core cases.

The operator requested implementation of T04 and T05 before E2E integration and
execution. TASKS.md remains unchanged. Implementation checks reuse existing core
coverage; no new unit/integration tests or auxiliary-tool tests are added.
Task-wide E2E acceptance remains pending until the requested later execution.

Implementation: `InputControl::charge_allocation` routes shared parser scratch
to the actual normal or fallback ledger. `DebugProgram` and `EncodedValues` reuse
their existing algorithms on both paths. `RawMethodView` retains its original
MethodId, so `check_method_layout` checks debug parameters, boundaries and local
records under the normal budget without guessing a declaration.

`ProgramIndex::method_input` is the owning producer of `MethodInput`,
`ValidatedRawMethod` and `ValidatedRawDeclaration`; their constructors and input
identity fields are private. Executable products retain the checked layout and
selected annotation associations. The producer checks all explicit references,
including dead sites, and rejects illegal allocation-type and return-opcode
kinds. A declaration-only product requires the original no-code/native/abstract
consistency. Annotation tables and parameter counts are checked, and the existing
iterative grammar checks nested values. Unselected annotation bodies and other
members' code are not read as a condition of validating the selected member.
Existing field delivery still owns field-declaration diagnostics; selecting a
method no longer resolves unrelated field declarations as method prerequisites.
Known code-range failures are returned before later metadata allocation can mask
their classification. These are raw input guarantees, not dispatch/register-type
or Java verification.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
git diff --check
python3 tools/audit/desensitize_check.py --range HEAD
git diff --cached --check
python3 tools/audit/desensitize_check.py --staged
```

All 110 existing core tests pass without skips; format, Clippy and release build
pass. No tests or fixtures were added. The new method-input producer still needs
the T05 public consumer and the later operator-ordered E2E execution; this record
does not claim T04 task-wide acceptance. T05 supplies owned reports/rendering and
the first real declaration/raw consumer rather than empty analysis wrappers.

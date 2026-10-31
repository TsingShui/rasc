# T11 external constant-field read guards

Goal: reject D080's Constant and Unknown variants distinctly before Java field
lowering, retaining the original read/debug facts and introducing no class-init
helper. Contracts: ALGORITHMS §9.1 step 5 and TASKS T11. The adapter selects the
caller-requested certified declaration variant, never an expected result.
Existing static-call, void-effect, primitive and resource cases remain regressions.

P0 now carries exact field identity/access and the exclusive JavaConstantKind
states Constant/NonConstant/Unknown. Missing or duplicate field facts remain
unproven. The normal engine path resolves the original FieldId in its own logical
program and applies this guard only to an external static read. Available local
field ownership is not replaced by an external catalog assertion.

For a static final primitive/String, Constant produces typed
JavaConstantFieldRead and Unknown produces BindingEvidenceMissing. The guard
retains Unsupported/raw and never introduces a helper, extra initialization,
reflection or a fabricated successful Java read. NonConstant does not by itself
admit a field expression; source binding remains a separate obligation.

The thin adapter projects classfile field facts, using Unknown for absent API
constantness proof. For D080 it selects the requested certified explicit field
variant from the invocation input. The Unknown variant withholds the independent
ConstantValue proof rather than leaking the known-constant variant into native
input. It does not derive a fact from the invocation's expected result.

On macOS arm64, `.cache/e2e/engine/run-090ap3v8/report.json` records the complete
mandatory set: 86 cases, 177 invocations and 181 seams retained; 37 whole cases
and 90 seams passed. Both D080 variants passed their distinct typed rejection
and independent complete raw comparison. The command exited 1 for remaining real
capability/application failures. `run-z75m0pmp` separately passed all 11 selected
invocations of D022/D061/D071/D080.

```sh
python3 tools/e2e.py run --case D080 --case D022 --case D071 --case D061
python3 tools/e2e.py run
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing fixed core evidence covered this guard;
no new tests or auxiliary-tool tests were introduced. Fixed inputs, independent
expectations, manifest and TASKS.md remain unchanged. T11's remaining call-kind,
reference overload, access and checked-throws evidence stays pending.

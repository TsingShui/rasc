# T15 instance field ordering

Goal: recover D028 with exact nominal receiver/field binding and distinct ordered
read, arithmetic and write statements. Reuse the existing typed access chain rather
than build another decoder. Original flags, access, unique declaration and constant
behavior remain evidence obligations, including volatile and unused reads.

The existing array access files are renamed heap_access and retain their original
array behavior. The same ordered statement/value chain now admits one exact nominal
parameter type, original int instance fields and int literal addition. P4 seals
read/write use, original FieldId/owner/name/type and known public instance flags.
It rejects final writes, possible constant folding, contradictory modifiers and
incomplete/ambiguous bindings rather than assuming public/final. Available native
owner declarations take precedence over external field evidence, including flags,
absent local declarations and same-name multiplicity. An enum distinguishes those
local facts from an external owner.

P8 independently replays original field references, receiver/value definitions,
flags, use kind, arithmetic and transfers. The receiver's exact source type resolves
the original owner without an added cast. P9 emits separate get/add/put statements
and retains unused get operations; it performs no compound-assignment rewrite or
DCE. The original reference remains in the owned binding, not replaced by its
printable source name. Existing static-call evidence also remains available.

```sh
python3 tools/e2e.py run --case D028 --case D024 --case D026
python3 tools/e2e.py run --case D028 --case D024 --case D026 --case D027 --case D025 --case D073 --case D074 --case D080
python3 tools/e2e.py run
```

On macOS arm64, D028's two required Java invocations pass. Final related
`run-6wwhcmhn` passes all eight selected fixed cases / 20 invocations, retaining the
previous array, fill, cast and external-constant classifications.

Authorized supplemental sources under `.cache/e2e/t15-instance-fields/` make n
volatile and insert an opaque effect between read and write. That effect observes
the old field, writes 100, and optionally throws. Both actual methods compile;
six independent value/event/mutation/exception observations match the reference
JVM, including old-value return, later overwrite with old+3, overflow, null,
failure before the method's write, and an unused read. The actual unused method's
classfile retains getfield, and the independent field declaration has volatile
flags. No volatile instruction is substituted with a helper or constant.

Missing evidence and counterfactual static/nonvolatile flags retain Unsupported/
FieldBindingEvidenceMissing. Separate original Smali proves that an instance
opcode targeting another local class's static field remains Malformed even with
an empty catalog; native duplicate/absent declarations cannot be overridden by a
source catalog claiming one field. Sources, compiler commands/logs, actual
classfile disassembly and observations.json retain this supplemental core evidence;
these are not extra fixed passes or auxiliary-tool tests.

The complete final `run-bi7d0lc8` retains 86 mandatory cases / 177 invocations /
181 seams: 50 whole cases / 116 seams pass. Only D028's two seams are newly passing
relative to run-h1k20ghz; no pass is lost. The full command exits 1 for genuine
remaining obligations. Fmt, strict all-target/all-feature Clippy, locked workspace/
example release builds and all 118 cargo-nextest tests with zero skips pass.
Existing core coverage was reused; TASKS.md, inputs and independent expectations
are unchanged.

T15 remains pending. Next action: preserve static field initialization/read/write
semantics through the corresponding complete declaration and constantness facts.

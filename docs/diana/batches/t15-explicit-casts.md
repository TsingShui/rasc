# T15 explicit reference casts

Goal: recover D025's unused original check-cast as an effectful typed temporary,
not erase it. Reuse initialized Object/String parameter facts, original TypeId,
checked instruction/register ranges and source qualifier evidence. Existing D025
Drivers supply normal/failing core acceptance; aliases and null remain observable
at the original cast positions. T15 broader heap obligations remain separate.

The P3 cast producer handles ordinary static void methods with initialized
Object/String parameters and pure object aliases. A fresh typed value is created
only at the original explicit check-cast TypeId; no type is guessed from a use.
P8 independently replays parameters, words, register definitions, aliases, type
references, cast/result identities and return position. Source qualifier and
annotation/flag checks occur after that replay. The owned AST retains each cast,
including unused casts, and P9 emits them in original order as typed locals.
There is no DCE, extra cast or helper. Unhandled shapes retain classified raw.

```sh
python3 tools/e2e.py run --case D025
python3 tools/e2e.py run
```

On macOS arm64, `run-1guoztz4` passes all three fixed D025 invocations through
real compilation and independent Driver checks, including the unused failing cast
and null behavior. Supplemental authorized Smali under
`.cache/e2e/t15-explicit-casts/` fixes alias moves and three original casts in
sequence, with a separate Java reference. Locked javac/Smali and unchanged D025
scope facts produce an actual method that compiles and matches eight input pairs:
null/string combinations, first versus second failure and opposite invalid input
classes. Full ClassCastException messages match rather than being normalized;
Integer versus Object messages independently reveal failure order. A distinct
undefined original alias remains Malformed/UndefinedRegister even with an empty
catalog, before a missing source qualifier can suppress that hard fact. Compiler
logs, source/actual files, raw inputs and observations.json retain the evidence.
These supplemental observations do not add fixed passes or test auxiliary tools.

The complete final `run-c53evc4a` retains 86 mandatory cases, 177 invocations and
181 seams: 44 whole cases / 101 seams pass. Only D025's three seams are newly
passing relative to run-nscdi4n9, with no lost pass. The complete command exits 1
for remaining genuine failures. Workspace fmt, strict all-target/all-feature
Clippy, locked release and all 118 cargo-nextest core tests pass with zero skips.
Existing native/core coverage was reused; no new tests, fixed expectations or
fingerprint gates were added. TASKS.md is untouched.

T15 remains pending. Next action: preserve D073's zero-count fill length read and
null exception, then add the required FreshArray evidence for D027.

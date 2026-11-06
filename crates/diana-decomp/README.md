# diana-decomp — native engine library

Diana's independent native Rust engine borrows caller-owned DEX bytes. Files,
ZIPs, network access and output ownership belong to the application. Production
forbids unsafe code and external runtime/process dependencies; declarations come
only from the analyzed input.

The active method chain is P0 class facts → method reader → P1 disassembly → P2
execution model → P3 SSA → P4 value/use interpretation → P5 storage/transfer
plans → P6 control/protection/monitor plans. `Engine::analyze_method` and class requests use one method task. The
application decompilation interface is being rebuilt.
The approved architecture is [PIPELINE_DESIGN](../../docs/diana/PIPELINE_DESIGN.md).

P1 retains complete instruction/payload facts, original references, entry layout
and structural associations. P2 owns ordered operations, exact slots, explicit
normal/exception frame/heap/lock completion rules, transfers, exits and coverage.
It does not expose its complete P1 predecessor. Read-only `MethodEvidence` supports
original inspection and checks without product extraction or construction access.
P2–P6 constructors are internal and require Engine's method-task authority.
P3 privately retains P2 and delivers exact-slot states, definitions, ordered use
bindings, necessary Phis, per-transfer incoming states and original StatePoint
mappings. Borrowed `ExecutionFacts` exposes normal execution queries, not a
predecessor product. [P3's contract](../../docs/diana/spec/03-ssa.md) describes
exception-aware LT dominators, live-demand IDF placement and test-only correspondence checks.
Undefined/invalidated sources remain explicit availability facts, not default values.

A P2 stop retains the complete P1 input, classified error and available unvalidated
construction facts. A P3 stop retains complete P2 input and a partial SSA
snapshot; neither stop can be promoted to a successful product. P4 construction
stops likewise retain SSA and a distinct unpublished `TypePartial`. P5 stops retain
P4 and an unpublished `StoragePartial`. P6 stops retain P5 and an unpublished
`RegionPartial`; these snapshots are not mature successors.
Class requests
reserve result storage before starting method work and retain all selected known
member outcomes. A method-task resource/invariant error no longer stops siblings;
`ClassAnalysis.terminated` and its kind-based truncation path are removed.
Range/capacity checks and fallible allocation remain mandatory. The CLI/host owns process termination; Engine has no request
cancellation callback/token or checkpoint parameter. There are no numeric quotas.

The [P1–P9 local-result contract](../../docs/diana/PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果)
requires each stage to preserve independent results within the same method.
P1–P3 now continue through local reference-read/use, register-span, invoke-grouping,
result-use and return-kind issues. Target declarations, grouping and return category
keep separate states; invoke binding reuses P1's resolved prototype rather than
reading declarations again. Unresolved inputs are not zero arguments, and unknown
results create no ValueId. SSA preserves exact-slot scope, exceptional old state,
unknown Phi incoming and recovery after an established overwrite.

This does not establish the entire target: P1 still requires reliable instruction
boundaries, entry layout and control/protection associations. An unreliable graph
stops with original facts instead of producing SSA by dropping edges.

P4 consumes existing SSA/execution facts and retains P3 inside `TypedMethod`.
It delivers value families/type sources, per-use literal/reference/integral views,
Phi incoming adaptations and position-related constructor states. P0's class
declaration now travels inside the method products as one shared immutable
`OwnerFacts` handle, so `this` carries its declared owner type and constructor
calls on `this` distinguish `This`, `Super`, proven same-DEX `Ancestor` and
`OtherOwner`; unresolved owner relationships retain their concrete declaration gap. Typed-catch descriptor
results now travel from the method reader through execution facts, including local
read/use issues. Array fills consume checked payload width/count/bits and select
an element interpretation from existing SSA array sources. Ordinary reference
inputs now use `Direct(Reference)` without proving a class hierarchy. Original
source/use declarations and invocation prototypes stay separate; invocation
identity is not selected from argument types. Machine reads are established before
optional value-preserving declared views. Insufficient range evidence retains the
original read rather than making it unavailable; use declarations do not add
conversions or establish boolean naturalization. Runtime Phi representation now
consumes available literal-input constraints before joining nonliteral groups;
source literals keep their independent views. Initialization follows original SSA
reference bindings and execution positions, including edge-correlated paired
observations; Multiple identity is not an initialization failure by itself. No
general input-context query is implemented. A restricted same-DEX superclass
query now follows original declaration facts without exposing an index or stage
constructor to P4. Successful check-cast bounds now refine array
component knowledge at original execution positions without an SSA write or global
source-declaration rewrite. Actual object-relation uncertainty and unsupported
interpretations remain separate from missing input facts.
The [P4 chapter](../../docs/diana/spec/04-types.md) records its algorithm and guarantees.

P5 privately retains P4 in `StorageMethod`; P6 privately retains that product in
`PlannedMethod`, returned as `MethodOutcome::Planned`.
It maps each SSA value to a parameter, independent local, exact literal or explicit
reference carrier, preserving P4 numeric bit storage without choosing a new family.
Definitions keep their original normal commit/materialization sites. Every Phi
incoming retains its original transfer and adaptation: normal copies execute only
on the selected transfer; exceptional incoming uses before-throw saves and
handler-only receives after the original exception capture. Dependency-indexed
copy scheduling preserves simultaneous-copy requirements and uses matching
temporaries to break cycles. Local gaps follow affected storage dependencies;
independent producers and plan components remain available. The
[P5 chapter](../../docs/diana/spec/05-variables.md) defines its active interface.
Construction plans now select natural groups for established allocation/constructor
correspondence, including private alias publication and compatible snapshot
preludes; other relationships retain explicit operations and concrete reasons.
P6 delivers Block/Sequence/If/Loop/original Switch/LabelledBlock/Try/Synchronized
and explicit Graph regions through the same method chain. Synchronized plans
separate held execution from individual release outlets and retain cleanup control,
handler receives and before/after-release P5 obligations. Normal LT dominators,
finite-exit postdominator candidates and iterative SCC analysis drive composition.
Labels and selected edge copies remain concrete. Ordered throw-site chains,
original captures, P5 construction ownership and explicit monitor separation reasons
survive failed natural composition. The [P6 chapter](../../docs/diana/spec/06-regions.md)
defines the boundary and scope rules. P7–P9 remain inactive.

Independent translation checking compiles only under `cfg(test)`; ordinary
Engine requests do not run symbolic replay, output audits or duplicate dominator
verification. There is no runtime or feature switch. Parsing and construction
still enforce input safety, checked arithmetic and allocation failures.

Test verification compares ordered local symbolic behavior and completion
rules and matches every original transfer/coverage owner. It does not prove ART
admission or concrete external-call behavior. Real-input corpus runs have not
encountered custom/polymorphic invokes or method-handle/type constants. The scoped
`src/dynamic_instructions_test.rs` unit test constructs DEX 038/039 inputs and
checks public Engine products against independently specified references, ordered
slots, result categories and normal/exception completion rules. It also checks
classified stops for unsupported versions and retained local errors for references,
argument layouts and results.
The same test checks register-renaming relations, equivalent wide-copy/constant
encodings, discarded call results, SSA joins/loops/exception/overlap cases, local
recovery inside one method and continued analysis of an executable sibling. P4
cases cover independent zero views, float bits versus numeric conversion, array
components, local use conflicts, Phi/range/grounded-loop facts and constructor
normal versus exceptional initialization.
This is static synthetic-input evidence, not ART execution evidence.
A [Verus trial](proofs/README.md) proves the wide-copy patch predicate used by
test replay, using the same Rust function body. It is absent from production. The adapter and remaining
P2 test checks are not machine-proved; this is not a proof of whole-method correctness.
Simulated P2 capacity/invariant and P3 capacity failures have been observed through
class requests with an executable sibling continuing. These are typed-failure
observations, not actual allocator-failure injection; the latter remains unperformed.

Use the single engine inspection entry from the repository root:

```sh
cargo run -p diana-decomp --release --example dbg_shell --locked -- \
  --dex <classes.dex> --exec '{"cmd":"p2","class":0,"method":0,"details":true}'
```

`p3` (`details:true`) observes state positions, bindings, definitions and Phi
incoming records. `p3-all` reports SSA counts, unavailable uses, upstream failures
and per-method construction failures. Optional `class` selects one original class
through `Engine::analyze`; `analysis_ns` measures that analysis, excluding index
opening, JSON formatting and inactive rendering. `partial_methods`, `local_issues`
and unresolved input counts distinguish ready products from fully established
facts; batch `complete` describes traversal, not semantic verification. P3–P6 are not
formally verified. `p4` (`details:true`) exposes type origins, bits, requirements,
views and initialization paths. `p4-all` adds typed/partial counts, interpretation
problems, array-fill interpretation counts and missing-fact occurrences to the
same traversal. Array-access counts and component gaps remain separate from
unreadable use views. Detailed P4 output includes projected component sources and
gaps, catch facts and array-fill interpretations with original payload bits. These are diagnostic
counts, not semantic verification badges. `p5` (`details:true`) exposes bindings,
original reads/definitions, copy assignments/steps, snapshot routes, constructor
receiver accesses and local gaps. `p5-all` aggregates the same facts, including
unavailable plan components. `p6` (`details:true`) exposes the region arena,
continuations, labels, original links and copy delivery, handler/site ownership,
lock events, release outlets, copy emission ownership and selected operation roles.
`p6-all` aggregates those plans and
construction stops. The timed Engine traversal includes P6, even when a command
inspects a retained earlier product. Each shell command creates a
fresh Engine. `Engine::with_method_jobs(n)` enables a bounded Rayon pool for
multi-member class requests when `n > 1`; the default is one. Single-method calls
keep the same sequential chain. The shell exposes this setting as
`--method-jobs <workers>`. Separate DEX-process concurrency and method-worker
concurrency need joint configuration rather than multiplying both blindly.
`p5-all` with `details:true` also exposes each class member's retained P3/P4/P5
products and the P6 plan from that one class request.

Earlier engine format/check/Clippy and release builds passed for unified component
projection, scoped check bounds and same-DEX ancestor relationships; those changes
added, modified and ran no tests.
The focused observation covered 1,112 methods associated with component gaps or
constructor diagnostics. All 2,224 retained P2/P3 products matched. P4 value/use,
Phi, type-name, array-access, fill and initialization-use products matched.
Constructor relationships and diagnostics changed in exactly the 11 affected
methods; no other selected product changed. Component precision now
comes from original declarations and successful checks at the actual access point,
not a fake cast output or consumer-desired type.

Machine reads remain independent of optional numeric views. Absent int/long ranges
and full-int ranges at boolean-declared uses are not new range evidence; a Z
declaration alone does not authorize boolean naturalization. Literal views retain
their original bits. No runtime conversion or new arithmetic rule was added.

Full traversal of the two APKs in the working directory covered all 55 DEX entries
(41 and 14) and delivered 2,644,578 TypedMethod products. Construction/upstream/class
stops, ordinary unknown reads and representation conflicts were zero. Ten
same-DEX constructor-owner relationships are now Ancestor. The one remaining
recovery diagnostic is Missing(InitializationOwnerRelation), with a concrete
UnknownOwner(MissingDefinition) constructor result. The 3,142 formerly incomplete
component positions now have established bounds; all 523,479 observed array
accesses have complete component knowledge. This includes Copy/Phi and nested
array dependencies, not just direct checked receivers. Null-only alternatives
supply no element type and do not import a different checked array's component
into a non-null path. Complete bounds do not prove non-nullness, an exact runtime
component class or permission to remove the original checks.

The same-DEX constructor-owner query reuses the current declaration and follows
only necessary local ancestor headers. It preserves the original calls to Object
rather than replacing them with an intermediate parent's constructor. A target
reference can establish ancestry without a target definition. The remaining case
requires a parent definition from another DEX; local absence stays Unknown, not a
mismatch, and the query does not search other images. This is declaration-based
initialization evidence, not ART execution or admission evidence. The
[pinned ART source](../../docs/diana/SOURCE_MAP.md#84-additional-mature-projects-pinned-commits-specific-symbols-and-what-not-to-borrow)
records the runtime-source review's separate scope.

Same-DEX ancestor-query timing compared with ae25282 through the same shell:
three warmups and 12 alternating before/after processes for each of six selected
classes. Each process issued 13 class requests with warmed input files. The shell
recreates Engine for every command, so every request had fresh Engine/declaration
caches. First-command median ratios were 1.011–1.053, or approximately 0.5–10.1
microseconds higher; last-ten-command median ratios were 0.904–1.072. Those later
commands measure a warm process, not declaration-cache hits. These observations cover
three ordinary classes, two local-ancestor classes and one missing-local-parent
class. Median process peak RSS did not increase in these samples. They establish
neither universal speed/memory bounds nor a general performance improvement.

The focused single-DEX runs retained all 279 fill interpretations and checked
1,444,571 byte-roundtrip items. The full APK traversal produced 10,212 fill views
without unknown element interpretations, including 28 Short and 1,334 Double
views. Full byte roundtrip checked 48,102,163 items with zero mismatches, skipped
items or method/class errors. These finite observations are not correctness rates
or ART admission evidence. Aggregate counters do not establish branch-specific
coverage of source-unavailable, representation-conflict, nonzero-reference-literal
or out-of-domain literal-view rejection.

Post-Phi construction with multi-origin receivers, opposite allocation selections,
constructor failure/repetition uses, retained older loop allocation instances and
unreadable-direct-call events also lack execution evidence from these runs.
Malformed catch declarations and conflicting/width-mismatched fills retain the
same evidence limitation. Unavailable array sources, ungrounded
component cycles, numeric wide-bit Phi storage and mixed numeric/reference
representation rejection also have static review rather than new execution
evidence from these inputs. The local missing-definition ancestor case was
observed; malformed/ambiguous/cyclic local chains and declaration-lookup allocation
failures were not exercised by this corpus.

Signature-qualified Java-like calls (`Api.register<Listener>(h)`, with a complete
original descriptor when needed) are specified under
[the P7 contract](../../docs/diana/PIPELINE_DESIGN.md#5731-原目标签名限定).
They introduce no operand conversion. P7/P9 source expression/rendering is not yet
active; the current engine delivers the facts needed for that contract.

`p2-all` observes retained execution facts from the same Engine chain and reports
classified outcomes and encountered opcode variants. `roundtrip-all` checks independent byte re-encoding over retained
P1 evidence. These inspection commands do not run the test-only correspondence checks.
No phase-specific debug binary is required.

## P5 observations

The current authorized-input traversal covered 55 DEX files and delivered
2,644,578 StorageMethod products. It retained 29,138,117 value bindings,
44,074,425 use accesses, 3,085,278 Phi incoming records and 12,407 handler/Phi
snapshot locals. Reported storage gaps, partial copy plans, stage stops and class
errors were zero. The original single P4 initialization-owner uncertainty remained;
it was not converted into a storage failure. The 1,112-method focused comparison
retained identical P2, P3 and P4 products, for 3,336 matching observations.

Copy-cycle saves were not encountered. Source-unavailable and representation-gap
storage propagation also lacks execution evidence from this traversal. The
installed scheduling/dependency algorithms and finite counters do not prove these
branches or whole-method behavioral correspondence. Existing byte-roundtrip
evidence was reused because input and P1 were unchanged; no second full chain was
run merely to repeat that check. These are authorized private-input observations,
not repository-reproducible fixtures or correctness rates.

### P5 correspondence tests

The existing scoped core test now checks P5 through public Engine products.
Its original cases retain their independently specified expectations, including
the four-worker class-recovery case. P5 coverage checks also run on those retained
products. The new input variants exhaust runtime/literal choices for two two-way
join values and cover wide transfer, entry backedges, multiple throwing producers,
throw terminals, shared/split handler entries, snapshot fan-out, unavailable
incoming, conflicting views and unresolved wide representation. Finite traces use
fresh definition-occurrence symbols and undefined synthetic storage, not a full
DEX interpreter or P5 construction helpers.

The checks distinguish upstream-to-plan correspondence from simultaneous-copy
versus ordered-step equivalence. A separate slow fixed point checks gap locality
and whether each reported cause can actually reach its consumer. Normal/exception
traces preserve old values, original captures, normal commit, allocation failure
and constructor failure. Both debug and release runs of the sole core test passed.

Eight temporary production mutations were rejected by P5-specific obligations:
wrong storage source, omitted copy steps, wrong transfer identity, snapshot moved
after commit, unknown source replaced by zero, omitted dependent-use gap,
allocation materialized before normal commit and one lost fan-out save. Each
mutation compiled, failed the core test and was removed. This demonstrates
sensitivity to those faults, not a proof of all implementations or paths.

Cross-destination cycles are structurally excluded under the current independent
binding and exact-slot guarantees. The generic cycle/temporary branch still lacks
an Engine-generated execution witness; no private product was fabricated to claim
coverage. The [test contract](../../docs/diana/spec/05-variables.md#test-only-correspondence)
states this boundary. Production input and P1 were unchanged, so prior corpus and
byte-roundtrip observations were reused rather than rerun.

### Natural construction observations

P4 now publishes original allocation-source sets and position-related receiver
comparisons. P5 consumes these facts to select construction groups without
rebuilding reference provenance. Original sites and commits remain facts; selected
deferred publications happen only after normal initialization. The core test
covers direct and aliased creation, precomputed/literal arguments, straight-line
cross-block corridors, both failure stages, compatible snapshot hoisting, differing
snapshots/protection, visible failed carriers and stale allocation occurrences.
Debug/release execution passed. Six targeted mutations of occurrence identity,
publication point, protection, alias coverage, once-only emission and snapshot
merging were rejected and removed.

A 55-DEX P5 traversal delivered the same 2,644,578 code methods with no new stage
stops or storage gaps. It selected 720,360 natural groups and retained 459,253
explicit allocation carriers. Uncertainty in the original initialization-owner
relation stayed unchanged. These observations are not whole-method equivalence or
ART admission evidence.

### Region planning observations

The same core test checks original operation/terminal ownership, selected
control targets and edge copies, before-throw saves, catch order and exactly-once
natural emission. Finite original-graph versus planned-target traces cover
branches, nested regions, while/do-while conditions, repeated producers, exit-side
work, nested loops, a divergent prefix, original switch fallthrough/shared targets,
natural and explicit protection, and successful/failing lock paths. Additional
cases cover internal-header/tail-condition selection, multiple releases and returns,
break/continue corridors, nested captures, divergent held bodies, branched cleanup,
secondary exceptions, handler receives and pre/post-release copies. The independent
checker compares outlet obligations with original P5 locations and preserves
unlocked calls and path-specific copies as explicit when required. Debug and
release execution of the sole core test passed. The model consumes supplied
branch roles and P5 storage; it is not a full VM or a P7/P9 execution test.

Nine temporary P6 outlet mutations compiled and failed the targeted core
obligations: blocked tail selection, missing release outlet, hoisted post-release
copies, lost pre-release save, lost post-release copy, wrong unlocked continuation,
wrong rethrow access, split loop/lock corridor and hidden cleanup escape.
All mutations were removed. These finite checks do not prove every graph, caller,
callee, monitor pattern or resource-failure path.

A 55-DEX traversal delivered 2,644,578 PlannedMethod products without P6 stops or
upstream/class stops. The pre-existing P4 issue remained and storage gaps stayed
zero. It selected 28,695 synchronized regions with 64,777 release outlets, including
28,469 existing-exception rethrow outlets. Natural and explicit plans coexist;
incompatible unlocked continuations and unsatisfied post-release copy obligations
retain complete explicit plans. Recovery diagnostics are not execution-fact
failures or whole-method verification badges. All 4,448 retained P2/P3/P4/P5
products for 1,112 focused methods matched. Serial/four-worker products including
P6 matched for 19 classes and 4,585 members. Input/P1 were unchanged, so prior
byte-roundtrip evidence was retained rather than rerun.

## Method scheduling observations

Rayon workers share immutable input/P0 facts and run the same MethodTask, with
private phase products and scratch. The scheduler reserves selected-member result
storage before dispatch, retains classified member failures and joins tasks before
returning. Pool startup errors retain their typed source and are not cached as an
absence. The [scheduling contract](../../docs/diana/phases.md#current-method-scheduling)
defines startup and local-failure scopes.

A serial/four-worker comparison covered 19 existing focused classes and 4,585
members. Detailed retained P3/P4/P5 products matched. Full 55-DEX traversals under
8-process/1-worker, 4-process/2-worker and 2-process/4-worker configurations retained
identical P5 counters and 2,644,578 StorageMethod products, without storage gaps,
stage stops or class errors. Existing P4 uncertainty remained unchanged.

For six selected large classes, three warmup pairs preceded 12 alternating
serial/four-worker pairs. Analysis-time median ratios were 0.341–0.939; process
wall-time ratios were 0.372–0.940. Every command used a fresh Engine/pool and warm
input files. These finite measurements are not universal speedup bounds.
Whole-batch observed wall times were approximately 32.0, 41.2 and 71.0 seconds for
the respective configurations above, without reused P5 results. Thus this batch
retains the faster eight-process/single-method-worker configuration; internal
method parallelism is available for class requests, not forced on every workload.
These were individual batch observations, not a repeated throughput benchmark.
Actual worker-startup failure and allocation-failure execution remain unobserved.

Applicable engine gates:

```sh
cargo fmt --all -- --check
cargo clippy -p diana-decomp --all-targets --all-features --locked -- -D warnings
cargo build -p diana-decomp --release --locked
cargo nextest run -p diana-decomp --all-features --locked \
  -E 'test(dynamic_instructions_preserve_layout_references_and_completions)'
cargo nextest run -p diana-decomp --release --all-features --locked \
  -E 'test(dynamic_instructions_preserve_layout_references_and_completions)'
```

Dependency, reference-isolation and unit-test policies are owned by root/package
AGENTS.md and [CONTRIBUTING](../../docs/diana/CONTRIBUTING.md). The crate remains
unpublished (`publish=false`, version `0.0.0`).

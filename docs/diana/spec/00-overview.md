# 00 — Product and engine overview

Code: `apps/diana/src/analysis/`, `crates/diana-decomp/src/engine.rs`,
`crates/diana-decomp/src/pipeline/` and `crates/diana-decomp/src/substrate/`.

Diana accepts APK, ZIP and DEX input and exposes class, method and reference
queries. The application owns files, archive handling and presentation; the
engine borrows DEX bytes and caller-supplied declaration facts. The stage map
and the current ordinary-method entry are in [phases.md](../phases.md).

## Single source of truth

The analysed input is Diana's only source of decompilation facts. Diana MUST
derive declarations, references, types, annotations and debug information from
the APK/DEX bytes it was given, together with caller-provided original identity.
Diana MUST NOT require, query or parse external declaration sources such as class
files, `android.jar`, classpaths, symbol databases or network services, and it
MUST NOT accept an external declaration catalog as a condition for output.

Facts the input does not carry are `Unknown`. `Unknown` is not a failure: the
result continues with the available facts and records the uncertainty. A method
or member is denied only when the input cannot be read safely (for example an
out-of-range structure, an unknown instruction width or a truncated code item) or
when the input itself is internally inconsistent in a way that no source form can
express.

Consequences for source recovery:

- Call targets come from the original DEX references and invoke kind.
- Generic signatures come from DEX `Signature` annotations; external library
  generics are rendered erased.
- Access flags, inheritance, overload sets, declared throws and constant-field
  status are known only for definitions inside the input.
- Readable output does not claim compilability or semantic equivalence, and
  strict verification cannot be a gate for producing it.

## Readable result

The product direction is to display available readable reconstruction by default,
including a method whose meaning cannot be fully established. The output MUST
identify material uncertainty and retain original member identity and available
DEX facts. A local failure MUST NOT discard other members. A readable result is
not a claim that source compiles or is semantically equivalent.

P7 retains a renderable `JavaMethod` with concrete recovery diagnostics.
Diana MUST NOT use whole-result Java legality or compilability as an analysis
goal, result status or output gate. Missing analysis or representation products still use original-fact
debug output. Text and JSON present the same underlying report; neither is a
capability switch. Current coverage is maintained in the documentation README.

## Boundary and failure behavior

Input bytes are untrusted. The application and engine MUST preserve checked
ranges, explicit resource limits and cancellation. A malformed or unavailable
input MUST NOT be reported as successful reconstruction. Unknown instruction
widths MUST NOT be guessed; available original facts and coverage remain explicit.
P8 renders existing products and MUST NOT perform another analysis or repair
missing semantics.

## Control and cancellation

open/index, method analysis and render share one `&mut RunContext`, which carries
only cancellation. The context stores no IR, spawns nothing and is not shared
globally; cancellation is checked in every loop pop, instruction, set growth/copy
and long string chunk. Cancellation ends immediately and MUST NOT switch to raw.
Convergence of the analyses is guaranteed by the finite state or single-schedule
measures of each algorithm, and a fixed step count MUST NOT hide an algorithm that
cannot terminate.

Diana exposes no numeric work, allocation, metadata, code-unit, nesting or output
quota. Every allocation checks the platform-representable size and the minimum
input record size, then calls try_reserve; a real allocation failure is reported
explicitly as ResourceExhausted and MUST NOT be turned into format damage. Env,
metadata, UTF16, AST, spans and text all follow this rule. Traversal and release of
untrusted depth use an explicit stack or arena and do not rely on host recursion or
recursive Drop.

Artifacts are dropped after their last consumption, and the Report keeps only the
necessary owned facts rather than the whole SSA/Flow history graph. Output is
checked against the actual escaped UTF8 increment as it is produced; the engine
MUST NOT first build an unbounded String and then claim the work was bounded.
Performance records come from authorized release runs and do not claim a fixed
time/RSS upper bound for every input.

## 1. Product boundary and engineering scope

Diana is a native APK/DEX decompiler that by default delivers the real code it can
recover; a failure in one method does not block the display of other methods. Java
readability, independently established behavior and whether one request
completed normally are distinct judgments. Diana MUST favor useful readable
reconstruction while preserving original facts and making uncertainty explicit.
A local failure MUST NOT discard other members.

- The application Module lives in `apps/diana/` and owns input files, snapshots,
  ZIP, lightweight queries and output. The engine Module lives in
  `crates/diana-decomp/`, borrows DEX bytes and declaration facts, and MUST NOT
  access files, the network or external processes. The code is native Rust,
  Rust>=1.93, edition2024, and both crates forbid unsafe.
- The root workspace contains only those two crates. Diana MUST NOT depend on the
  archived reference; historical reference paths are resolved against their pinned
upstream URL and commit
  locally, while the public source commit and path stay unchanged.
- Input, source, code-unit and UTF16 coordinates are independent; Diana MUST
  range-check the offset, count and arithmetic of every input before any slice or
  allocation.
- Independent results, source text and synthetic names have no required stable
  display order; Diana MUST NOT add a sorting or arrival-waiting reorder buffer.
  Diana MUST return every match from a query, with no offset, limit or cursor.
  Diana MUST preserve the original instruction, evaluation, argument, effect,
  catch, lock and init order unchanged.
- Diana MUST expose available readable reconstruction with its uncertainty, or
  return Debug or Unavailable when reconstruction is not available. Diana MUST
  NOT fabricate a successful method with an empty body or exception stub.

Application source lives in `apps/diana/src/` and engine source in
`crates/diana-decomp/src/`. Reference material lives in `.cache/references/`.

## 2. Success and degradation are separated in the type system

#### 2.1 Observable semantics

On a matching dependency environment and valid program input, successful Java
recovery MUST preserve:

- Return values, including integer overflow, FP NaN, signed zero and
  bit-pattern-dependent operations; termination and loop behavior.
- Field, array and call order, the receiver and the original identity of the
  member, the dispatch kind, and class initialization triggering.
- Exception types, throw and handler selection order, the visible effect of
  finally, and the register state **before the failure** of the throwing
  instruction.
- The object, count, order and exceptional path of monitor acquisition and
  release; the declared synchronized semantics.
- String UTF-16 code units, including lone surrogates; static encoded values and
  their initialization order.

Source positions, local names, the number of temporaries and stack-frame shape are
not commitments. Diana MUST NOT describe implementation-dependent behavior of
Java/JVM and ART, such as the exact OOME timing, stack-trace line numbers or GC
timing, as fully equivalent; the algorithm MUST NOT use that reason to cross
potentially throwing, allocating or synchronizing operations arbitrarily, and a
validation report MUST state the limitations of the run environment. These are semantic goals, not a prerequisite that every readable result passes
all native checks. Diana MUST identify missing evidence and known uncertainty;
readability alone does not establish semantic equivalence.

#### 2.2 Result model

```rust
pub struct DexImage<'a> { pub bytes: &'a [u8], pub header_offset: usize, pub origin: ImageOrigin }
pub struct RunContext<'c> { /* private cancellation callback and shared worker flag */ }
// RunContext::new(is_cancelled: &'c dyn Fn() -> bool) -> Self
pub struct Engine<'a> { /* immutable ProgramIndex<'a> + request configuration; the bytes are held by the root scope */ }
pub struct Options { pub recovery: RecoveryPolicy }
pub struct Request { pub class: ClassKey, pub method: Option<MethodSelector> }
// MethodSelector = { name, proto }; a local MethodKey exists only after resolution, and a method is not selected by name alone.
pub struct Report {
    pub class: ClassOutput,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: RunStats,
}
pub enum MethodOutput {
    /// Readable Java. `body` distinguishes a recovered body from a declaration
    /// that has no body in the original DEX (abstract, native, interface method).
    Java { source: String, body: JavaBodyKind, spans: Vec<OriginSpan>, uncertainties: Vec<Diagnostic> },
    Debug { source: String, spans: Vec<OriginSpan>, reason: Diagnostic },
    Unavailable { reason: Diagnostic, excerpt: Option<RawExcerpt> },
}
pub enum JavaBodyKind { Code, Declaration }
pub enum ClassOutput {
    JavaUnit { source: String, members: Vec<MethodOutput> },
    Mixed { shell: DeclarationOutput, members: Vec<MethodOutput> },
}
pub enum DeclarationOutput { Java(JavaClassDecl), Debug(String), Unavailable(Diagnostic) }
```

Debug source is an analysis-oriented snapshot of original DEX facts, rendered as
readable JSON under [08-emission](08-emission.md). It is not Java, standard Smali or a reassembly
promise. It preserves the selected method identity, typed reason, original
instructions/targets/payloads/handlers/references and explicit coverage. Text and
JSON expose the same information at the failed member without another request.
No assembler or alternative engine belongs to this exit.

The internal library uses the following interfaces, and cancellation is wired before method analysis rather than deferred to it:

```rust
Engine::open(images: Vec<DexImage<'a>>, options: Options,
             control: &mut RunContext<'_>)
    -> Result<Engine<'a>, InputError>;
Engine::decompile(&self, request: Request, control: &mut RunContext<'_>)
    -> Result<Report, RequestError>;
render(report: &Report, control: &mut RunContext<'_>)
    -> Result<RenderedReport, RequestError>;
```

The application MUST create one RunContext per request and pass it in sequence through read/index, analysis and printing, and MUST NOT rebuild the context and retry in order to bypass a limit. The Engine MUST NOT store the callback or a mutable control, and it needs no global state or scheduler; open and render likewise MUST check cancellation and return Cancelled/ResourceExhausted, and the engine MUST NOT misreport them as Malformed. Report owns the declarations, literal UTF16, original identity and evidence summaries that the result requires and MUST NOT borrow a temporary SSA/Env; render MUST NOT require the whole graph to be retained, and it MUST NOT re-analyze or start another recovery route because output failed.

Caller-supplied external declarations are not the product path. The ordinary
request derives class and member facts for every definition in the input and
treats owners outside the input as unknown; Diana does not accept external declarations as a verification policy. A single DEX uses an images vector of length 1; DexIds are allocated within the request and map back to the root physical/logical identity through ImageOrigin, different arrival orders are not required to produce the same numbers, and no extra sorting is done for file names or input order. The catalog MAY be empty; insufficient type/call evidence remains explicit in the local result. The library MUST NOT access disk, processes or the network, and the root request scope holds the byte owner until Engine use ends; an ordinary DEX has header_offset=0; T04 implements native 041 reading, H04 only performs application borrowing and original-identity adaptation, and T25/H04 provide the D065 evidence for their respective sides.

Readable Java MUST identify material unresolved assumptions or failed checks and preserve original method identity, available DEX debug facts and coverage. Its producer MUST use the existing shared analysis chain and available checked input facts; it MUST NOT fabricate instruction widths or claim unknown declaration, type or control-flow facts as proven. If a useful reconstruction cannot be formed, the engine MUST retain Debug or Unavailable. P8 renders the produced result without further analysis. Readable output does not suppress Malformed, ResourceExhausted, Cancelled or InternalInvariant classification.

P7 produces readable Java from the shared analysis products and records material uncertainties. Internal checks preserve input safety and analysis/transform invariants; they do not qualify output as legal Java. The engine MUST NOT present unknown facts as proven or call javac; independent compilation and behavior checks provide evidence only for their tested environments.

Generated members retain original identities and MUST NOT be duplicated in owner syntax. If a member cannot be expressed independently, Diana MUST present its available readable reconstruction or original facts without inventing a wrapper.

A class-shaped result is not a guarantee of compilation or equivalence. Members retain their original identities and uncertainty markers; an unavailable shell MUST NOT hide available members. A declaration without a body is permitted only when the original member has no code; Diana MUST NOT fabricate a body to fill a gap. File suffixes MUST NOT determine eligibility.

#### 2.3 Failure kinds and cancellation

| Category | Example | Handling |
|---|---|---|
| `Malformed` | out-of-range ID, truncated opcode, wrong branch boundary, invalid LEB, use of an undefined register | If a global header/catalog cannot be located, RequestError; a locally invalid method → Unavailable, optionally with an explicit raw excerpt. This is not an ordinary Unsupported, and available original facts/bytes remain diagnostic information, not verified Java or a valid-program claim |
| `Unsupported` | known legal invoke-custom, constructor or monitor/exception structure whose operations cannot yet be represented | retain checked original instructions, references and coverage as an analysis-oriented debug snapshot; if decoding cannot continue safely, keep available facts/bytes and identify the missing range |
| `ResourceExhausted` | an actual allocation or platform-capacity check fails, or a bounded queue/worker cannot be created | stop that method's analysis; the raw exit still reads only checked original facts if that remains possible, otherwise Unavailable; there is no numeric quota and the engine MUST NOT retry with an increased bound |
| `InternalInvariant` | identity inconsistency before and after the verifier, certificate missing an edge, algorithm-impossible state | the diagnostic MUST state BUG explicitly and the dev test MUST fail; the request report MAY keep a method-level raw fallback, but the status remains an internal error and MUST NOT count as a normal Unsupported or a Java success |
| `Cancelled` | the caller's cancellation token | stop the request with RequestError::Cancelled; the engine MUST NOT continue producing per-method debug output after cancellation |

The engine retains an available readable reconstruction as Java together with its original identity and explicit uncertainty reasons. Java output is not a claim of compilability or semantic equivalence. The engine retains other completed members independently. Debug information reads only checked original bytes/facts, not the failed Java reconstruction. It includes the original identity, typed reason, decoded instructions and related control/reference facts, with explicit undecoded ranges. Unknown width permits only checked prefix facts and original byte/word excerpts; Diana MUST NOT guess instruction width or claim executable/reassemblable source. A local failure does not change the configuration of other methods.

##### Diagnostic contract

| Field | Meaning |
|---|---|
| `code` | Typed reason identifier, independent of human-readable detail |
| `kind` | One failure kind from the table above |
| `severity` | `warning` for a local Unsupported; `error` for a hard failure, including a BUG with raw attached |
| `method` / `site` | Optional original MethodKey and CodeUnit; a site implies its method, and absence does not mean offset 0 |
| `phase` | Optional finite PhaseId identifying the producer; no pass registry or traversal history |
| `detail` | Owned explanation; it does not carry an entire graph, input or unbounded trace |

Class-level reasons retain their original ClassKey in the enclosing result. Input
and request errors use the same kind/code distinction. A reason does not claim a
site when global damage prevents locating it. Typed codes already named in
[output reasons](07-java.md), such as BindingEvidenceMissing, ArrayFillShape and
MonitorScopeUnproved, retain those meanings; add a code only with an actual
failure producer, not a speculative enumeration of every possible rejection.

The application MUST compute request success from the failure kinds and delivery
state under §6.8, not from severity, the presence of raw, or a single code-to-exit
lookup. A local Unsupported is distinct from a global unsupported input. The
acceptance adapter compares these typed facts against independent expected facts; it MUST NOT
parse `detail` or stderr to infer the failure category.

#### 3.1 Actual Modules and files

A phase is an ownership/responsibility split and does not require one file per responsibility. Diana MUST keep `mod.rs` focused on module declarations and re-exports; substantial algorithms, data types and errors belong in named implementation files. Diana MUST create those files only for an actual producer or consumer, and MUST NOT create empty passes or generic Context placeholders.

| Module | First actual files / main functions | Owner |
|---|---|---|
| Engine/input | engine.rs, p1_input/{mod,cursor,header,catalog,code}.rs; open/index/method_input | T01/T02/T04 |
| Raw format | p1_input/formats.rs, p2_flow/decode.rs; decode/validate_raw | T03/T04 |
| Flow | p2_flow/{mod,build,effects,results,protection}.rs; decode_and_bind | T06/T16 |
| SSA | p3_ssa/{mod,inputs,liveness,build,verify}.rs; build_ssa | T06/T07/T16 |
| Calls | p4_types/declarations/{mod,members,allocation,contracts}.rs; resolve_calls | T11/T12 |
| Types | p4_types/{mod,constraints,lattice,solve,conversions}.rs; solve_types | T06/T10 |
| Storage | p5_variables/{mod,locals,copies,exceptional,definitions}.rs; allocate_and_copy | T08/T17 |
| Regions | p6_regions/{mod,normal,protection,monitors,verify}.rs; identify_regions | T09/T13/T18/T19 |
| Java | p7_java/{mod,expressions,dispatcher,structured,declarations,initializers,ownership,enums,verify}.rs; lower_method/assemble_class | the respective core capability cards |
| Emit | p8_emit/{mod,java,debug,literals,spans}.rs; render | T03/T06 |
| Control | pipeline/{control,diagnostics,request}.rs; direct typed calls and cancellation checks | T01 |
| Facts | context/{mod,catalog,input_facts}.rs; cross-DEX lookup and lazy cache of P1 declaration facts | T01/T11 |
| Data | substrate/{ids,origin,types,cfg,java}.rs; only the containers/IDs currently used | created together with the actual consumer |

Diana MUST NOT create registry.rs, a Pass trait, a scheduling macro, a build.rs generator or a restart framework. An optional trace MUST use only a finite PhaseId and bounded events, and Diana MUST NOT give it its own test suite or platform.

#### 3.2 Public interfaces and lifetime

The application MUST create one context per request and use it for the whole chain, and MUST NOT build another context for a retry; the Engine MUST NOT store a mutable control. The loops in open/index/render MUST be cancellable as well. InputError/RequestError carry a unified FailureKind, and the engine MUST NOT wrap cancellation or resources as Malformed. The callback is a cooperative check and does not promise preemption of an external codec or a hard timeout.

The application MUST allocate DexIds by enumerating this request's images, and they map to the original ImageOrigin; original identity is the origin plus the original table index/proto, not an output array position. Input admission MUST count the 041 views of the same physical slice once; each logical catalog MUST be independent. The same origin supplied by the caller MUST correspond to the same original bytes/header, and the engine MUST reject a conflicting request.

Report is an owned result: the literal UTF16 in the Java AST, the engine MUST copy or intern the declarations used and the original identity into ResultFacts; Report MUST NOT borrow an Env/Phi arena and MUST NOT carry the whole analysis graph. render only needs to read ResultFacts and does not need a live Engine. RenderedReport owns the UTF8 text and spans; JSON, files and the CLI are the application's responsibility. ResultFacts retains the used DexId-to-ImageOrigin mapping and each emitted original declaration's class/member index and descriptor/name/proto. The application derives §6.3 locators from those facts; it MUST NOT reopen files, retain an Engine or guess an index from result order to render identity. Library-owned origin tokens are caller-supplied, not host file paths.

The input helper shared by the engine and the actual lightweight scanner is
`raw::instruction_layout(code_bytes: &[u8], pc_cu: u32, version) -> Result<InsnLayout, RawDecodeError>`:
`InsnLayout { width_cu: u32, refs: [Option<IndexedRef>; 2] }`, where ref contains kind/index/operand_slot.
The helper MUST check only the format, padding and the complete word/payload range, and MUST NOT claim whole-method/index/branch validation; the caller charges its loop and checks its own catalog. The implementation MUST share one OpcodeSpec with the internal decoder and MUST NOT expose SSA or private phases.

#### 3.3 Internal products: moved by value, without copying historical graphs

```text
RawMethodView { key, checked code/try/debug ranges }
MethodSource { key, raw code or declaration-only, declaration, flags, metadata state }
DecodedMethod { raw_view, structural layout, instruction/payload directory, references }
ClassFacts { class declaration, four-group member directory, static preparation, metadata }
FlowBody { raw, cfg, origins }
SsaBody { flow, values, phis, bound_uses }
ResolvedBody { ssa, bindings, allocations }
TypedBody { resolved, value_types, use_conversions }
StorageBody { typed, locals, copies, definition_evidence }
PlannedBody { storage, regions, protection, monitors }
```

raw borrows the original scope; each stage MUST consume the previous owned product. The engine MUST release temporary live/Env/solver worklists at their last consumption point; they need not stay attached to the wrapper permanently. The implementation MUST NOT drop a table early while evidence still references it; after P7 finishes validation, the engine MUST copy the result-required facts into the owned output and MAY release the whole PlannedBody. The implementation MUST NOT use an all-graph Arc<Mutex<_>> or a second mutable MethodArena.

##### 3.3.1 IDs, slots and original origin

```text
DexId/ClassKey/MethodKey/FieldKey       original declaration handle within the request
InsnId/BlockId/EdgeId/ExceptionEdgeId   same-method arena handle, never reused
ValueId/PhiId/LocalId                   definition/Phi/source storage, not equal to a physical reg
Slot { start: u16, width: One|Two }
Site { method: MethodKey, code_unit: u32 }
Origin { sites: Vec<Site>, synthetic_reason: Option<Reason> }
```

The engine MUST check arena growth capacity before allocation; exhausting the u32 ID space is ResourceExhausted, and the allocator MUST NOT wrap. Diana MUST NOT mix UTF16, input byte and source UTF8 coordinates. Debug names are hints only, and they MUST NOT decide member identity, slot or type.

##### 3.3.2 FlowGraph

```text
Block { ops: Vec<OpId>, terminator: Terminator }
Op { raw_opcode, inputs: Vec<Slot>, normal_defs: Vec<Slot>, effect, origin }
Terminator = Jump(EdgeId) | Branch(Condition, true_edge, false_edge)
           | Switch(ValueUse, Vec<(i32, EdgeId)>, default_edge)
           | Return(optional use) | Throw(use)
Edge { src, dst, kind }
ExceptionEdge { throw_site, handler_target, chain_id, rank, catch_type }
```

P2 is the only topology-editing phase; it MUST place a protected may-throw last and MUST keep normal and exception edges separate. A producer and its move-result share the semantic Op but retain the original two sites, and a payload MUST NOT become an Op. The entry also has an EntrySeed incoming, which is not real jumpable code. An unvalidated EndInvalid MUST NOT enter SsaBody.

##### 3.3.3 SSA fixed-entry Phi

```text
ValueDef = Parameter(slot, descriptor) | This
         | OpResult(op, slot) | PhiResult(phi) | ExceptionAt(handler)
Sym = Def(ValueId) | Phi(PhiId) | Bad(Undefined|PartialWide)
Phi { block, slot, result: ValueId, inputs: Vec<(IncomingEdge, Sym)> }
IncomingEdge = EntrySeed | Normal(EdgeId) | Exceptional(ExceptionEdgeId)
Env = sparse map<Slot, Sym>             // builder scratch, not part of the final SsaBody
```

[03-ssa](03-ssa.md) applies: the builder grows live-in only, gives every block live-in a Phi, binds each block once, connects each incoming once, and marks retained/bad/grounded each once and monotonically. The builder MUST represent a not-yet-filled construction entry as an Option, which is not the semantic Undefined. The builder MUST NOT do trivial-Phi or alias reduction, and MUST NOT propagate a ValueId until it "looks unchanged".

The verifier MUST check that each Op input is an entry Phi of this block or an already executed normal Def, and that each Phi input equals the corresponding predecessor exit/pre-throw slot binding. The verifier MUST keep a missing definition or partial wide separate from a type that cannot be expressed. The verifier MUST NOT let an ordinary dominator substitute for the exception check.

##### 3.3.4 Types, binding and allocation

```text
ValueFacts { width, family_group, producer_origins, integral_bits, integral_range, required_uses }
UseConversion { use_site, target_source_type, kind, supporting_fact }
CallEvidence { original_ref, invoke_kind, candidate_group, completeness,
               parameter_words, access/throws/init facts, binding_obligations }
Allocation { id, new_site, init_site, aliases, use_availability }
FillPlan = LengthOnly | FreshArrayStores(shape_evidence, exact_elements)
```

Type solving first closes the descriptor facts, family equality groups and monotonic value facts, then selects the source type once;
P4 MUST allow ProvenNarrow only when the producer range falls entirely inside the B/S/C target range, and MUST NOT change the word value through an arbitrary primitive cast. P7 MUST generate Java for fill only by the LengthOnly/FreshArrayStores rules of [02-flow](02-flow.md); the fill MUST register may-throw, P7 MUST NOT let a zero-element fill vanish without a statement, and a non-empty expansion MUST prove that the stores cannot fail and that the intermediate writes are unobservable.
P5 MUST receive the body only once every final use conversion is defined; the printer MAY spell a purely numeric literal per use,
while the type solver MUST NOT guess a non-literal as a cast between int/float/ref. What P4 hands to P4 is constraints, and only P7 checks the final Java overload/
dispatch/throws; Diana MUST NOT impose a circular requirement that P4 already know the source static types.

#### 3.7 RunContext and cancellation implementation

`RunContext` carries only a cancellation callback, a latched cancelled state and,
for scoped workers, a shared cancellation flag. It owns no IR, no limits and no
cumulative resource ledger.

```rust
pub struct RunContext<'c> { /* private cancellation callback and shared flag */ }
// RunContext::new(is_cancelled: &'c dyn Fn() -> bool) -> Self
// RunContext::new_parallel(callback: &'c (dyn Fn() -> bool + Sync)) -> Self
```

Every decode, graph, slot-set/constraint change, copy, subtype and output loop
checks cancellation at its natural boundaries. String and buffer processing uses
fixed chunks so a long unit cannot postpone cancellation indefinitely. Scoped
Rayon workers share the callback and a latched flag; joining finishes before the
request checks cancellation for publication.

Container growth checks the platform-representable size and calls `try_reserve`.
An actual allocation failure is reported as a classified error and is not
converted into format damage. Cancellation does not switch to the raw exit.

## 9. Implementation trade-offs

Diana MUST NOT wholesale adopt advanced ValueRecovery/GVN/LICM, an all-platform catalog, an SMT/prover, automatic retry or globally optimal structuring. The order is fixed-entry Phi and independent locals first, then restricted dispatcher/natural structure, and finally exceptions, locks and complete classes. If an independent counterexample refutes an algorithm, the plan MUST be corrected rather than silently switching route or changing the expected values. Reference provenance is maintained only in SOURCE_MAP.

## 10. Resources, policy and invalidation

#### 10.1 Direct scheduling and optional inlining

pipeline/schedule.rs MUST call the typed transitions of §4 directly; local liveness/type closure is done by an ordinary loop inside its Module,
and Diana MUST NOT build a Pass trait, registry, scheduling macro, TOML or restart tree. PhaseId serves only as a bounded diagnostic label.

RecoveryPolicy controls normal structure and inline_pure. normal structure defaults to
StructuredThenDispatch; DispatchOnly MUST apply only to dispatcher eligibility. inline_pure defaults to
false, and when it is true the single pure-expression inlining algorithm of [07-java](07-java.md) runs. The required lowering, type/binding/
protection-domain/resource checks have no off switch.

#### 10.2 Explicit invalidation scope

| Change | Retained | Recomputed/invalidated |
|---|---|---|
| literal formatting/indent/output names | every verified artifact | P8 text/spans (if a name change affects Java binding, P8 alone is not enough) |
| normal structured vs dispatcher policy | P1–P5 | P6/P7/P8; only the plan is selected, the CFG is not changed |
| original declaration/hierarchy facts | unchanged checked bytes | a new input creates a new Engine and rebuilds the shared method chain |
| CFG topology/exception chain | only raw/catalog | everything downstream of P2 is invalidated; the engine MUST NOT reuse old ValueIds/CopyPlans |
| generic/nested source policy | the value identity reusable by raw/P2–P3 | binding/types/storage/Java after P4; the whole method is recomputed |

The engine MUST re-run the whole method for a new request and MUST NOT cache mutable analysis products; the Engine retains only the immutable ProgramIndex. Requests are not required to share numbers or a cross-run canonical fingerprint, and Diana MUST NOT sort fact tables for cache identity. A local worklist modifies only the specified finite facts and enqueues only on an actual change, charging before execution. There is no generic Repeat/Changed scheduling protocol, and Diana MUST NOT create a mechanical test suite for observability.

#### 10.3 Resource control and allocation failure

Diana exposes no numeric work, allocation, metadata, code-unit, nesting or output
quota, and no default threshold is substituted for one. The engine keeps no
cumulative work/allocation ledger and does not report such usage.

Removing quotas does not remove safety:

- Every input read checks offsets, lengths, counts and arithmetic before slicing.
- Container growth checks platform-representable capacity and calls `try_reserve`.
- An actual allocation failure is reported explicitly as a classified resource
  failure and MUST NOT be turned into Malformed input.
- Cancellation, bounded queues and publication atomicity remain required.
- Traversal of untrusted depth uses an explicit stack or arena and MUST NOT rely
  on host recursion.
- The CLI MUST NOT truncate an over-long result into success, because no caller
  may request a partial result through a numeric bound.

## Semantic goals

**Rules** define the algorithms; **derivations** state their correctness preconditions.
Independent execution evidence MUST NOT be replaced by a derivation.
The arguments below are not machine-checked proofs. They describe conditions for
semantic preservation, not a prerequisite for showing readable source. When a
condition or a declaration fact is unknown, Diana MUST retain the uncertainty and
MAY display the reconstruction without claiming that condition holds. Input
safety, classified failures and cancellation/allocation obligations remain binding.

### Objects and states

| Notation | Definition | MUST NOT be conflated with |
|---|---|---|
| `N` / `pc` | physical byte count / code-unit position within the method | `FileByte`, `CodeUnit`, UTF16 and source UTF8 byte |
| `I,B,E,X` | original instruction count, reachable block count, normal edge count, exception edge count | different edges to the same target are still different choices |
| `R,S,P,A` | register words, requested Slot set, retained Phi count, total Phi incoming count | a register is not a ValueId or LocalId |
| `sigma` | runtime original register store, a wide value being two related words | undefined is not 0, uninit is not null |
| `lambda` | typed locals store of the generated program | an initial value added by DA is not a semantic definition |
| `H,K` | heap / current lock stack | a local copy MUST NOT pretend to be a heap operation or a lock operation |
| `tau` | ordered observable trace of calls, reads and writes, exceptions, synchronization and initialization | the display order of an independent analysis report does not belong to tau |

A ValueId is a **static definition site**, not a runtime value that stays constant across loop
iterations. A proof MAY write `value(v, activation)` to distinguish the instances that one
Op/Phi produces on two block entries; the implementation does not store an activation
counter. After each entry Phi assignment or Op execution, lambda represents the current
instance rather than the set of historical instances.
This is also why §11 checks, beyond the same root, that the root producer does not re-execute
while the lock is held.

### Preservation relation and applicability preconditions

The block-entry relation `Rel(b,sigma,lambda)` holds when, for every live-in slot of b, the
word value of its Phi local interpreted by the source type equals sigma, and the Defs
already executed earlier in this block also correspond to the current instance. The observable
parts of the applied heap and locks agree, and the original event prefix tau agrees.
Non-live slots, unread DA initial values and pc/temp are not required to be equal.
A pure literal is interpreted through a chosen view of its original bits; it is not required
to have a local.

The goal is: every original normal or exception step is simulated by finitely many Java
steps; added steps only read and write unobservable locals; the same edge is selected, the
same observable events are produced, and the subsequent Rel holds. The reverse direction
MUST NOT add observable paths either; the verifier MUST prove that the illegal-pc branch of
the dispatcher and the false branch of the static-initialization `if(true)` are unreachable.
Finite copy and scheduling steps MUST NOT stutter indefinitely and make an originally
terminating method diverge. A finite Driver provides evidence for the tested inputs only
and does not replace this argument.

The preconditions include a matching declaration and linking environment, the selected Java
semantics profile, and no implementation-related VM resource failure being triggered.
Asynchronous exceptions and actual OOM/GC/stack-frame differences lie outside the
per-instruction simulation guarantee of this document; that exclusion does not authorize
moving an original allocation, may-throw, volatile access, lock or class-init. Synchronous
exceptions and the original protection domains are still preserved site by site. Evidence
from JVM21 does not automatically become a guarantee for historical JVMs or for all ART
behavior; in particular, a single-thread trace MUST NOT be called a complete JMM proof.

### Decision trade-offs

| Decision | Reason for choosing it | Explicitly not adopted |
|---|---|---|
| original addresses + borrowed bytes | one coordinate system verifies original identity and range, and 041 shared data stays undistorted | rewrite the header and then call the copy raw truth |
| split the CFG after atoms | leader rules cannot split a producer/result into a wrong definition timing | expose a successful result to catch |
| fixed entry Phi | separates the identity problem from information propagation and yields an easy finite measure | rewrite aliases and Phi alternately until they “look unchanged” |
| choose the family first, then fix the source type | separates bit interpretation, nominal type and use-conversion duties | one Unknown variable doing join, guessed cast and printed type at the same time |
| independent locals + parallel copy | sacrifices compactness to avoid coalescing interference and lost old values | reuse a Java local per physical reg |
| explicit transfer and lexical scope | copy, catch and lock can be checked per edge and per site | coverage count equals a semantic proof |

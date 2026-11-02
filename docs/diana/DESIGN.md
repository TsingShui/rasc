# Diana product and engine technical specification

This document defines product behavior, Module responsibilities, interfaces, data
structures and resource rules. See [ALGORITHMS.md](ALGORITHMS.md) for algorithms
and state transitions, and [TASKS.md](TASKS.md) for tasks and validation.

## 1. Product boundary and engineering scope

Diana is a native APK/DEX decompiler that by default delivers the real code it can
recover; a failure in one method does not block the display of other methods. Java
recovery eligibility, complete JavaUnit eligibility and whether one request
completed normally are three distinct judgements. Mixed/Debug is the normal exit
for a local failure; Diana MUST NOT change its product positioning to
diagnostic-only, and Diana MUST NOT use the permission to degrade to abandon Java
recovery quality or to turn required Java acceptance into a pass.

- The application Module lives in `apps/diana/` and owns input files, snapshots,
  ZIP, lightweight queries and output. The engine Module lives in
  `crates/diana-decomp/`, borrows DEX bytes and declaration facts, and MUST NOT
  access files, the network or external processes. The code is native Rust,
  Rust>=1.93, edition2024, and both crates forbid unsafe.
- The root workspace contains only those two crates. Diana MUST NOT depend on the
  reference source `ref-fork/rasc/`; historical reference paths carry that prefix
  locally, while the public source commit and path stay unchanged.
- Input, source, code-unit and UTF16 coordinates are independent; Diana MUST
  range- and budget-check the offset and count of every input before any slice or
  allocation.
- Independent results, source text and synthetic names have no required stable
  display order; Diana MUST NOT add a sorting or arrival-waiting reorder buffer.
  Diana MUST return every match from a query, with no offset, limit or cursor.
  Diana MUST preserve the original instruction, evaluation, argument, effect,
  catch, lock and init order unchanged.
- Diana MUST NOT make correctness configurable off. Where the Java representation
  constraints cannot be satisfied, the engine MUST return Debug or Unavailable and
  MUST NOT fabricate a success with an exception stub.

Application source lives in `apps/diana/src/` and engine source in
`crates/diana-decomp/src/`. Shared self-made inputs and expected values live in
`tests/e2e/`, generated artifacts in `.cache/e2e/`, and reference material in
`.cache/references/`.

## 2. Success and degradation are separated in the type system

### 2.1 Observable semantics

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
validation report MUST state the limitations of the run environment. Java success
requires that every value, binding, storage, control, protection-domain and
declaration check defined in this document passes; Diana MUST NOT change the
contract dynamically with the current implementation capability.

### 2.2 Result model

```rust
pub struct DexImage<'a> { pub bytes: &'a [u8], pub header_offset: usize, pub origin: ImageOrigin }
pub struct RunContext<'c> { /* private Limits + cumulative ledger + cancellation callback */ }
// RunContext::new(limits: Limits, is_cancelled: &'c dyn Fn() -> bool) -> Self
pub struct Engine<'a> { /* immutable ProgramIndex<'a> + request configuration; the bytes are held by the root scope */ }
pub struct Options { pub recovery: RecoveryPolicy } // Limits are held only by RunContext
pub struct Request { pub class: ClassKey, pub method: Option<MethodSelector> }
// MethodSelector = { name, proto }; a local MethodKey exists only after resolution, and a method is not selected by name alone.
pub struct Report {
    pub class: ClassOutput,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: RunStats,
}
pub enum MethodOutput {
    Java { declaration: VerifiedJavaMethod, evidence: MethodEvidence },
    DeclarationOnly { declaration: JavaMethodDecl }, // abstract/native, declaration only
    Debug { source: String, spans: Vec<OriginSpan>, reason: Diagnostic },
    Unavailable { reason: Diagnostic, excerpt: Option<RawExcerpt> },
}
pub enum ClassOutput {
    JavaUnit { unit: VerifiedJavaUnit, members: Vec<MethodOutput> },
    Mixed { shell: DeclarationOutput, members: Vec<MethodOutput> },
}
pub enum DeclarationOutput { Java(JavaClassDecl), Debug(String), Unavailable(Diagnostic) }
```

Debug source is an analysis-oriented snapshot of original DEX facts, rendered as
readable JSON under ALGORITHMS §7. It is not Java, standard Smali or a reassembly
promise. It preserves the selected method identity, typed reason, original
instructions/targets/payloads/handlers/references and explicit coverage. Text and
JSON expose the same information at the failed member without another request.
No assembler or alternative engine belongs to this exit.

The internal library uses the following interfaces, and cancellation is wired before method analysis rather than deferred to it:

```rust
Engine::open(images: Vec<DexImage<'a>>, options: Options,
             catalog: DeclarationCatalog, control: &mut RunContext<'_>)
    -> Result<Engine<'a>, InputError>;
Engine::decompile(&self, request: Request, control: &mut RunContext<'_>)
    -> Result<Report, RequestError>;
render(report: &Report, control: &mut RunContext<'_>)
    -> Result<RenderedReport, RequestError>;
```

The application MUST create one RunContext per request and pass it in sequence through read/index, analysis and printing, and MUST NOT rebuild the context and retry in order to bypass a limit. The Engine MUST NOT store the callback or a mutable control, and it needs no global state or scheduler; open and render likewise MUST check cancellation and return Cancelled/BudgetExceeded, and the engine MUST NOT misreport them as Malformed. Diana MUST NOT store Limits again in Options/Engine. Report owns the declarations, literal UTF16, original identity and evidence summaries that the result requires and MUST NOT borrow a temporary SSA/Env; render MUST NOT require the whole graph to be retained, and it MUST NOT re-analyze or start another recovery route because output failed.

A single DEX uses an images vector of length 1; DexIds are allocated within the request and map back to the root physical/logical identity through ImageOrigin, different arrival orders are not required to produce the same numbers, and no extra sorting is done for file names or input order. The catalog MAY be empty; insufficient type/call evidence degrades locally instead of guessing. The library MUST NOT access disk, processes or the network, and the root request scope holds the byte owner until Engine use ends; an ordinary DEX has header_offset=0; T04 implements native 041 reading, H04 only performs application borrowing and original-identity adaptation, and T25/H04 provide the D065 evidence for their respective sides.

The `VerifiedJavaMethod/Unit` fields are private, and the P8 verifier MUST be the only constructor, after the semantics, source rules and directly determinable Java8 target checks pass, see ALGORITHMS §12.4–§12.6; DeclarationOnly likewise checks the declaration boundary. The P8 verifier MUST NOT reject Java by guessing the code, locals, stack, handler or pool size from the AST node count, and an unknown compilation size is not a failure reason. Verified means that those native checks have completed; it does not mean production has compiled the code or proven the final classfile size; actual compilation and classfile format validation belong to V4 acceptance. The evidence MUST record EnvironmentId and the fixed compilation contract; **the production engine MUST NOT call javac**. The external environment MUST satisfy the declaration facts, and a test MUST use a real matching classpath; Diana MUST NOT treat Unknown as Complete.

`VerifiedJavaMethod` internally uses `MemberEmission::Explicit | Generated(owner_node, rule)`: Generated is used for members that have code and are proven generated by enum/override/inner syntax, and does not count as DeclarationOnly. The unit printer MUST print only the owner syntax and MUST NOT emit conflicting declarations such as bridge/values a second time. The unit printer MUST map original MethodKeys to owner spans. When a standalone request cannot express a Generated method independently, the engine MUST return the original method as Debug and MUST NOT fabricate a wrapper, see ALGORITHMS §12.

`JavaUnit` requires a valid class shell, every member being Java (including verified Generated) or DeclarationOnly, and the binding/initialization/constructor constraints passing. If one method is Debug/Unavailable or the shell cannot be expressed, the result is Mixed; this is a result eligibility, not a condition for refusing display by default. The CLI MUST retain the successful methods, the failure location, the original identity and the Debug/unavailable reason in a Mixed presentation, and MUST state at the top that it is not a complete Java compilation unit; it MUST NOT fill gaps with an empty body or an exception stub. Diana MUST NOT let the file suffix decide eligibility and MUST NOT automatically choose a `.java` file name for Mixed; it MUST NOT reject an explicit user `-o` because of its suffix, and MUST still retain the marker. Method-level Java MAY exist inside Mixed, while compilation validation MUST still require a legal context; Diana MUST NOT change dependencies to fabricate a stub.

### 2.3 Failure kinds and cancellation

| Category | Example | Handling |
|---|---|---|
| `Malformed` | out-of-range ID, truncated opcode, wrong branch boundary, invalid LEB, use of an undefined register | If a global header/catalog cannot be located, RequestError; a locally invalid method → Unavailable, optionally with an explicit raw excerpt. This is not an ordinary Unsupported, and available original facts/bytes remain diagnostic information, not verified Java or a valid-program claim |
| `Unsupported` | known legal invoke-custom, constructor that cannot be legalized, monitor/exception structure that cannot be proven, missing external declaration evidence | retain checked original instructions, references and coverage as an analysis-oriented debug snapshot; if decoding cannot continue safely, keep available facts/bytes and identify the missing range |
| `BudgetExceeded` | a resource limit explicitly configured by the caller is hit, or an actual allocation/platform capacity fails | stop that method's analysis; if resources are still available for raw, print it completely, otherwise Unavailable; there is no numeric quota by default and the engine MUST NOT retry with an increased limit |
| `InternalInvariant` | identity inconsistency before and after the verifier, certificate missing an edge, algorithm-impossible state | the diagnostic MUST state BUG explicitly and the dev test MUST fail; the request report MAY keep a method-level raw fallback, but the status remains an internal error and MUST NOT count as a normal Unsupported or a Java success |
| `Cancelled` | the caller's cancellation token | stop the request with RequestError::Cancelled; the engine MUST NOT continue producing per-method debug output after cancellation |

The engine discards the unverified Java AST/text of a failed method and retains the other completed members. Debug information reads only checked original bytes/facts, not the failed Java reconstruction. It includes the original identity, typed reason, decoded instructions and related control/reference facts, with explicit undecoded ranges. Unknown width permits only checked prefix facts and original byte/word excerpts; Diana MUST NOT guess instruction width or claim executable/reassemblable source. A local failure does not change the configuration of other methods.

#### Diagnostic contract

| Field | Meaning |
|---|---|
| `code` | Typed reason identifier, independent of human-readable detail |
| `kind` | One failure kind from the table above |
| `severity` | `warning` for a local Unsupported; `error` for a hard failure, including a BUG with raw attached |
| `method` / `site` | Optional original MethodKey and CodeUnit; a site implies its method, and absence does not mean offset 0 |
| `phase` | Optional finite PhaseId identifying the producer; no pass registry or traversal history |
| `detail` | Owned explanation charged to the request; it does not carry an entire graph, input or unbounded trace |

Class-level reasons retain their original ClassKey in the enclosing result. Input
and request errors use the same kind/code distinction. A reason does not claim a
site when global damage prevents locating it. Typed codes already named in
ALGORITHMS, such as BindingEvidenceMissing, ArrayFillShape, JavaTargetLimit and
MonitorScopeUnproved, retain those meanings; add a code only with an actual
failure producer, not a speculative enumeration of every possible rejection.

The application MUST compute request success from the failure kinds and delivery
state under §6.8, not from severity, the presence of raw, or a single code-to-exit
lookup. A local Unsupported is distinct from a global unsupported input. The
acceptance adapter compares these typed facts under TASKS §4.3.5; it MUST NOT
parse `detail` or stderr to infer the failure category.

## 3. Modules and data

The three seams are input, method analysis and printing; Diana MUST NOT expose a private CFG/Phi/AST. Diana MUST create only the files and containers that the current implementation actually consumes.

### 3.1 Actual Modules and files

A phase is an ownership/responsibility split and does not require one file per responsibility. Diana MUST keep `mod.rs` focused on module declarations and re-exports; substantial algorithms, data types and errors belong in named implementation files. Diana MUST create those files only for an actual producer or consumer, and MUST NOT create empty passes or generic Context placeholders.

| Module | First actual files / main functions | Owner |
|---|---|---|
| Engine/input | engine.rs, p1_input/{mod,cursor,header,catalog,code}.rs; open/index/method_input | T01/T02/T04 |
| Raw format | p1_input/formats.rs, p2_flow/decode.rs; decode/validate_raw | T03/T04 |
| Flow | p2_flow/{mod,build,effects,results,protection}.rs; decode_and_bind | T06/T16 |
| SSA | p3_ssa/{mod,inputs,liveness,build,verify}.rs; build_ssa | T06/T07/T16 |
| Calls | p4_calls/{mod,members,allocation,contracts}.rs; resolve_calls | T11/T12 |
| Types | p5_types/{mod,constraints,lattice,solve,conversions}.rs; solve_types | T06/T10 |
| Storage | p6_variables/{mod,locals,copies,exceptional,definitions}.rs; allocate_and_copy | T08/T17 |
| Regions | p7_regions/{mod,normal,protection,monitors,verify}.rs; identify_regions | T09/T13/T18/T19 |
| Java | p8_java/{mod,expressions,dispatcher,structured,declarations,initializers,ownership,enums,verify}.rs; lower_method/assemble_class | the respective core capability cards |
| Emit | p9_emit/{mod,java,debug,literals,spans}.rs; render | T03/T06 |
| Control | pipeline/{schedule,control,diagnostics}.rs; direct typed calls and quota checks | T01 |
| Facts | p0_knowledge/{mod,declarations,policy}.rs; immutable catalog/source policy | T01/T11 |
| Data | substrate/{ids,origin,types,cfg,java}.rs; only the containers/IDs currently used | created together with the actual consumer |

Diana MUST NOT create registry.rs, a Pass trait, a scheduling macro, a build.rs generator or a restart framework. An optional trace MUST use only a finite PhaseId and bounded events, and Diana MUST NOT give it its own test suite or platform. tests/support MUST connect only to existing public interfaces and the root tests/e2e assets; a core test MUST be split into files by behavior and MUST NOT construct goldens from private phase invocation counts.

### 3.2 Public interfaces and lifetime

The application MUST create one context per request and use it for the whole chain, and MUST NOT build another context for a retry; the Engine MUST NOT store a mutable control. The loops in open/index/render MUST be cancellable as well. InputError/RequestError carry a unified FailureKind, and the engine MUST NOT wrap cancellation or resources as Malformed. The callback is a cooperative check and does not promise preemption of an external codec or a hard timeout.

The application MUST allocate DexIds by enumerating this request's images, and they map to the original ImageOrigin; original identity is the origin plus the original table index/proto, not an output array position. Input admission MUST count the 041 views of the same physical slice once; each logical catalog MUST be independent. The same origin supplied by the caller MUST correspond to the same original bytes/header, and the engine MUST reject a conflicting request.

Report is an owned result: the literal UTF16 in the Java AST, the engine MUST copy or intern the declarations used and the original identity into ResultFacts and charge them against the same request's quota; Report MUST NOT borrow an Env/Phi arena and MUST NOT carry the whole analysis graph. render only needs to read ResultFacts and does not need a live Engine. RenderedReport owns the UTF8 text and spans; JSON, files and the CLI are the application's responsibility. ResultFacts retains the used DexId-to-ImageOrigin mapping and each emitted original declaration's class/member index and descriptor/name/proto. The application derives §6.3 locators from those facts; it MUST NOT reopen files, retain an Engine or guess an index from result order to render identity. Library-owned origin tokens are caller-supplied, not host file paths.

The input helper shared by the engine and the actual lightweight scanner is
`raw::instruction_layout(code_bytes: &[u8], pc_cu: u32, version) -> Result<InsnLayout, RawDecodeError>`:
`InsnLayout { width_cu: u32, refs: [Option<IndexedRef>; 2] }`, where ref contains kind/index/operand_slot.
The helper MUST check only the format, padding and the complete word/payload range, and MUST NOT claim whole-method/index/branch validation; the caller charges its loop and checks its own catalog. The implementation MUST share one OpcodeSpec with the internal decoder and MUST NOT expose SSA or private phases.

### 3.3 Internal products: moved by value, without copying historical graphs

```text
RawMethodView { key, checked code/try/debug ranges }
RawPrintable = ValidatedRawMethod | ValidatedRawDeclaration
ValidatedRawMethod { raw_view, raw_insns, payloads, checked metadata }
ValidatedRawDeclaration { key, checked flags/proto/annotations, code_off: Zero }
MethodInput { key, raw, decl }
FlowBody { raw, cfg, origins }
SsaBody { flow, values, phis, bound_uses }
ResolvedBody { ssa, bindings, allocations }
TypedBody { resolved, value_types, use_conversions }
StorageBody { typed, locals, copies, definition_evidence }
PlannedBody { storage, regions, protection, monitors }
```

raw borrows the original scope; each stage MUST consume the previous owned product. The engine MUST release temporary live/Env/solver worklists at their last consumption point; they need not stay attached to the wrapper permanently. The implementation MUST NOT drop a table early while evidence still references it; after P8 finishes validation, the engine MUST copy the result-required facts into the owned output and MAY release the whole PlannedBody. The implementation MUST NOT use an all-graph Arc<Mutex<_>> or a second mutable MethodArena.

#### 3.3.1 IDs, slots and original origin

```text
DexId/ClassKey/MethodKey/FieldKey       original declaration handle within the request
InsnId/BlockId/EdgeId/ExceptionEdgeId   same-method arena handle, never reused
ValueId/PhiId/LocalId                   definition/Phi/source storage, not equal to a physical reg
Slot { start: u16, width: One|Two }
Site { method: MethodKey, code_unit: u32 }
Origin { sites: Vec<Site>, synthetic_reason: Option<Reason> }
```

The engine MUST charge every arena growth first; exhausting the u32 ID space is BudgetExceeded, and the allocator MUST NOT wrap. Diana MUST NOT mix UTF16, input byte and source UTF8 coordinates. Debug names are hints only, and they MUST NOT decide member identity, slot or type.

#### 3.3.2 FlowGraph

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

#### 3.3.3 SSA fixed-entry Phi

```text
ValueDef = Parameter(slot, descriptor) | This
         | OpResult(op, slot) | PhiResult(phi) | ExceptionAt(handler)
Sym = Def(ValueId) | Phi(PhiId) | Bad(Undefined|PartialWide)
Phi { block, slot, result: ValueId, inputs: Vec<(IncomingEdge, Sym)> }
IncomingEdge = EntrySeed | Normal(EdgeId) | Exceptional(ExceptionEdgeId)
Env = sparse map<Slot, Sym>             // builder scratch, not part of the final SsaBody
```

ALGORITHMS §3 applies: the builder grows live-in only, gives every block live-in a Phi, binds each block once, connects each incoming once, and marks retained/bad/grounded each once and monotonically. The builder MUST represent a not-yet-filled construction entry as an Option, which is not the semantic Undefined. The builder MUST NOT do trivial-Phi or alias reduction, and MUST NOT propagate a ValueId until it "looks unchanged".

The verifier MUST check that each Op input is an entry Phi of this block or an already executed normal Def, and that each Phi input equals the corresponding predecessor exit/pre-throw slot binding. The verifier MUST keep a missing definition or partial wide separate from a type that cannot be expressed. The verifier MUST NOT let an ordinary dominator substitute for the exception check.

#### 3.3.4 Types, binding and allocation

```text
ValueFacts { width, family_group, producer_origins, integral_bits, integral_range, required_uses }
UseConversion { use_site, target_source_type, kind, supporting_fact }
CallEvidence { original_ref, invoke_kind, candidate_group, completeness,
               parameter_words, access/throws/init facts, binding_obligations }
Allocation { id, new_site, init_site, aliases, use_availability }
FillPlan = LengthOnly | FreshArrayStores(shape_evidence, exact_elements)
```

Type solving first closes the descriptor facts, family equality groups and monotonic value facts, then selects the source type once;
P5 MUST allow ProvenNarrow only when the producer range falls entirely inside the B/S/C target range, and MUST NOT change the word value through an arbitrary primitive cast. P8 MUST generate Java for fill only by the LengthOnly/FreshArrayStores rules of ALGORITHMS §2.4; the fill MUST register may-throw, P8 MUST NOT let a zero-element fill vanish without a statement, and a non-empty expansion MUST prove that the stores cannot fail and that the intermediate writes are unobservable.
P6 MUST receive the body only once every final use conversion is defined; the printer MAY spell a purely numeric literal per use,
while the type solver MUST NOT guess a non-literal as a cast between int/float/ref. What P4 hands to P5 is constraints, and only P8 checks the final Java overload/
dispatch/throws; Diana MUST NOT impose a circular requirement that P4 already know the source static types.

### 3.4 DeclarationCatalog and minimal call binding

The concrete immutable data tables are TypeDecl, MethodDecl, FieldDecl and HierarchyEdge. FieldDecl additionally records
`JavaConstantKind=Constant|NonConstant|Unknown`, which prevents sget from being constant-inlined by javac; a raw encoded
value does not by itself prove the constant attribute of the external source. Each owner/candidate
group has Complete/Partial/Unknown plus its source. An empty catalog is legal, and missing evidence affects only the corresponding Java capability; the catalog MUST NOT read files,
query the network, spawn or scrape an all-platform database. EnvironmentId identifies only this request's facts snapshot, and Diana MUST NOT cache content across requests.

```text
lookup_type(TypeRef) -> Fact<TypeDecl>
lookup_field(FieldRef) -> Fact<FieldDecl>
lookup_method(MethodRef) -> Fact<MethodDecl>
overloads(owner, name, invoke_kind) -> {candidates, completeness}
subtype(a,b) -> ProvenYes|ProvenNo|Unknown
```

subtype traverses the known parent/interface graph with a visited set and an explicit queue, and charges loops/depth/candidate count; it MUST return ProvenNo only when the complete ancestor set was exhausted without a hit, and it MUST report a missing node as Unknown. overloads includes the facts required for inheritance/hiding and access; P4 MUST NOT let a candidate traversal replace the Java selection with the first arriving item.

P8 uses fixed-arity calls with exact formal static types, and it MUST allow only known reference/
primitive widening as candidate applicability, without introducing boxing or varargs expansion. P8 MUST require the applicable set to contain a unique most-specific erased
target that agrees with the original invoke resolution. P8 MUST NOT guess generic inference, return-only collision or unknown parent contracts
in this profile. See ALGORITHMS §9 for the complete steps.

### 3.5 Storage and lexical plan

```text
CopyPlan {
  entry: ParallelCopySet,
  normal: EdgeId -> ParallelCopySet,
  before_throw: InsnId -> Vec<ShadowSnapshot>,
  ingress: HandlerEntry -> Vec<LocalCopy>,
  temps: Vec<LocalDecl>
}
LocalBinding = Parameter | OrdinaryLocal | InlineLiteral | DeferredAllocation(AllocationId)
ShadowSnapshot { handler, phi, dst_shadow, src_local_or_literal, exact_type }
TransferPlan { original_edge, copies, target, abrupt_kind }
RegionPlan { owned_blocks, owned_edges, nodes, entry, follow }
ProtectionPlan { owner_try, ordered_catches, site_to_catch_stack }
MonitorPlan { enter_site, lock_identity, body, normal_exits, release_handler }
```

P6 MUST select the edge before executing the copies; the exception snapshot MUST sit immediately before the original throw, and a catch parameter MUST bind ExceptionAt before the shadow is copied. An independent replay of the original uses/entry copies/transfer checks that the default local is unobservable, and the verifier MUST NOT trust a boolean certificate. P7 MUST NOT change the CFG and MUST NOT copy effects for natural source. P7 MUST reduce try from inner to outer and validate the actual catch-stack, and it MUST replace synchronized with a monitor only after the finite lock stack and every exit are validated. The concrete matching rules belong to ALGORITHMS, and the printer MUST NOT fill in a structure that does not satisfy them or widen the protection domain.

### 3.6 Java results, Generated and printing

```text
VerifiedJavaMethod { private method_decl, body_or_generated, ResultFacts, evidence, target_admission }
TargetAdmission { parameter_words, utf8_lengths, member_counts, compiler_owned_layout }
// stores only directly verifiable facts; there is no field that predicts compilation cost such as code/pool.
JavaMemberKind = Method | Constructor | StaticInitializer
MemberEmission = Explicit(JavaBody) | Generated(owner_node, canonical_rule)
VerifiedJavaUnit { private class_decl, explicit_members, generated_member_evidence }
Expr { source_type, origin, kind }
Stmt = Decl|Assign|ExprStmt|If|While|Switch|Break|Continue|Return|Throw|Try|Synchronized
```

`Generated` still has its original executable method and is merely generated by enum/override/inner syntax, so it is not DeclarationOnly.
P8 MUST match the canonical behavior of ALGORITHMS §12 and MUST NOT delete a member based on a missing synthetic marker alone. When a standalone request cannot express a
Generated method independently, the engine MUST return the original Debug. JavaUnit requires every member or its generation rule and the whole shell to pass validation;
Diana MAY display Mixed but MUST NOT let it masquerade as a complete compilable unit. Only the owning validator MAY call the private Verified constructors.

P8 MUST emit only a static block for a StaticInitializer and MUST NOT emit a method named <clinit>; return-void and certainly-abrupt
Java legalization follow ALGORITHMS §12.2, and the printer MUST NOT guess control flow on a temporary basis.

P9 is a pure printer and MUST NOT solve types or structure; its input is already sealed, but it MUST still check budget/cancellation per segment. Java expressions use the
precedence table: primary/postfix→unary/cast→mul→add→shift→relational→equality→bitwise
and/xor/or→logical and/or→conditional→assignment. The printer MUST parenthesize an operand below the parent precedence; at equal precedence the
right operand keeps parentheses for sub/div/rem/shift and for non-associative operations, and the printer MUST NOT reassociate FP. A simple implementation MAY add more parentheses.
The printer MUST limit statement-expression to call/new/update/assignment and MUST emit a bare array/field/arithmetic read as a typed assignment.

An OriginSpan is a half-open interval in the final UTF8 text; the printer MUST record the byte cursor before each write and the origin after the node completes.
The printer MUST relocate class/unit prefixes uniformly when it assembles them; Generated maps to its owner syntax without losing the original key. A local name
only needs a per-request unique v-prefix, and the printer MUST NOT do name recovery or stable textual sorting.

### 3.7 RunContext and budget implementation

Limits belong only to RunContext. Public methods are called in sequence; the internal RunControl borrows `&mut RunContext` and adds the current
method sub-quota. The implementation MUST NOT need RefCell/Mutex, a global context or self-referential structures.

```text
Quota { limit: Option<u64>, used, held }
claim(n):        checked used+held+n; check <=limit only when Some(limit); held+=n
spend(n):        checked used+held+n; check <=limit only when Some(limit); used+=n
spend_held(n):   held-=n; used+=n          # only for the reservation that was already claimed
release_unspent: held-=remaining         # not a refund of consumed work/allocation
```

By default every limit is None, so there is no numeric quota and no fixed raw reservation. When a limit is configured explicitly, the engine MUST check the
request and the applicable sub-quota separately for the same operation, and MUST NOT add them together as if they were two copies of memory. The normal and raw work sub-quotas are independent and both count toward the request;
the method allocation limit covers the normal/raw total. The engine MUST claim only explicitly finite fallback work/allocation values from the parent quota, and
normal MUST NOT consume held; the engine MUST NOT claim an "unlimited reservation" for a None limit, and raw shares the request's remaining resources with no additional reserve promised.
A MethodBudget drop only returns unconsumed held, and it MUST NOT roll back used. A failed explicit reservation MUST NOT start normal analysis; the engine MUST report a resource Unavailable for a locatable
method, and MUST return RequestError if even the report does not fit.
The engine MUST propagate Cancelled to the request immediately at the checkpoint and MUST NOT generate raw per method.

The engine MUST charge allocation by capacity increment: Vec by delta_capacity*sizeof(T), strings by encoded bytes, HashMap/
sets with a conservative bucket/control overhead; the allocator MUST call try_reserve only after the check. The engine MUST keep input admission and derived allocation separate, and MUST NOT count resident memory twice for the
same caller-owned slice borrow. This ledger is a cumulative logical upper bound, and Diana MUST NOT claim it is exact RSS.

Every decode, graph, slot-set/constraint change, copy, subtype and output loop MUST call charge/check_cancel;
Diana MUST process long strings in fixed chunks, and trace/diagnostic output MUST NOT bypass the limits. The configured limits and their accounting MUST use DESIGN §10.3
as the single table, and Diana MUST NOT generate a second registry. render MUST emit only the complete result and MUST NOT re-analyze or retry on error.

## 4. Phases, products and actual order

The directory layout uses the P0–P9 numbering familiar from Kuna, but Diana defines its own semantics and execution.

| Phase / Module | Owned products | Input and responsibility | Does not own |
|---|---|---|---|
| P0 `p0_knowledge` | `RequestFacts`, configuration snapshot, `EnvironmentId` | DEX/catalog fact sources, run profile, trace/catalog metadata | does not generate bytecode facts, does not modify the IR, holds no persistent learning engine |
| P1 `p1_input` | `ProgramIndex`, `MethodInput`, `RawMethodView` → `ValidatedRawMethod` | safe container and metadata, UTF-16, raw coverage of the version/opcode format; range checking is graded separately from complete structure validation | does not decide control structure, does not reject a whole legal class because the source is hard to express |
| P2 `p2_flow` | `FlowBody` | decode, move-result binding, blocks, normal/ordered exceptional edges, effect sites | does not optimize effects, does not structure, does not speculate new call targets |
| P3 `p3_ssa` | `SsaBody` | input parameters/this, a stable ValueId per definition, normal/exception edge environments, Phi | does not merge source locals, MUST NOT eliminate exception edges |
| P4 `p4_calls` | `ResolvedBody`, `CallEvidence` | original owner/proto/kind, declaration candidate/access/throws/class-init facts and the Java binding obligations still to be validated; allocation/init constraints | does not restore native calling conventions; MUST NOT become an empty phase merely because the DEX has a proto |
| P5 `p5_types` | `TypedBody` | primitive/reference/null/raw constant bits, use conversions, Phi join, finite monotonic work queue | does not have to rebuild complete generic source; does not modify CFG topology |
| P6 `p6_variables` | `StorageBody`, `CopyPlan` | one LocalId per definition, no general coalescing; parallel copies on normal edges, exception shadow slots; DA certificate | does not infer lexical structure, MUST NOT put SSA construction in a Repeat that regenerates identities |
| P7 `p7_regions` | `RegionPlan`, `ProtectionCertificate` | analyzes normal SCC/SESE, try ranges/handlers and monitors from a read-only StorageBody | does not modify the original CFG, MUST NOT treat catch-all automatically as finally |
| P8 `p8_java` | `VerifiedJavaMethod/Unit` | exact block/edge coverage, structured/dispatcher, expression legalization, class/constructor/initializer and throws validation | does not guess unparsed bytes, MUST NOT skip a block merely because of a visited set |
| P9 `p9_emit` | `RenderedReport`, final UTF-8 `OriginSpan` | AST printer, original-fact debug renderer, diagnostic renderer | does not do value/type/control-flow recovery |

**Scheduling order**:

```text
ProgramIndex(P1) ──→ RequestFacts(P0, frozen)
class request: fix owner/MemberEmission first (P8, direct template proof from P1 raw + declaration facts)
per Explicit method: ValidatedRawMethod(P1)
  → decode_and_bind(P2) → verify_flow
  → build_ssa(P3)       → verify_ssa
  → resolve_calls(P4)   → verify_declaration_evidence
  → solve_types(P5)     → verify_types
  → allocate_and_copy(P6) → verify_storage
  → identify_regions(P7)
  → lower_java(P8)      → verify_method
Generated member: original code + template evidence → owner syntax and target admission (no independent fake Java body)
class assembly(P8)     → verify_unit_or_mark_mixed
render(P9)
any classified failure ──→ original method + available resources (explicit raw quota if any) ──→ debug / unavailable(P9)
```

The only fixed points are the P3 live-in requirement and the P5 finite-facts closure; P3 uses fixed-entry Phi, one binding per block, one connection per incoming and a finite bad/grounded propagation, and it MUST NOT repeatedly guess ValueIds/aliases. Those fixed points only collect finite facts and MUST NOT create new identities in the loop. The P2 topo normalization MUST collect edits first and apply them once atomically. Pure expression inlining MUST run in P8 per ALGORITHMS §15 and MUST NOT change the SSA/CFG. **Diana MUST NOT put all of P3–P6 into a mutual fixed point**, and MUST NOT force every phase to have the same number of passes.

Rationale: the ValueRecovery of dexdec depends on a structured Semantic IR [DX-01]; this design does not adopt it and keeps an explicit three-address computation that solves SSA/type/storage on the CFG first, which avoids repeated mutual scheduling of BDD/gated Phi/language syntax. This scheduling does not require DEX to have a natural phase order; explicit locals/copies increase output length in exchange for local verifiability of value provenance and transfer.

### 4.1 Stage interface authority

The scheduler owns analysis-construction order; a phase owns its algorithms and
validated products. Reading an earlier product is distinct from starting its
analysis again. A read-only wrapper prevents mutation, not unauthorized analysis
or interpretation in the wrong phase.

- Contributors MUST keep phase implementation modules private and expose only
  the stage entries, products and read-only queries needed by actual consumers.
  Contributors MUST NOT use convenience re-exports to expose private stage
  implementations through the scheduler or another phase.
- Stage interfaces MUST restrict analysis-construction entry points to
  scheduler-owned authority. If an entry uses a scheduling credential, its thin
  entry wrapper MUST NOT forward that credential into the algorithm
  implementation. Phase implementations MUST NOT invoke another phase's
  analysis-construction entry point.
- Stage interfaces MUST distinguish analysis construction from read-only fact
  queries and independent verification. Query consumers MUST NOT use a query to
  build substitute whole-method analysis products. Independent verifiers MUST
  retain their checks against original facts.
- Contributors MUST limit stage inputs and product views to the facts required
  by the consumer's stated obligations. Contributors MUST NOT expose an entire
  program index or upstream graph merely to avoid defining the required view.
  Earlier-phase facts and original bytes remain valid inputs where a specified
  proof or independent replay requires them; phase numbers do not imply
  adjacent-only reads.
- The owning validator MUST be the only constructor of a sealed validated product.
  Product interfaces MUST preserve that construction authority and immutable consumer
  access. Consumers MUST NOT treat partial facts as whole-method validation or
  infer guarantees beyond the product's stated scope.
- The scheduler MUST compose ready products and classified outcomes rather than
  perform phase-owned SSA, typing, storage or Java-legality analysis. P9 MUST
  consume sealed presentation products and original debug facts without starting
  analysis or recovering missing semantics.

Compiler-enforced visibility, private construction and restricted entry authority
protect different obligations. They do not establish that a newly written
algorithm belongs in its phase. Contribution review and authorization follow the
root [architecture guardrails](../../AGENTS.md#architecture-guardrails).

## 5. Identity, origin and product invariants

### 5.1 IDs and immutable raw

"Fixed/stable identity" in this section means only that **references are consistent within a single analysis and arena IDs are not reused**; it does not mean result ordering or determinism across runs. Different worker arrival orders may generate different synthetic IDs and local names, and the verifier MUST check every reference/certificate/final span against this request's identity.

```rust
struct DexId(u32);
struct ClassKey { dex: DexId, class_def_idx: u32, descriptor: String }
struct MethodSelector { name: String, proto: String }
struct MethodKey { dex: DexId, method_idx: u32 } // a method_id uniquely resolves to owner/name/proto
struct FieldKey { dex: DexId, field_idx: u32 }
struct InsnId(u32);    // decode order within the method, never reused
struct BlockId(u32);   // identity is not reused after a split in the same request; numbers are not guaranteed to be equal across runs
struct EdgeId(u32);    // two different switch/exception edges to the same block are not merged
struct ValueId(u32);   // definition identity, not equal to a physical register
struct LocalId(u32);   // source-variable identity, one LocalId per ValueId by default
struct Site { method: MethodKey, code_unit: u32 }
struct Origin { sites: Vec<Site>, synthetic_reason: Option<SyntheticReason> }
```

`RawMethodView` proves only that recorded original ranges can be read safely. P1 validates format, operand, reference, target and metadata facts before using them in Java analysis. The debug exit may retain individually checked facts when the complete method cannot be validated, with explicit coverage and missing ranges; it does not certify ART validity. Later definite invalidity remains Malformed/Unavailable with useful original information, never a successful Java or valid-program claim.

The offset unit MUST be expressed in the type/field name: Diana MUST NOT mix DEX code_unit with file_byte, or a UTF-16 code unit with an output UTF-8 byte. A debug local name is only a hint and MUST NOT decide identity or type. The original bytes are held by the root request scope, and P1 borrows a DexImage whose header was not rewritten; P1 MUST validate and record the method's code/try/debug ranges independently. Diana MUST NOT reassemble the supposed original input with a third-party `Instruction::bytes()` [DX-11].

Each DEX instruction MUST be consumed by one Op/Terminator or explicit metadata (payload); a move-result is covered by the producer across both sites, a goto is covered by the terminator, and the pipeline MUST NOT silently delete the instruction as a "Nop". Only a real NOP is marked NoEffect; an unsupported instruction that cannot be expressed in Java may still fall back to raw in its known format.

### 5.2 CFG and effects

`FlowBody` owns the blocks and the ordered ops; the block terminator is separate. normal edges are Goto/Branch/Switch/Return; exceptional edges are `(throw_site, handler_order, handler_target, catch_type)`, and P2 MUST NOT add them to the normal successor list of the ordinary dominator algorithms. P3 MUST consume the normal and exception graphs separately.

A protected may-throw op MUST split the site: the last op of its block is that possibly throwing operation, and the normal result is visible only in the normal successor. The handler chain MUST retain the original try-table ranges, the typed catch order and the catch-all as the last item; P2 MUST NOT eliminate priority by sorting on type or target. P2 MUST build an exceptional edge that accepts the pre-error state and MUST NOT commit the default result; if a particular opcode has a special rule, P2 MUST build an explicit transfer and MUST NOT use one rule to guess every instruction.

The `Effect` table is driven by fixed opcode metadata: heap read/write, call, may_throw, monitor, class_init. An unknown external call means full conservatism; P2 MUST NOT delete a field/array read even without a side-effect write, because it may throw, be volatile or trigger init. Integer div/rem may throw, while float div/rem does not throw for that reason. P2 MUST NOT reassociate FP arithmetic; the printer MUST still emit a temporary assignment for an unused computation by default. A pure local copy/constant may be used for register shuffling, and it MUST NOT introduce a checked cast.

### 5.3 SSA scheme

ALGORITHMS §3's **fixed-entry Phi construction** applies:

- `Slot={start,width}`; a wide value is read as one slot, the builder MUST read every source before the normal writes, and a write MUST invalidate overlapping old slots.
- The builder MUST compute the normal/exception live-in requirements first, then allocate a Phi for every live-in slot of every block (a single predecessor is kept as well). A virtual EntrySeed supplies the parameters/this, so the entry back edge also has a correct first value.
- The builder MUST scan each block once, bind uses from the entry Phi and the earlier Defs of this block, and connect each normal/exception incoming once. An exception MUST use the old environment of the corresponding site, not the successful result.
- The builder MUST retain the Phi closure from real uses, MUST propagate Bad monotonically along Phi users, and MUST propagate grounded monotonically from parameters/OpResult/ExceptionAt. The builder MUST remove unused Phis and MUST reject an actual missing definition, a partial wide or an unrooted definition cycle explicitly.
- **The builder MUST NOT eliminate trivial Phis and MUST NOT do alias reduction**. Every state/traversal MUST have a finite measure and be charged; the builder MUST drop the construction Env work table after sealing and MUST NOT carry the R×E matrix all the way to P8.
- The verifier MUST replay the locally latest definition from the raw read/write order and the edge transfer, MUST check the Phi sources, the word width and the old exception state, and MUST NOT trust a builder bool or a normal dominator.

Complex exception SSA is a phase risk; the phase MAY support no handler first, then a single protected invoke, then widen. The phase support matrix MUST explain the degradation reason, and it MUST NOT claim that general SSA is complete while ignoring the exception environment.

## 6. CLI contract

The eleven commands, fields, selectors and exit codes are maintained only in this section; a task card MUST NOT duplicate another copy of the specification.

### 6.1 Top-level help and command table

The command section of the target help:

```text
Commands:
  get-class   Decompile one class
  entries     List APK/ZIP entries (columns: name, compressed_size, uncompressed_size, compression)
  extract     Extract original entry bytes to -o
  strings     List DEX string literals (columns: at, value, entry)
  xref        Search code references across every DEX (columns: entry, context, match)
  manifest    Extract decoded AndroidManifest.xml
  classes     List classes across every DEX (columns: at)
  get-method  Decompile one method
  fields      List fields across every DEX (columns: at)
  methods     List defined methods (columns: at)
  help        Print this message or the help of the given subcommand(s)
```

The table below is the command contract; usage shows the argument form and does not mean the implementation already exists.

| Command | Summary | Columns | Usage |
|---|---|---|---|
| get-class | Decompile one class | — | diana get-class <input> <class> |
| entries | List APK/ZIP entries | name, compressed_size, uncompressed_size, compression | diana entries <input> |
| extract | Extract original entry bytes to -o | — | diana extract <input> <entry-name> -o <file> |
| strings | List DEX string literals | at, value, entry | diana strings <input> |
| xref | Search code references across every DEX | entry, context, match | diana xref <input> <kind> <target> |
| manifest | Extract decoded AndroidManifest.xml | — | diana manifest <input> |
| classes | List classes across every DEX | at | diana classes <input> |
| get-method | Decompile one method | — | diana get-method <input> <method> |
| fields | List fields across every DEX | at | diana fields <input> |
| methods | List defined methods | at | diana methods <input> |
| help | Print command help | — | diana help [command] |

The core form is uniformly **`diana <command> <input> [selector] [options]`**. `help`, `--help` and `--version` do not require input. input is an APK, a ZIP containing a root DEX or a bare DEX; the narrower input ranges of the entry commands and manifest are described below. Diana MUST NOT invoke d8 automatically and MUST NOT handle ordinary JVM `.class` / JAR bytecode directly.

### 6.2 Common output and options

- The default table is **UTF-8 TSV**, whose first line holds the specified column names exactly in the order of the table above; the CLI MUST NOT add a row number, an index table, color or a debug column. Record order is not guaranteed and records are emitted as they complete or arrive.
- `--format text|json`, default text; a table in JSON is the object `{ "items": [...] }` with fields equal to the column names. The JSON of decompilation/manifest is an explicit domain report, see §6.5/§6.7; it is not an MCP envelope, and no protocol tool is designed.
- `-o, --output <file>`: text/JSON commands may write to a file, and on success the file holds the same payload bytes as stdout for that run; `extract` is the exception, writing only binary to the file and no copy to stdout.
- `--filter <literal>`: applies only to `classes` / `fields` / `methods` / `strings`; a case-insensitive literal substring that matches the semantic name/descriptor inside the complete `at` expression (the opaque e/l/c/m/f/s index prefix is not matched), while for strings it matches value. It is not a regex and does not cut records.
- `--class <class>`: the defining-class selector of fields/methods; the referencing-class selector of xref. It MUST use the exact class selector of §6.3, MUST NOT default to fuzzy matching, and MUST NOT replace the xref target.
- `--jobs <N>`: the concurrency configuration for scanning independent DEX files, and N MUST be greater than 0; Diana does not promise stable output and does not require the engine to start a separate thread pool per method. Diana MUST NOT expose an ineffective option for a command without related parallel work.
- `--debug`: diagnostics/trace go to stderr and MUST NOT pollute TSV, JSON, Java, XML or extracted files. Diana MUST NOT ship an option whose switch changes but whose implementation never consumes it.
- **There is no `--offset` / `--limit` / cursor / preview / stable-output**. A successful query returns every match. Safety input/work/output budgets are retained: exceeding a budget is an error, and the CLI MUST NOT present returning only the first N items as success. Internal batching/backpressure is not pagination.

Any string field of TSV is escaped for display: `\\`, `\t`, `\r`, `\n`, `\0`; a lone UTF-16 surrogate uses `\uXXXX`, and Diana MUST NOT change it to a replacement character or let one record spread over multiple lines. The CLI MUST NOT rely on lossy splitting for a value that contains the delimiter. The JSON serializer likewise retains the necessary Unicode escapes; the concrete representation of control characters and lone surrogates MUST be accepted by F03/Q09. `at` itself can be passed back to the CLI verbatim and needs shell quoting when it contains a space or `$`.

A streaming text query may have written some records before a failure; the CLI MUST then exit non-zero and state the error clearly on stderr; such partial output is not a complete success. Diana MUST build a JSON report, a decompilation display (including Mixed), manifest and extract files as complete artifacts and commit them atomically, and MUST NOT leave a falsely complete file on failure; an intentionally marked Mixed is not a truncated JavaUnit. A large table in JSON is subject to the complete-result budget; the user may switch to streaming text with external shell filtering, and Diana MUST NOT solve this by adding pagination.

### 6.3 `at`: an original-identity locator, not a result row number

A listing MUST provide an exact locator that can be passed back, and the locator MUST NOT depend on result order. **All indices are of the original input**, not sorted row numbers, synthetic LocalIds or Java names.

#### 3.1 Canonical format

```text
class-at  = e<physical-entry>:l<logical-member>:c<class-def>::<class-descriptor>
method-at = e<physical-entry>:l<logical-member>:c<class-def>:m<method-id>::<owner>-><name><proto>
field-at  = e<physical-entry>:l<logical-member>:c<class-def>:f<field-id>::<owner>-><name>:<type>
string-at = e<physical-entry>:l<logical-member>:s<string-id>
```

The numbers are non-negative decimal; `e` is the physical ZIP central-directory record index, `l` is the logical DEX ordinal inside that entry (0 for an ordinary DEX), `c` is the class_def index, and `m/f/s` is the index in the corresponding DEX's original id table. A bare DEX is treated as the single entry `e0:l0`. A canonical class descriptor is such as `Lcom/example/App;`; a method proto MUST contain the parameters and the return type, such as `(ILjava/lang/String;)Z`.
Examples (example indices, not a real corpus):

```text
e0:l0:c7::Lcom/example/App;
e0:l0:c7:m23::Lcom/example/App;->check(I)Z
e0:l0:c7:f9::Lcom/example/App;->count:I
e0:l0:s15
```

Diana MUST check the descriptor/name/proto in the canonical format and the indices in both directions: after the input is modified the original locator may become invalid and Diana MUST return an error, and it MUST NOT skip the name check when the index points at a different object. Diana MUST NOT merge equal names across different inputs/DEX files. "No duplicate and no omission" means that each original identity is recorded once, not that records are deduplicated by textual name.

#### 3.2 Readable short selectors

- get-class / `--class` accepts a class-at, `Lowner;` or a dotted `com.example.App`; Diana normalizes a dotted name only as an explicit class name and MUST NOT trigger a fuzzy search.
- get-method accepts a method-at or the complete `Lowner;->name(args)return`; the owner may be dotted as well, but Diana MUST NOT rewrite descriptors inside the proto from dots to slashes arbitrarily. **There is no bare method name and no separate `--method` / `--proto` combination**; one complete selector already expresses the identity.
- The xref field/method target accepts an original complete reference descriptor; it also accepts a canonical field-at/method-at, but then Diana MUST validate the locator and resolve it to a reference first, and references are then searched across every DEX. Diana MUST NOT let selecting a definition as the target restrict the referencing entry automatically.
- If a short name has multiple definitions, Diana MUST return `AMBIGUOUS_SELECTION` (exit4) with canonical at locator information and MUST NOT pick one arbitrarily from the first finishing worker. An exact at can distinguish duplicate class-defs with the same descriptor in the same DEX.
- A query over an empty set exits 0 normally; fetching or extracting a single object that is missing exits 3. An invalid at or proto syntax is a usage error, exit2; a stale range or a name-index inconsistency is an input/selector error, exit1, and Diana MUST NOT emit the wrong object.

### 6.4 Listing commands

#### entries

`diana entries <apk-or-zip> [--format text|json] [-o file]`.

The columns are exactly `name, compressed_size, uncompressed_size, compression`; sizes are non-negative byte counts, and compression is the numeric ZIP method (for example 0 stored, 8 deflate), not a label that varies with the language. The entries command MUST NOT decompress all entries to list the directory and MUST NOT deduplicate or sort by name. Every physical record is listed, and an unknown compression method may be listed as metadata, which does not mean that extract supports it.

The specified columns contain no extra entry_index. Exact extraction of a duplicate name goes through `extract --entry-index`, and details and ambiguity diagnostics may carry the index; Diana MUST NOT add a fifth column to the entries default columns on its own. Diana MUST NOT treat a bare DEX as a ZIP directory; that is an explicit input type error.

#### classes / fields / methods

```sh
diana classes sample.apk --filter 'example'
diana classes sample.apk --implements 'com.example.Repo'
diana classes sample.apk --extends 'com.example.Base'
diana classes sample.apk --subinterface 'com.example.Repo'
diana fields sample.apk --class 'com.example.App'
diana methods sample.apk --class 'com.example.App' --filter 'check'
```

`classes` restricts its definition list with these options, which MAY be combined
as a conjunction:

- `--extends <class>`: the definition's own superclass is that class.
- `--implements <interface>`: the definition is a class whose own interface list
  contains that interface.
- `--subinterface <interface>`: the definition is an interface whose own
  interface list contains that interface.
- `--kind class|interface`: the definition is or is not an interface.

Each relation uses only the definition's own direct `super_class_idx` or
`interface_list` edge. Diana MUST NOT compute a transitive superclass,
implementation or sub-interface closure, and MUST NOT report a class that merely
inherits the relation through another definition. Missing relations are an empty
result, not a guessed one.

Each command has only the `at` column. classes enumerates class_defs definitions; fields enumerates the static/instance fields actually defined in each class_data; methods enumerates direct/virtual **defined methods**, including abstract/native/constructor/clinit declarations, and the command MUST NOT treat every external reference in method_ids as a defined method.

The original DEX name/proto/index is used, including the original methods involved in a synthetic/bridge flag (the flag is retained in the original declaration facts and adds no list column); Diana MUST NOT run its SSA/Java backend just to list symbols. Duplicate definitions each have an independent at, and the CLI only lists their original identity; an invalid structure is still an error.

#### strings

`diana strings <input> [--filter literal] [--format text|json] [-o file]`.

The columns are `at, value, entry`. The default here is the **string_ids value enumeration** of the original query (which covers code literals as well as names/descriptors/annotation values), and Diana MUST NOT claim to scan only the strings that appear in `const-string`. Actual code use sites are queried with `xref ... string ...`, and Diana MUST NOT confuse the two. The complete UTF-16 value is retained and escaped per §6.2.

The entry column is the physical entry's original name; the 041 logical/member identity lives in at, and Diana MUST NOT fabricate another real ZIP name. For a bare DEX the entry uses the basename of the caller-supplied input as a display label, and Diana MUST NOT use that label for identity judgements.

### 6.5 get-class / get-method: deliver the decompilation result by default

```sh
diana get-class sample.apk 'com.example.App' -o App.txt
diana get-class sample.apk 'e0:l0:c7::Lcom/example/App;'
diana get-method sample.apk 'Lcom/example/App;->check(I)Z'
diana get-method sample.dex 'e0:l0:c7:m23::Lcom/example/App;->check(I)Z'
```

- The CLI MUST display the existing Report directly as default text and MUST NOT use VerifiedJavaUnit as a whole-class switch. When get-class yields a JavaUnit it outputs complete Java; when it yields Mixed it outputs clearly marked decompiled text: the real shell/fields, the recovered Java methods, and the Debug or Unavailable reason annotated per original method. Diana MUST retain every member and MUST NOT discard the other results because one method failed.
- get-method displays the exactly selected method: Java outputs the validated declaration and body; DeclarationOnly outputs the code-free declaration only, with no body; Debug outputs readable original DEX facts and coverage with the reason; and Unavailable outputs the original identity/reason with the bounded raw excerpt that is available. Diana MUST NOT call debug information executable or reassemblable source and MUST NOT fabricate Java with an empty body or a throwing stub. A method fragment needs its owner context, and Diana MUST NOT claim it compiles on its own.
- text/json only choose the representation of the same result, and they MUST NOT choose the algorithm, the fallback or the recovery threshold; Diana MUST NOT require switching to JSON to see Mixed/raw, and no `--require-java` switch is added. fields/methods remain independent symbol queries, and Diana MUST NOT prefix the source with an index table.
- When only a local Unsupported is present (including missing catalog, invoke-custom or Java target representation limits), a fully displayed Mixed/Debug/Unavailable result MAY exit0, meaning that the decompilation request and its display completed normally, not that every method was recovered as Java. Malformed, BudgetExceeded, InternalInvariant, Cancelled and I/O still fail per §6.8; a Report that can already be rendered completely MAY be kept for display with a non-zero exit, and Diana MUST NOT use a non-zero code to conceal the error.
- The top of Mixed and each failed member MUST carry the status/original identity/reason; the language section and OriginSpan can be located exactly. The marker is display metadata, and the CLI MUST NOT add an executable statement to a failed method. Diana MUST write the same display payload for an explicit `-o`; Diana MUST NOT filter out Mixed by suffix and MUST NOT automatically choose a `.java` file name for Mixed; only JavaUnit eligibility means a complete Java compilation unit.
- JSON uses the local report schema below, not an MCP envelope or serialized AST/arena. Acceptance MUST NOT golden-test format or layout; complex members, retention, raw, UTF16 and original identity/spans are accepted through the public Module interfaces.

The validation threshold for JavaUnit and per-method Java is unchanged, and a required Java case MUST NOT pass because "Debug has already been displayed".
A successful compilation does not mean semantic equivalence; a readable local failure does not mean that a request error was swallowed, and the application MUST NOT treat it as a reason to re-analyze or to start another engine.

#### Decompilation JSON schema 1

The schema applies only to delivered get-class/get-method results. All listed
fields are present; empty collections are `[]`, and `null` has only the meanings
specified below. A request that cannot produce a complete payload yields an error,
not an invented empty report. JSON object-key and independent-member order are
unconstrained.

| Object / field | Type and contract |
|---|---|
| Report `schema_version` / `command` | Integer `1` / `get-class` or `get-method` |
| Report `selection` | `{class_at: string, method_at: string or null}`; canonical original locators per §6.3, with null method only for a class request |
| Report `status` | `java_unit` or `mixed` for a class; `java`, `declaration_only`, `debug` or `unavailable` for a method |
| Report `source` | Complete rendered UTF8 display string, including Mixed/failure markers; it is not automatically a Java compilation unit |
| Report `methods` | Array of Method records; every selected original member occurs once, including Generated and Unavailable |
| Report `diagnostics` | Array of Diagnostic records, including class and hard-error reasons retained for display |
| Report `spans` | Array of Span records whose ranges refer to Report.source |
| Method `at` / `status` | Canonical original method locator / one of the four method statuses above |
| Method `emission` | `explicit`, `generated` or null; non-null only for Java, with Generated checked per §3.6 |
| Method `language` | `java`, `dex_debug`, `raw_excerpt` or `none`; declaration/Generated source is Java, and an excerpt never claims complete decoded coverage |
| Method `source` | Java declaration/body, code-free declaration, readable JSON debug snapshot, or available raw excerpt; null only for Generated owner syntax or Unavailable without an excerpt |
| Method `source_range` | Half-open `[start,end]` in Report.source, or null when no source representation exists; Generated points to its owner's syntax and may share a range with other Generated members |
| Method `spans` | Span array relative to this Method.source; empty when source is null |
| Method `reason` | Diagnostic for Debug/Unavailable; null for Java/DeclarationOnly |
| Diagnostic | `{code: string, kind: string, severity: string, method_at: string or null, code_unit: integer or null, phase: string or null, detail: string}`; kind is `malformed`, `unsupported`, `budget_exceeded`, `internal_invariant` or `cancelled`, severity follows §2.3, and a non-null code_unit requires method_at |
| Span | `{range: [start,end], member_at: string or null, sites: [{method_at: string, code_unit: integer}], synthetic_reason: string or null}`; declaration spans can have no instruction sites |

Offsets are non-negative integers in UTF8 **source bytes**, before JSON escaping;
`0 <= start <= end <= source.len()`, with endpoints on UTF8 character boundaries.
CodeUnit is a method-relative instruction coordinate, not a source offset. A
Method.source_range indexes the top-level display; a Method.spans range indexes
only that method's source. The application MUST relocate top-level spans after
prefix/marker assembly and MUST NOT substitute an in-member range for it. A Mixed
marker or unavailable reason is never a fabricated executable Java body. For an
Explicit method, source_range identifies the source payload without a surrounding
failure marker; slicing that range yields Method.source exactly. When no raw
excerpt exists, an Unavailable method still has its identity/reason record, but
source and source_range are null. Top-level diagnostic/marker spans are separate.

Top-level status expresses source eligibility, not request success: a rendered
Mixed with only Unsupported may exit0, while a retained Malformed/resource/BUG
reason still forces a hard failure. A single-method selection has exactly one
Method record; a Generated method that cannot stand alone follows the Debug rule,
not a null successful standalone source. The serializer MUST preserve escaped
UTF-16 values and charge the final JSON bytes under §7. Test assertions about
files, behavior and raw integrity come from TASKS §4.3.5, not extra production
JSON fields such as `debugFactsExact` or `tempFilesRemaining`.

### 6.6 xref

```sh
diana xref sample.apk string 'Authorization'
diana xref sample.apk type 'Ljava/lang/String;'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z'
diana xref sample.apk field 'Lcom/example/App;->count:I'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z' --class 'com.example.Caller'
```

kind is explicitly `string|type|method|field`; target is a literal exact value/descriptor (for string it is complete UTF16 string equality, not a regex; type accepts a dotted normalized class or a standard primitive/array descriptor). A method reference MUST contain the complete proto; Diana MUST NOT provide substring/fuzzy matching and MUST NOT guess an overload from a bare value.

By default Diana scans code references in every root DEX and 041 logical member of the input, using the original opcode/ref index; the search MUST NOT decompile the source first and then search. `--class` constrains only the referencing definition, and the ambiguity rules for readable short selectors are the same as §6.3.

The columns are exactly `entry, context, match, use`:

- entry: the physical entry name of the referencing side; a display label does not decide identity.
- context: `<referencing-method-at>@cu:<offset>`, where offset is the original code units of that method, displayed in hexadecimal such as `@cu:0x0012`; it is not an output line number.
- match: the string value, TypeRef, MethodRef or FieldRef that is actually referenced, escaped per §6.2; Diana MUST NOT merely echo the user target while ignoring the real reference.
- use: the original DEX mnemonic of the referencing instruction (`check-cast`, `instance-of`, `const-class`, `invoke-interface`, `iget`, `sput`, ...). It is the format-defined instruction name, not a Java expression or a source line kind.

A site exists only where an original instruction carries a reference slot of the
queried kind. Diana MUST NOT report a declaration type, a `catch` type or a
`super_class_idx`/`interface_list` edge as an xref site; those relations are
queried through `classes` per §6.4. When one instruction references the target
twice, both slots are separate lines with the same context and `use`.

**One line per actual reference site**; multiple sites in the same method get separate lines, and the CLI MUST NOT silently merge them as the old method-level findrefs result did. When the same site may reference the parameter repeatedly, the record is expressed by the defined opcode operand-site identity, and the search MUST NOT deduplicate by name. This refinement is intentional and is accepted by H09/Q09. An empty result with header/empty items exits 0; a successful search returns every match, with no pagination and no guaranteed order.

### 6.7 extract / manifest

#### extract: the original entry payload, not a rebuilt DEX

```sh
diana extract sample.apk 'classes.dex' -o original.dex
diana extract sample.apk 'classes.dex' --entry-index 4 -o original.dex
```

`extract` handles only physical APK/ZIP entries. Original bytes means **the complete decompressed entry payload**: method 0 yields the original payload directly, and method 8 yields the decompressed original deflate stream; it is not the ZIP local header or the compressed stream, and extract MUST NOT modify the DEX header/checksum, MUST NOT slice a 041 logical view and MUST NOT do any Java/smali conversion. The file content MUST be byte-for-byte equal to the decompressed bytes of the original entry.

Diana MUST match `<entry-name>` exactly; a duplicate name without `--entry-index` exits 4 and lists the candidate physical indices; when an index is given, Diana MUST check that the name and index agree. Only one entry is extracted, `-o` is required and is a single file path, and Diana MUST NOT provide batch directory unpacking. In this way names such as `../` inside the ZIP cannot become output paths, which avoids ZipSlip.

By default an existing output is not overwritten; `--force` explicitly allows replacing an ordinary file, and Diana MUST reject an output directory, a symbolic link, the input file itself or a hardlink alias. Diana MUST create a private temporary file in the target directory first and commit atomically after bounded decompression and integrity validation; on cancellation, a decompression error or an insufficient budget, the implementation MUST clean up the temporary file and MUST NOT output a half product. Diana MUST NOT let a no-force publish have a check-then-rename overwrite race; acceptance MUST also cover force per platform semantics. The CLI MUST NOT print binary data or status lines to stdout; errors/`--debug` go only to stderr.

extract MUST report an unsupported compression method, encryption and a bad range/size/CRC as explicit errors, and Diana MUST NOT take a successful directory listing to imply that extraction is reliable. extract MUST verify CRC by traversing the actual payload per ALGORITHMS §14 and MUST NOT depend on codec default behavior. The public domain reasons distinguish AmbiguousEntry, TargetExists, UnsafeTarget, CrcMismatch, BudgetExceeded, Cancelled, UnsupportedCompression, EncryptedEntry and I/O; extract MUST NOT report an existing target in place of a CRC/budget/unsupported-format failure. Writing/copying checks cancellation and budget at most every 64KiB; the public cancellation callback may observe the final file state read-only, and no separate private publish hook is added. The budget is subject to the original input/inflate limits and is not pagination.

#### manifest

`diana manifest <apk-or-zip> [-o file] [--format text|json]`.

By default Diana outputs the decoded `AndroidManifest.xml`; the original binary AXML bytes are obtained with extract. The original project's rule of selecting the last physical entry with that name is retained (based on directory order, not on worker completion order), so the manifest selection does not change because of unordered display. A missing manifest exits 3; a bare DEX or an ordinary JAR without a manifest is an input error.

Attribute names come from the original string pool. When a file leaves them empty
and identifies attributes only through `RES_XML_RESOURCE_MAP`, Diana MUST use the
public Android attribute table for those ids and MUST otherwise emit the id itself
(`attr_0x...`), declaring the Android namespace it needs. A rewritten name is a
warning, never a silent change.

Manifest decoding is allowed to deliver a **balanced partial** document. A record
that cannot be decoded, a text node that is not rendered, an out-of-pool name, an
unsupported typed value, a repeated attribute or a damaged tail MUST be reported
as a warning with its original byte offset, and the reader MUST close the elements
that are still open, so the emitted text stays well-formed XML. Warnings appear on
stderr for text and in the `warnings` array of the JSON result; they MUST NOT be
silently dropped, and a warning-only result exits 0 because the request and its
display completed. A non-AXML container, an unreadable or encrypted entry, an
exceeded explicit budget, cancellation and I/O remain hard failures with a
non-zero exit; the reader MUST NOT invent records to hide those.

text is XML; json is `{ "xml": "..." }` with no fake Java status. The application
uses a reviewed native AXML reader through the checked contract of ALGORITHMS
§14.4, with no automatic external tool. Focused reuse of the patched vendored
reader requires review of its PATCHES, all reachable reads/allocations and depth
handling; a patch list or chunk-skeleton check is not a safety certificate. Diana
MUST NOT depend on the reader under ref-fork. Independently validate a decoder
change through the public manifest Module within the agreed core test scope.

### 6.8 Help, errors and exit codes

- `diana help`, `diana --help`, `diana help get-method` / `diana get-method --help` succeed with exit0.
- exit0: a complete query/extract/manifest, or a get decompilation result without a hard error that was delivered completely; text/json follow the same rule, and a local Unsupported that yields Mixed/Debug/Unavailable does not by itself force failure.
- exit1: request failures such as I/O, Malformed, budget, internal BUG, cancellation, unsupported input or a canonical locator whose indices disagree with its descriptor/name/proto; an existing decompilation Report that can be delivered completely is not cleared because of that, but the exit is still non-zero. When global damage prevents locating the object, when Cancelled leaves no Report or when render exceeds its limit, Diana MUST NOT fabricate a report or truncate the payload.
- exit2: unknown command/flag, syntactically invalid selector/proto, missing input, missing `-o` for extract, or an inapplicable option combination.
- exit3: no result for a unique object/entry/manifest. An empty match for a list/xref is a normal exit0.
- exit4: the unique selection is ambiguous, for example a duplicate class/method/entry name, and Diana MUST reject taking the first item arbitrarily.

The CLI MUST end normally on BrokenPipe per the original Unix filter behavior and MUST NOT print a panic. The project name is uniformly Diana; Diana MUST NOT provide the previously proposed interfaces `getclass`, `findrefs`, `skill`, `--members`, `--require-java`, `--class-id`, `--offset` and `--limit`, because the canonical at already resolves exact identity.

`mcp` is deferred, the old implementation exists only in ref-fork, and Diana MUST NOT restore it as an active interface or design a new protocol; Diana MUST NOT make the old agent skill installation logic migrate or delete user directories automatically. Agent usage instructions are ordinary documentation/release assets, and Diana MUST NOT add a twelfth public `skill` command.

### 6.9 Interface validation

The basic conventions are checked per this chapter; complex behavior is validated through the public library/module interfaces, without starting a CLI subprocess.
A query compares original identity and content, extract compares the original payload and file safety, and decompilation validates values/exceptions/effects;
Diana MUST NOT use formatting or synthetic names as the semantic contract.

## 7. Application limits and lifecycle

Every numeric quota MUST be configured explicitly by the caller and is unset by default; the application MUST NOT restore an engine default limit that was cancelled by using its own fixed threshold.

| Layer | Contract |
|---|---|
| File/ZIP | input, directory count/bytes and inflated bytes are unlimited by default; Diana MUST still check the actual file range, the minimum directory record size, the end state of the compressed stream, and the actual size and CRC |
| CLI request/decompression | an explicit quota covers held bytes, decompression, dependencies and scratch; cancellation and bounded-queue backpressure are always retained, and only the bytes this request needs are reused |
| Engine | Limits are all None by default; the application passes only explicit configuration and MUST NOT inject numeric initial values for input/work/graph/output |
| CLI output | a query may stream, while a decompilation report MUST be rendered completely before it is committed; text/JSON MUST be checked against their actual bytes separately, and Diana MUST NOT truncate an explicit overrun into a success |

The application JSON output limit is optional configuration as well and defaults to None, with no fixed threshold such as 64MiB. The application MUST check a decompilation JSON against
the corresponding item only when the application limit or `Limits.max_report_output_bytes` is set explicitly, and the smaller value MUST apply only when both are set;
a lightweight query JSON MUST use only the explicit application limit. The application MUST check the configured allowance segment by segment against the final JSON bytes, and it MUST count repeated source, escapes,
diagnostics and spans. The application MUST check text/JSON against their own length separately and MUST NOT add the two representations together; actual work/
allocation MUST still be charged to the same request, and Diana MUST NOT claim that the cumulative ledger is RSS.

A query MUST use bounded admission and output queues and MUST NOT wait for display
order. Without `--jobs`, the worker count is min(8, available_parallelism), or 1
when the CPU count cannot be obtained. Explicit `--jobs N` selects N workers for
independent-image scanning. N is positive and platform-representable; the
application starts at most N workers and no more than the independent physical
inputs available. Failure to create a needed worker is a resource failure, not
silent clamping. A worker scans the 041 logical members of its admitted physical
image under that image's lease; the pool is not recreated per member or method.
Decompilation runs sequentially on the calling thread and has no `--jobs` option.

Admission holds at most one input per worker, and each worker has at most one
pending complete output row; it waits for that row to be consumed before queuing
the next. The sink consumes whichever worker is ready. The weight is
the actual retained input, decompression buffer and queued row/string capacity,
charged before growth against explicitly configured request limits. A capacity
estimate from a ZIP header is not permission to allocate unchecked. One row or
image larger than a transport chunk is processed with checked growth, not rejected
by a hidden default quota. Backpressure bounds outstanding rows, not the bytes
of an arbitrarily large single row or a retained complete JSON result.

Closing the consumer stops admission and wakes blocked workers; cancellation or
a hard producer failure stops the request and releases pending owners. Completed
streaming records may already be visible, but that prefix is not a success.
Native inflate is not preemptible; incremental calls and queue waits cooperate
with cancellation. See ALGORITHMS §14 for input and publication rules.

## 8. Java representation constraints

The verifier MUST validate value/edge/copy/binding/protection-domain separately, and Diana MUST NOT let structural coverage substitute for value semantics. P8 MUST make the Java AST and declarations satisfy
the Java8 language constraints, and MUST NOT emit a literal that changes UTF16/FP bits. P8 MUST check parameter words, single-item MUTF8 and the already determined member count
against the real format bounds; the engine MUST NOT predict the compilation size from node counts, DEX code units or repeated mentions and then reject the source.
The V4 real compilation and classfile reading MUST validate the actual format limits of code/locals/stack/handler/pool, and Diana MUST NOT present an unperformed
compilation as a production proof. P8 MUST still reject an unknown generation template whose member semantics cannot be proven, and the engine MUST NOT split a method/class or add helpers
to bypass a real limit. A method in a known context is not equal to a complete unit;
the P8 verifier MUST accept a class as JavaUnit only when the shell, every member/Generated rule, the constructors and the initializers all pass.
Generated has an original executable method and does not count as DeclarationOnly; the engine MUST return the original Debug for a generated method that cannot be expressed independently.
See ALGORITHMS for the check relationships, and TASKS for public behavior evidence and statistics.

## 9. Implementation trade-offs

Diana MUST NOT wholesale adopt advanced ValueRecovery/GVN/LICM, an all-platform catalog, an SMT/prover, automatic retry or globally optimal structuring. The order is fixed-entry Phi and independent locals first, then restricted dispatcher/natural structure, and finally exceptions, locks and complete classes. If an independent counterexample refutes an algorithm, the plan MUST be corrected rather than silently switching route or changing the expected values. Reference provenance is maintained only in SOURCE_MAP.

## 10. Resources, policy and invalidation

### 10.1 Direct scheduling and optional inlining

pipeline/schedule.rs MUST call the typed transitions of §4 directly; local liveness/type closure is done by an ordinary loop inside its Module,
and Diana MUST NOT build a Pass trait, registry, scheduling macro, TOML or restart tree. PhaseId serves only as a bounded diagnostic label.

RecoveryPolicy controls normal structure and inline_pure. normal structure defaults to
StructuredThenDispatch; DispatchOnly MUST apply only to dispatcher eligibility. inline_pure defaults to
false, and when it is true the single pure-expression inlining algorithm of ALGORITHMS §15 runs. The required lowering, type/binding/
protection-domain/resource checks have no off switch.

### 10.2 Explicit invalidation scope

| Change | Retained | Recomputed/invalidated |
|---|---|---|
| literal formatting/indent/output names | every verified artifact | P9 text/spans (if a name change affects Java binding, P9 alone is not enough) |
| normal structured vs dispatcher policy | P1–P6 | P7/P8/P9; only the plan is selected, the CFG is not changed |
| external declaration/hierarchy catalog | raw input/P2 original semantics | P4–P9; P3 is rebuilt as well if its exception-edge/throws input changed, and the request recomputes the whole method |
| CFG topology/exception chain | only raw/catalog | everything downstream of P2 is invalidated; the engine MUST NOT reuse old ValueIds/CopyPlans |
| generic/nested source policy | the value identity reusable by raw/P2–P3 | binding/types/storage/Java after P4; the whole method is recomputed |

The engine MUST re-run the whole method for a new request and MUST NOT cache mutable analysis products; the Engine retains only the immutable ProgramIndex. The in-request `EnvironmentId`/profile identifier can detect a misused artifact; different requests are not required to share numbers or a cross-run canonical fingerprint, and Diana MUST NOT sort fact tables for cache identity. A local worklist modifies only the specified finite facts and enqueues only on an actual change, charging before execution. There is no generic Repeat/Changed scheduling protocol, and Diana MUST NOT create a mechanical test suite for observability.

### 10.3 Explicit optional resource limits

Every field of `Limits::default()` is `None`. Each item has type `Option<u64>`:
`None` sets no quota for that item, and only `Some(n)` checks the bound against the caller-specified n; `Some(0)` is a genuine zero quota.
Diana MUST NOT use `u64::MAX` as an unlimited substitute and MUST NOT fill in an unconfigured input size, node count, work, allocation, nesting or output threshold.
Default application requests follow this rule too, and an adapter MUST NOT restore a fixed upper bound secretly.

| Limits field | Object/timing constrained once configured explicitly |
|---|---|
| `max_image_bytes` / `max_request_input_bytes` | a single DEX / deduplicated physical input total, before open and while accumulating |
| `max_metadata_items` / `max_string_units` | total metadata entries / UTF16 units of a single string, after the format range check and before allocation |
| `max_method_code_units` | code length of the normal method entry; raw uses a separate field |
| `max_blocks` / `max_edges` | before CFG growth |
| `max_values` / `max_constraints` / `max_phi_inputs` | before value, constraint or incoming growth |
| `max_ast_nodes` / `max_copy_assignments` | before AST and normal/shadow/cycle copy growth |
| `max_method_allocation_bytes` / `max_request_allocation_bytes` | method normal/raw total / cumulative derived allocation of the whole request, excluding caller-owned borrowed input |
| `max_work_steps` / `max_request_work_steps` | single-method normal work / open, all normal/raw and render combined; raw does not consume the normal sub-quota |
| `max_nesting` | before an explicit stack push; when unconfigured, the engine MUST NOT use host recursion for untrusted depth |
| `max_method_output_bytes` / `fallback_output_bytes` | escaped normal member / UTF8 length of the original-fact debug snapshot |
| `max_report_output_bytes` | final RenderedReport.text total, including shell, members, markers and diagnostics |
| `fallback_code_units` | original code-unit range examined by the debug exit |
| `fallback_work_steps` / `fallback_allocation_bytes` | independent debug work/allocation quota; only an explicitly finite value takes part in the reservation, see §3.7 |

The three output fields are independent: only configured limits are checked, and the engine MUST NOT reset the others when one changes. normal source,
raw source and report are different length boundaries; assembling the same member into the report does not charge that member's length twice.
The capacity of unprinted OriginSpans counts as actual derived allocation; serialization length is charged only after JSON conversion, see §7.
Actual work and allocation accumulated by a temporary/failed candidate is not refunded, but the engine MUST NOT thereby turn an unconfigured allowance into an implicit limit.

The engine MUST return BudgetExceeded naming the actual field/bound when an explicit limit is hit, and MUST NOT change it to Malformed, MUST NOT truncate it into success
and MUST NOT re-run normal/raw. Diana MUST always check the remaining format input, index width, checked arithmetic and platform-representable capacity; the allocator MUST use try_reserve for container growth and MUST report an actual allocation failure explicitly as BudgetExceeded without fabricating a configured limit.
Cancellation, file integrity and bounded-queue backpressure do not depend on whether Limits is configured. An unlimited quota does not mean unlimited physical memory,
and Diana MUST NOT claim a fixed time/RSS upper bound for every input; performance is recorded only from actual release runs.

A trace is optional bounded localization only; Diana MUST NOT add a registry or an invocation-count test for it and MUST NOT introduce a separate console.

# Diana phases

Diana separates APK/file handling from the native decompiler. The application in
`apps/diana/src/analysis/` owns snapshots, archives, input selection and queries.
The engine in `crates/diana-decomp/src/` borrows DEX bytes and derives declaration
facts from them.

| Phase | Engine directory | Current responsibility |
|---|---|---|
| Input context | `context/` | Request-level cross-DEX lookup and lazy caching of P1 declarations |
| P1 | `p1_input/` | Checked DEX tables, method input and original facts |
| P2 | `p2_flow/` | Instruction operations, effects and control-flow graph |
| P3 | `p3_ssa/` | Definitions, uses, Phi and exceptional environments |
| P4 | `p4_types/` | Types, declaration constraints, use conversions and initialization facts |
| P5 | `p5_variables/` | Storage, edge copies and exception snapshots |
| P6 | `p6_regions/` | Region, protection and monitor plans |
| P7 | `p7_java/` | Java declarations, method lowering and class assembly |
| P8 | `p8_emit/` | Render existing Java and original debug facts |

The ordinary-method scheduler is `engine.rs::normal_member`; it invokes the
P2–P5 chain through `engine/method_chain.rs`, then P6 and P7. P8 renders the resulting
product. Declaration-only and generated members have their own phase-owned
checks. A declined method currently goes to the classified raw exit.

P7 produces readable methods and concrete recovery diagnostics. It does not
certify Java legality. Class assembly preserves original headers, preparation
and member coverage; missing analysis or representation products still decline
to debug. The [stage chapters](README.md) describe each phase.

## 4. Phases, products and actual order

The engine has eight numbered phases. Input context and Engine scheduling are not analysis phases.

| Phase / Module | Owned products | Input and responsibility | Does not own |
|---|---|---|---|
| Input context `context` | Cross-DEX descriptor index and lazy declaration cache | locates original logical DEX definitions and caches declarations read by P1 | does not decode declarations or perform method analysis |
| P1 `p1_input` | `ProgramIndex`, `MethodInput`, `RawMethodView` → `ValidatedRawMethod` | safe container and metadata, UTF-16, raw coverage of the version/opcode format; range checking is graded separately from complete structure validation | does not decide control structure, does not reject a whole legal class because the source is hard to express |
| P2 `p2_flow` | `FlowBody` | decode, move-result binding, blocks, normal/ordered exceptional edges, effect sites | does not optimize effects, does not structure, does not speculate new call targets |
| P3 `p3_ssa` | `SsaBody` | input parameters/this, a stable ValueId per definition, normal/exception edge environments, Phi | does not merge source locals, MUST NOT eliminate exception edges |
| P4 `p4_types` | `TypedBody`, declaration constraints | primitive/reference/null/raw constant bits, use conversions, Phi join, finite monotonic work queue | does not have to rebuild complete generic source; does not modify CFG topology |
| P5 `p5_variables` | `StorageBody`, `CopyPlan` | one LocalId per definition, no general coalescing; parallel copies on normal edges, exception shadow slots; DA certificate | does not infer lexical structure, MUST NOT put SSA construction in a Repeat that regenerates identities |
| P6 `p6_regions` | `RegionPlan`, `ProtectionCertificate` | analyzes normal SCC/SESE, try ranges/handlers and monitors from a read-only StorageBody | does not modify the original CFG, MUST NOT treat catch-all automatically as finally |
| P7 `p7_java` | readable Java and uncertainty facts | structured/dispatcher lowering, expression legalization, class/constructor/initializer reconstruction and original Throws metadata | does not guess unparsed bytes, MUST NOT skip a block merely because of a visited set |
| P8 `p8_emit` | `RenderedReport`, final UTF-8 `OriginSpan` | AST printer, original-fact debug renderer, diagnostic renderer | does not do value/type/control-flow recovery |

**Scheduling order**:

```text
ProgramIndex(P1) ──→ Context(request-level lookup, lazy P1 declarations)
class request: fix owner/MemberEmission first (P7, direct template proof from P1 raw + declaration facts)
per Explicit method: ValidatedRawMethod(P1)
  → decode_and_bind(P2) → verify_flow
  → build_ssa(P3)       → verify_ssa
  → solve_types(P4)     → verify_types
  → allocate_and_copy(P5) → verify_storage
  → identify_regions(P6)
  → lower_java(P7)      → check operation correspondence and retain unresolved facts
Generated member: original code + template evidence → owner syntax and target admission (no independent fake Java body)
class assembly(P7)     → assemble_original_members_or_mark_mixed
render(P8)
any classified failure ──→ original method + available resources (explicit raw quota if any) ──→ debug / unavailable(P8)
```

P2 retains exact original call formals in `CallSite`. P4 consumes these formals
independently of source-binding availability. Declaration queries and constraints live in `p4_types/declarations/`. P4 owns
their products and the capture-prefix certificate; it does not depend on P7.
P7 consumes those products to check final source uses. Context caches partial
declaration directories with classified issues; partial directories cannot
establish member absence or hierarchy-wide uniqueness.

The only fixed points are the P3 live-in requirement and the P4 finite-facts closure; P3 uses fixed-entry Phi, one binding per block, one connection per incoming and a finite bad/grounded propagation, and it MUST NOT repeatedly guess ValueIds/aliases. Those fixed points only collect finite facts and MUST NOT create new identities in the loop. The P2 topo normalization MUST collect edits first and apply them once atomically. Pure expression inlining MUST run in P7 per [07-java](spec/07-java.md) and MUST NOT change the SSA/CFG. **Diana MUST NOT put all of P3–P5 into a mutual fixed point**, and MUST NOT force every phase to have the same number of passes.

Rationale: the ValueRecovery of dexdec depends on a structured Semantic IR [DX-01]; this design does not adopt it and keeps an explicit three-address computation that solves SSA/type/storage on the CFG first, which avoids repeated mutual scheduling of BDD/gated Phi/language syntax. This scheduling does not require DEX to have a natural phase order; explicit locals/copies increase output length in exchange for local verifiability of value provenance and transfer.

#### 4.1 Stage interface authority

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
  perform phase-owned SSA, typing, storage or Java-legality analysis. P8 MUST
  consume sealed presentation products and original debug facts without starting
  analysis or recovering missing semantics.

Compiler-enforced visibility, private construction and restricted entry authority
protect different obligations. They do not establish that a newly written
algorithm belongs in its phase. Contribution review and authorization follow the
root [architecture guardrails](../../AGENTS.md#architecture-guardrails).

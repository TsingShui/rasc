# Diana Reference Sources, Evidence and Reuse Constraints

Status: source-review notes for the design, not a cross-engine performance or correctness evaluation. This document pins reference versions; linked line numbers apply only to those versions. Diana is designed independently, not as a migration of an existing experimental branch.

## 1. Reference versions available to the team

References are identified by **public repository URL, pinned commit, and repository-relative file/symbol**. They do not depend on the author's home directory, a deleted worktree, the parent repository's disk layout, or SSH access. Public availability check: the pinned versions of the independent decompiler repositories can be accessed over HTTPS. This does not establish authorization for every fixture or the correctness of their algorithms.

| ID | Public source | Pinned commit | Relative root / review notes |
|---|---|---|---|
| DX | [TsingShui/rasc](https://github.com/TsingShui/rasc) | `ef8a4c92c6b89e8afed9ccbc1552feae67b6982d` | DX paths are relative to `crates/dexdec/`; root/rusty-dex paths are explicitly identified separately. [Pinned manifest](https://github.com/TsingShui/rasc/blob/ef8a4c92c6b89e8afed9ccbc1552feae67b6982d/crates/dexdec/Cargo.toml) |
| KU | [Noelo-Lab/kuna](https://github.com/Noelo-Lab/kuna) | `29c44b57f8c1d9b35044aa4ee3f636c193288009` | Paths are relative to that repository's root. [Pinned phases document](https://github.com/Noelo-Lab/kuna/blob/29c44b57f8c1d9b35044aa4ee3f636c193288009/docs/phases.md) |
| RG | [MG1937/radguard](https://github.com/MG1937/radguard), `0.1.3` | **`290de20c925c4a2c7642b1a63c9997185a06e130`** | Paths are relative to radguard's own repository root. [Pinned manifest](https://github.com/MG1937/radguard/blob/290de20c925c4a2c7642b1a63c9997185a06e130/Cargo.toml). An earlier version incorrectly identified the parent repository's commit as the radguard commit; the parent contains only a gitlink. This corrects that mistake. |
| ZD | [zed-industries/zed](https://github.com/zed-industries/zed) | `654efae441bc8eb117d0042bb28cd7cf3570bdc9` | GUI design reference: `crates/ui/src/components/{list/list_item,tab}.rs`; MIT color data: `assets/themes/ayu/ayu.json`. See §6.2. |
| RX | [TsingShui/RxD](https://github.com/TsingShui/RxD) | `54b38408e16371971c2cc1f5fbf70e8e0642907a` | GUI capability reference only: workspace features, CLI adapters, backend public surfaces and NOTICE. See §6.3. |

Explicitly obtain pinned references in the current checkout as described in [SOURCE_MAP.md](SOURCE_MAP.md#10-reproducible-inputs-and-tools). References are for review and implementation, not implicit dependencies of production or E2E test runs. Offline collaborators use source bundles verified at the same commits; do not automatically switch to main or the author's machine paths.

The old sources were archived under `ref-fork/rasc/` until commit 5cb156c, when
that tree was removed from the working tree; they do not participate in active
builds or tests and remain reachable in git history. Read the pinned DX/old root
paths through their public URL, commit and path anchors below; those anchors are
unchanged. See [00-overview](spec/00-overview.md).

Product scope: Diana is the entire project, `diana` is the active product/CLI, and `diana-decomp` is the internal engine. Existing rasc source is a reuse source for CLI, input and lightweight queries; see [00-overview](spec/00-overview.md). No MCP protocol, schema or migration is designed. Decompilation algorithms are independently implemented; the experimental branch is not ported.

The review covers source, public entry points and documentation; it makes no corpus coverage claim. Pinned commits do not track HEAD. Each reference update requires separate review; another version must not be substituted silently. The RG parent repository's gitlink does not automatically extend the parent's Apache declaration to the child repository; see §6.

## 2. Kuna: borrow mechanisms, not the whole engine

| Evidence ID | File / symbol (pinned version) | Source observation | Diana decision |
|---|---|---|---|
| KU-01 | `docs/phases.md`: Phases, Band B, Source layout | Phases classify artifacts; preparation is orthogonal; Band B execution order is not determined by directory numbering. | Physical directories follow artifact ownership; actual execution order is listed separately. Do not assume every numbered phase runs exactly once. |
| KU-02 | `decompiler/crates/kuna-decomp/src/infra/universalaction.rs:46–101`: `RuleRow`, `SchedNode`, `materialize` | The scheduling tree has Action/Pool/Group/RestartGroup; constructors actually build the execution tree. | Use direct typed-function calls and module-local bounded worklists; do not port dynamic plugins, ActionDatabase or generic Repeat scheduling. |
| KU-03 | Same file: `universal_sched:296`, `mainloop:550`, `fullloop:627` | `ActionBlockStructure` is inside mainloop; types, heritage and the rule pool interleave with later control-graph transformations. | “Kuna always finishes all dataflow before structuring” is not an accurate execution description. Diana uses the typed-call order in [phases](phases.md). |
| KU-04 | `docs/spec/00-overview.md §0.7` (approximately `3432–3475`) | Triggers, persistent state and re-entry scope are listed individually; unimplemented feedback edges are explicitly identified. | Establish feedback only for implemented producers/consumers; algorithms do not rewrite original bytes or user configuration. |
| KU-05 | `decompiler/crates/kuna-decomp/phases.toml`; `build.rs`; `build/registry.rs` | Registry and code generation provide option/catalog metadata, not the actual schedule. | Wire typed functions directly; do not build a registry or static metadata generator. PhaseId is for diagnostics only. Do not introduce TOML/build.rs as a second source of truth. |
| KU-06 | `p8_structure/blockaction.rs`: `ActionBlockStructure`; `p8_structure/region_structurer.rs` | Different structuring strategies and fallbacks after rejection exist. Some module header comments describe historical defaults and do not override live configuration. | Use actual defaults and call sites as evidence. Diana differs because Java has no goto: ordinary graphs may fall back to a state machine, but exception/monitor graphs cannot use it unconditionally. |
| KU-07 | `docs/spec/06-regions.md §7.1`; `p6_regions/kuna_regiongraph.rs` | Reduction operates on a private graph, leaving the real CFG intact; the reference uses stable ordering for output-related traversals. | Borrow the read-only CFG, private region arena and request-local identity, not stable sorting or deterministic output requirements. Diana publishes independent results as they complete. |
| KU-08 | `docs/observatory.md`; `docs/spec/08-structuring.md §8.4` | Phase artifacts, actual scheduling and quality proxies are observable; quality metrics do not prove semantics. | Design `TraceEvent` and block/edge coverage certificates; label/state counts measure readability only. |

Do not reuse SLEIGH, p-code, native calling-convention inference, code discovery, stack-frame layout or binary alias analysis. DEX supplies explicit methods, registers and most declarations, but invocation dispatch, resolution, access, exceptions and source legality remain problems; invocation-semantics analysis cannot be removed on that basis.

## 3. dexdec: borrow the correctness checklist and local algorithms

| Evidence ID | File / symbol | Source observation | Diana decision |
|---|---|---|---|
| DX-01 | `src/lib.rs:5–15`; `src/analysis/java_backend/method_pipeline.rs:53–179` | CFG/SSA → ExceptionAnalyzer → RegionGraphBuilder → RegionReducer → ValueRecovery → TypeSolver → SourceVariableAllocation → SourceSyntaxRecovery | These are the engine's algorithmic dependencies, not DEX format requirements. Diana does not port its ValueRecovery over structured Semantic IR, so it can solve types on the CFG before structuring. |
| DX-02 | `src/ir/passes/cfg_pipeline.rs:120–234` | Input/SSA validation, move-result binding, monitor/critical-edge splitting, SSA and constructor recovery. | Retain responsibility for these problems while simplifying the main path; all topology edits must precede SSA or explicitly rebuild it. |
| DX-03 | `src/ir/passes/ssa_transform.rs`: `validate_exception_boundaries` (around `200`), exceptional state (around `288`, `427`) | A block with an exceptional edge must end in its unique potentially throwing instruction; exceptional state need not be the state after normal execution. | Every protected may-throw instruction has its own site and records normal/exceptional register environments; a catch cannot directly use the instruction's successful result. |
| DX-04 | `src/ir/analysis/source_variables.rs:45–267` | Liveness/interference, coalescing, normal/exceptional Phi placement, types and lexical-exit handling work together. | Remove only **optional coalescing**; retain Phi typing, edge-copy, exceptional ingress preservation and lexical transfer placement obligations. |
| DX-05 | `src/ir/analysis/source_variables/phi.rs:1274–1361`: `ExceptionalCopyPlacement`; `:1785–1847`: `sequentialize` | Parallel-copy cycles need temporaries; exceptional edge copies require availability validation and saving before the throw site. | Use separate locals and explicit edge copies, with distinct normal/exceptional copy plans. Do not call one assignment per Phi a complete solution. |
| DX-06 | `src/ir/analysis/effects.rs`: `InstructionEffects`, `ThrowEffect` | Shared facts for reads, writes, calls, exceptions and synchronization; unknown calls are conservative. | Use an effect table with fewer fields but conservative defaults; an unused result is not a reason to remove a call, field read or exception. |
| DX-07 | `src/ir/exception.rs`; `src/ir/exception/cleanup.rs`; `src/ir/region/{exceptions,synchronization,exits}.rs` | Handles DEX intervals, handler ownership, cleanup equivalence, monitors and cross-region leaves. | Do not copy wholesale; control complexity with a restricted expressible profile, explicit structure certificates and original-information debug output. |
| DX-08 | `src/language/java/{dex,source_types,literals,assignment}.rs` | Legalization of boolean/int, null/0, generics/erasure/overloads, literals and definite assignment is not mere formatting. | Primitive/null handling and precise call binding are core; generic beautification may be deferred, but erasure contracts and overload ambiguity cannot be ignored. |
| DX-09 | `src/analysis/java_backend/constructor_syntax.rs`: `apply_with_method_returns` | Multiple pre-super extraction/scheduling/helper-generation techniques handle differences between Java and DEX constructor constraints. | Accept adjacent new/init and legal constructor prologues; do not promise that arbitrary prefixes can be converted to static helpers. |
| DX-10 | `src/analysis/java_backend/declaration_lowering.rs:1381`: `failure_body` | Recovery failure can produce a body that throws `UnsupportedOperationException`. | Do not inherit that successful representation; retain the raw method and explicitly classify the downgrade in the result model. |
| DX-11 | `src/decoder/method_decoder.rs:650–675` | An unmatched opcode may become nop; a missing `word_at` value may be 0. | The reader/decoder strictly distinguishes legal NOP, unknown opcode and truncation; prohibit that default path. |
| DX-12 | `src/api/decompiler.rs:28–55,105–138`; `src/api.rs` | Request isolation clears analysis scope but retains parsed metadata; request-local hierarchy/ABI is still consumed. | Do not call all caches “dead code.” Diana's internal Engine borrows root-owned DexImage/request scope; parser/IR/source caches are deferred until measurement demonstrates a need. Existing root decompression reuse has separate resource handling. |
| DX-13 | `Cargo.toml [lib] test=false`; `src/lib.rs` lint allow; Rayon calls in `src/analysis/java_backend/generator.rs`, etc. | Ordinary workspace tests do not automatically run legacy library tests; parallel map/collect may preserve order. | Do not delete tests to reduce size; new modules have no blanket allow and required tests enter the gate. Diana does not require stable order for parallel results; the engine executes on the calling thread. |

Check payload and `bytes()` paths in `crates/rusty-dex/src/dex/instructions.rs` for each kind; do not treat a third-party Instruction object as original fallback bytes. The design uses a safe read-only reader and may consult existing parsers, but must never trust an allocation before adding a guard.

## 4. radguard: borrow small dataflow and output provenance, reject permissive success

| Evidence ID | File / symbol | Source observation | Diana decision |
|---|---|---|---|
| RG-01 | `README.md:3–13` | Calls its output Java-like pseudocode and does not promise compilable Java. | Do not use its output as a semantic oracle. “Pseudocode” does not automatically mean incorrect behavior, nor does it substitute for type legality. |
| RG-02 | `src/decompiler.rs`: `write_method`; `src/dataflow.rs:479–519`: `optimize_impl`; `src/control_flow.rs:6–33` | lowering → Graph → def/use, split_variables, DCE/propagation → structure → writer | Borrow compact dense IDs, worklists and separation of analysis from printing. Diana makes SSA and verification explicit rather than adopting the entire flat Graph. |
| RG-03 | `src/ir/mod.rs:7–12`: `VariableId`; `src/dataflow.rs`: `ReachingDefinitions`, `split_variables_at` | No public layer directly named SSA/Phi, but versioned variables and def-use analysis exist. | “No SSA in grep means no definition identity” is too strong. Compare join/exception/type capabilities, not names. |
| RG-04 | `src/writer.rs:17–43`: `MemberReference`, `SourceOutput::append` | Text byte spans record original method/field descriptors; append adjusts offsets. | Include output OriginSpan. Java local renaming must not lose original member identity; all spans point to final UTF-8 bytes. |
| RG-05 | `src/writer.rs:140–176`: `visit_node` | The writer directly traverses the graph, tracking follow/visited/condition. | The new printer traverses only owned P7 presentation products; losing a block through visited must not become an implicit fallback. |
| RG-06 | `src/dex/mod.rs:847–925`: `parse_code_item`, `parse_catch_handlers` | Has checked_mul/range checks, but typed_count can reach Vec::with_capacity before complete validation. | The reader validates the count against remaining input before allocation; small size is not evidence of safety. |
| RG-07 | Same file, `read_mutf8:1059–1121` | Uses replacement characters for isolated surrogates when constructing a Rust String. | Diana preserves DEX strings as UTF-16 code units and does not accept semantic change; human diagnostics and literal representation are separate. |
| RG-08 | `tests/quality_gate.py:420–456`: `decompile_dad_cached`; `normalize_dad_*` | Frozen DAD output with DEX-aware normalization; missing caches are not automatically generated. pickle is suitable only for trusted internal input. | Borrow the discipline of a fixed comparator and explicit baseline; do not copy fixtures, the pickle loader or permissive normalization. Differential results are clues only. |
| RG-09 | `src/types.rs`, `src/decompiler.rs` `encoded_value` | Conversion from access flags/source literals to strings needs Java-legality checks. | synthetic/bridge have no Java modifier word; declared-synchronized conveys the declaration modifier, not implicit VM locking (see [P9](spec/09-rendering.md#declarations-metadata-and-literal-identity)); retain original flags in metadata. |

## 5. Algorithm applicability conditions

1. Fixed ValueId/PhiId fixes naming only; it does not prove state propagation converges. The set domains, transfers and finite measures are defined in [PIPELINE_DESIGN §§5.3–5.4](PIPELINE_DESIGN.md#53-p3异常感知的-pruned-ssa-与按需-idf). Do not inherit a reference paper's proof for a different domain.
2. Phi assignments must follow parallel-copy semantics. Arbitrary sequential assignments can lose old values; check actual edge placement.
3. Compilability, structural coverage, behavior on finite inputs and equivalence for arbitrary programs are distinct forms of evidence and cannot substitute for each other.
4. Constructor helpers cannot legally access arbitrary uninitialized this, locks or fields. Reject inexpressible forms under the adjacent new/init condition; do not provide a universal helper.
5. Generate enums using language rules, not by emitting `class extends Enum` or deleting synthetic members merely to compile.
6. Reference implementation size, popularity or publication venue is not proof of correctness; do not assume a percentage of removable LOC or coverage.

## 6. Reuse register and license review

For each algorithm ported during implementation, add a record of source project/pinned commit/symbol, original license and copyright, adaptation differences, invariants, tests and whether it still depends on original helpers. The principle is “understand and reimplement or document a focused extraction,” not moving a directory into the new crate to enable gradually.

- DX and rusty-dex declare Apache-2.0. Their archived copies were removed with the
  reference tree at commit 5cb156c; any future copying must retain license/copyright,
  record changes and update the new distribution inventory.
- The KU workspace declares its license and includes Ghidra/angr provenance. Before copying a file, inspect its header, the root LICENSE/NOTICE and upstream attribution, not just one Cargo line.
- RG is an independent git repository/gitlink; the pinned manifest has no `license` field, and no LICENSE was found in that repository. **The parent repository's Apache-2.0 license does not automatically cover the gitlink child repository.** Referencing designs/symbols is not permission to copy code. Until the rights holder grants explicit permission, block porting and distribution of radguard programs/fixtures; use an independent implementation and self-made fixtures. The earlier assumption that a subdirectory inherits the parent's license is withdrawn; DAD-derived attribution still needs review.
- This document cites symbols and algorithmic ideas; it does not copy unauthorized third-party programs or corpora. If license or fixture authorization cannot be confirmed, block that copying and retain the independent implementation/self-made fixture path; other design work is not blocked.

### 6.1 Current reuse register

Observed state: Diana engine and CLI implementation code contains no copied
reference source. The GUI's adapted theme data is recorded in §6.2. The
archived reference tree was removed from the working tree at commit 5cb156c and
survives only in git history; it is not a workspace member. DX and rusty-dex license files are still present;
RG remains unreviewable for copying, so it stays reading-only. No vendored
reference file was added to `crates/diana-decomp/` or `apps/diana/`.

Production dependencies remain the reviewed set in the package manifests: `serde`,
`serde_json`, `thiserror`, `rayon`, `unicode-general-category` and the optional
`fastrace` 0.7 `perf` feature. They are general-purpose libraries, not decompiler
implementations. Both workspace packages declare `Apache-2.0`; the repository
[LICENSE](../../LICENSE) is that license text. No per-crate license bundle was
added, per the repository instruction to retain existing files rather than
introduce a new bundle.

Test inputs are self-made. Java sources are compiled with the locked Temurin
21.0.8+9 `--release 8`; DEX fixtures come from locked D8 8.3.37, the self-made
D085 Python builder or hand-written Smali assembled by locked smali 2.5.2, and
declaration facts come from that locked JDK, locked classfiles or the same
self-made definitions. `Polymorphic.smali` and the D085 builder are authored here,
not recovered output or downloaded programs. Reference papers and tools stay in
ignored `.cache/references/` and are used as specifications and assertions, not as
oracles or copied fixtures.

One generated data asset exists: `apps/diana/src/analysis/attr_names.rs` maps public
Android attribute resource ids to their names. It was generated offline from
`android/R$attr.class` static constants in the platform `android.jar` of the
locally installed Android SDK; only factual id/name pairs are embedded, no AOSP or
SDK implementation code. It is compiled data, not a build- or runtime-time
dependency, and it is used only when a manifest leaves an attribute name empty.

This register records the observed repository state for the accepted batches. It
does not grant permission to copy from RG, and it does not certify any future
focused extraction; such a change requires a new record with source, commit,
license and adaptation differences.

### 6.2 GUI design reference and theme data

ZD at the commit in §1 supplies two distinct inputs:

- `crates/ui/src/components/list/list_item.rs`, `crates/ui/src/components/tab.rs`
  and `crates/project_panel/src/project_panel_settings.rs` were reviewed for
  compact spacing, indentation and active/inactive hierarchy. These UI sources
  carry GPL licenses. Diana does not copy or link their implementation; it uses
  GPUI Kit's Tree, ListItem, TabBar, Editor, menu, dialog and sheet interfaces.
- `assets/themes/ayu/ayu.json` supplies Ayu Dark color values, under
  `assets/themes/ayu/LICENSE` (MIT; copyright 2016 Ike Ku). The GUI's
  `appearance.rs` maps selected color roles and syntax colors into Kit's theme
  rather than importing Zed's theme system. Its `THEME_NOTICE` retains the full
  copyright and MIT notice, embedded in the executable and displayed through
  the Theme Credits command. This data adaptation adds no dependency on Zed.

The design reference does not establish pixel equivalence or interaction
correctness. Current GUI capabilities and verification scope belong in its
[README](../../apps/diana-gui/README.md).

### 6.3 RxD capability reference

RX at the commit in §1 was read for the GUI contracts in
[DESIGN](../../apps/diana-gui/DESIGN.md), not copied or built. Review targets were
`Cargo.toml`, `src/query/{catalog,mod,schema}.rs`, `src/cli/` command definitions,
backend public library surfaces, and `NOTICE`. These establish interface shapes
and declared command availability, not successful execution on any input.

The review distinguishes format crates from container readers, reconstruction
from instruction/metadata views, and compiled support from runtime availability.
`crates/rxd-dotnet/src/native.rs` uses NativeAOT FFI and configures a reference
directory; `crates/rxd-kuna-cli/src/lib.rs` captures linked CLI output, while
`src/cli/kuna/mod.rs` also configures process environment for isolated workers.
These are constraints to resolve before integration, not approved Diana paths.
RxD's LICENSE/NOTICE do not override Diana's existing reference-reuse or product
rules. No dependency, runtime exception or third-party extraction is authorized
by this capability review.

## 7. Additional one-command references and reading targets

```sh
python3 tools/fetch_references.py
```

The table at the top of [the download script](../../tools/fetch_references.py) is the sole inventory of actual URLs, source commits and selected directories. It currently lists 46 items; use `--list` or `--only <id> ...` to fetch only the materials for a batch. Files are saved under `.cache/references/`, with local entry point `README.generated.md`. Keep papers/slides only as same-name UTF-8 TXT, not PDF; do not re-download existing TXT. Original URLs remain available for online consultation. As requested, reference downloads do not use checksums, overwrite existing modifications, build/test references or add production dependencies. See [SOURCE_MAP.md](SOURCE_MAP.md#10-reproducible-inputs-and-tools).

| Script ID | Content / main use | Tasks and limitations |
|---|---|---|
| dx / ku / rg | The three reference versions above: raw safety, pipeline discipline, dataflow and text provenance | Pinned versions remain fixed; read rg only, do not copy while its license is unclear. |
| dex-format / dalvik-bytecode / dex-constraints | AOSP format, instructions and static/structural constraints | P1/P2, D055–D059; prefer the specifications, not permissive parser behavior. |
| jls-expressions / jls-execution / jvms-verification | JLS/JVMS 8 evaluation/invocation, initialization and verification | T10–T12/T19–T23; distinguish the language target from actual JVM/ART evidence. |
| jd-tester / java-decompiler-bugs | 2024 *Understanding and Finding Java Decompiler Bugs* and artifact documentation | Refer to executable small examples, failure classification and reduction methods. Do not download release executables/bug corpora or use output as an oracle. |
| jadx | Implementation and Java test-support sources for IntegrationTest/SmaliTest | Learn input, recompilation and execution observation during E2E preparation; do not copy text goldens, private seams or skip strategies. |
| ssa-construction / cranelift | Braun et al. 2013 SSA paper and Rust SSA builder/IR verifier | T07; borrow fixed-identity/Phi and verification ideas, without automatically replacing the chosen algorithm. DEX exceptional state still needs its own proof. |
| out-of-ssa | Boissinot et al. 2009 Out-of-SSA paper | T08/T09, D002/D003; handle parallel copies correctly before considering optional variable coalescing. |
| smali | Google smali/dexlib2 implementation | Fixture assembly and independent input projection; not a production-output reassembly requirement or an E2E tool-version upgrade. Assembler success does not imply semantic correctness. |
| soot / dexpler | Soot Dexpler type/exception-graph source and 2012 paper | Reference for type/exception batches; do not bring in the whole Jimple/Soot framework. |
| phoenix / sailr | 2013 Phoenix and 2024 SAILR structuring papers | T13/T14; native/C-oriented, not directly applicable to Java's no-goto, handler and monitor constraints. |
| vineflower | Java declaration and recovery source | Read as needed for constructor/nested/enum batches; do not treat recovered reference text as truth. |

Fixture-authoring and AXML format anchors: the locked Smali 2.5.2
[parser](https://github.com/JesusFreke/smali/blob/v2.5.2/smali/src/main/antlr/smaliParser.g),
[lexer](https://github.com/JesusFreke/smali/blob/v2.5.2/smali/src/main/jflex/smaliLexer.jflex)
and tree walker define input-fixture directives, escapes, literals and operands;
[PIPELINE_DESIGN](PIPELINE_DESIGN.md) requires readable original facts, not this assembler grammar.
AOSP Android 15 [ResourceTypes.h](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-15.0.0_r1/libs/androidfw/include/androidfw/ResourceTypes.h)
defines ResChunk_header, ResStringPool and ResXMLTree field layouts. These are
online source anchors, not new downloaded tool assets or a claim of round-trip
certification. The archived AXML PATCHES and `src/analysis/manifest.rs` are reuse
evidence only; neither certifies Diana allocation or malformed-read
safety.

Choose at most one main algorithm reference and one implementation reference per batch, producing a decision, invariant or counterexample. Do not turn close reading of every paper or building whole projects into a new prerequisite. Pin the URL/commit of a new reference in the script first, record its use and applicability limits, and do not automatically update existing expectations or expand implementation scope. Downloaded material is for development review only; public availability does not imply permission to copy or redistribute code/fixtures.

## 8. Additional research: major venues, foundational work and mature projects

This review extends “which projects exist” to “which findings support or challenge the design.” Selection priority is **direct relevance → verifiable primary evidence → publication venue/lasting impact → public implementation**, not filling out a list of four venues with unrelated work. DEX/JLS/JVMS and other specifications remain authoritative for language semantics; publication at a major venue or high citation counts do not automatically prove an implementation/fixture correct. The four major security venues are IEEE S&P, USENIX Security, CCS and NDSS; EuroS&P is not IEEE S&P. PLDI/POPL/OOPSLA are major programming-language venues, ISSTA is software testing/analysis, and TOPLAS/JAR are journals. SCAM 2019 is direct Java evidence, but is not FSE/ICSE.

Cytron SSA, Cousot abstract interpretation, Csmith, JMM and CompCert are selected as foundational work. Do not invent current citation counts or treat counts from search services with different methods as a stable ranking. Do not exclude recent papers merely because they have few citations. A limitation in an older paper does not automatically apply to the downloaded project's current HEAD.

**Review depth:** Unless explicitly noted below, the public original papers were obtained; only extracted TXT is now retained, not PDF. Abstracts, algorithm/semantic models and Diana-relevant discussion or limitations were reviewed, not every full text reproduced line by line. New projects were checked at pinned commits and key source locations; they were not built or tested. TXT is not lossless; check OCR, formulas and tables against the original URL. For Johnny, only the official slides were obtained; for the two ISSTA papers, only official entry points/abstracts were checked, without claiming to have read the full texts. ACM PDF returned an access challenge on the current network and Johnny's author PDF was unreachable; access restrictions are not bypassed and unread full texts do not support specific algorithm claims.

### 8.1 The four major security venues and a structuring survey

Download IDs and local files are maintained by the script. DOI/conference links below locate publications; the team does not need material on the author's machine. A paper's “proof” must be discussed with its language, model and trusted base.

| Download ID / paper | Venue / primary source | Main contribution and reading target | Relevance / inapplicability to Diana |
|---|---|---|---|
| dream — *No More Gotos: Decompilation Using Pattern-Independent Control-Flow Structuring and Semantics-Preserving Transformations* | **NDSS 2015**; [DOI](https://doi.org/10.14722/ndss.2015.23185), [public paper](https://www.ndss-symposium.org/wp-content/uploads/2017/09/11_4_2.pdf) | Pattern-independent structuring and CFG transformations; focus on combining conditions and eliminating unstructured control flow. | Candidate ideas for T13/T14, not a guarantee of always efficiently emitting faithful Java. Do not reevaluate effectful conditions, and do not assume Java handler/monitor/uninit are covered. |
| johnny — *Helping Johnny to Analyze Malware: A Usability-Optimized Decompiler and Malware Analysis User Study* | **IEEE S&P 2016**; [DOI](https://doi.org/10.1109/SP.2016.18), [official conference/abstract](https://www.ieee-security.org/TC/SP2016/program.html), [slides](https://www.ieee-security.org/TC/SP2016/slides/yakdan.pdf) | Relates decompilation form to analyst usability; only the abstract/slides were checked, not the full paper. | Do not pursue fewer gotos/lines alone; readability should serve real analysis tasks. Do not add a user-study platform or directly turn the findings into product scoring thresholds. |
| d-helix — *D-Helix: A Generic Decompiler Testing Framework Using Symbolic Differentiation* | **USENIX Security 2024**; [conference](https://www.usenix.org/conference/usenixsecurity24/presentation/zou), [paper](https://www.usenix.org/system/files/usenixsecurity24-zou.pdf) | Function-level recompilation, symbolic-model comparison and heuristic localization; the §4.2 loop-bounding/memory/external-call model is especially relevant. | Supports semantic checking and localization of the responsible decision stage. Its models for limiting branch execution after timeouts, dummy returns and memory initialization/enumeration do not prove arbitrary-program equivalence and must not enter Diana production semantics. TUNER is a debugging idea, not a switch for core correctness. |
| fidelity-taxonomy — *A Taxonomy of C Decompiler Fidelity Issues* | **USENIX Security 2024**; [conference](https://www.usenix.org/conference/usenixsecurity24/presentation/dramko), [paper](https://www.usenix.org/system/files/usenixsecurity24-dramko.pdf) | Open coding/thematic analysis of four C decompilers' output to classify fidelity issues beyond names/types. | Separate semantic errors, declaration/binding errors and readability defects. Output unlike the original source is not automatically a behavioral bug; long but correct output should not fail. T24 remains optional beautification. |
| everparse-paper — *EverParse: Verified Secure Zero-Copy Parsers for Authenticated Message Formats* | **USENIX Security 2019**; [conference](https://www.usenix.org/conference/usenixsecurity19/presentation/delignat-lavaud), [paper](https://www.usenix.org/system/files/sec19-ramananandro_0.pdf) | LowParse safety/correctness properties and the trusted base outside the generator; §2–§3 distinguish parser safety, format correctness and non-malleability. | Clarifies P1 checked-view proof boundaries: safe Rust does not automatically ensure consistent lengths, capacity or format correctness. TLS/TLV unique encoding does not define DEX, so it does not justify canonicalizing original bytes or adding an F*/generator dependency. |
| verified-lifting — *Verifiably Correct Lifting of Position-Independent x86-64 Binaries to Symbolized Assembly* | **CCS 2024**; [DOI](https://doi.org/10.1145/3658644.3690244), [author paper](https://www.ssrg.ece.vt.edu/papers/ccs24.pdf) | x86-64 → symbolized NASM; certificates, recompilation and Ghidra/Isabelle-supported validation of symbolization correctness. | Borrow the organization in which a producer supplies witnesses and a checker consumes explicit obligations. This is not a DEX→Java proof; OriginSpan/edge coverage certificates must not be represented as an equivalent semantic proof. |
| structuring-sok — *SoK: No Goto, No Cry? The Fairy Tale of Flawless Control-Flow Structuring* | **EuroS&P 2025 (not IEEE S&P, one of the four major security venues)**; [DOI](https://doi.org/10.1109/EuroSP63326.2025.00032), [public paper](https://oaklandsok.github.io/papers/behner2025.pdf) | §3–§5 systematize four interrelated challenges: definition and handling of resolvable/irresolvable subgraphs, resolution order and practicality; compare pattern-based/independent methods. | Directly constrains claims that one no-goto algorithm solves everything. Supports small natural structures first and a restricted explicit transfer representation; do not import linked malware samples or large corpora. |

The existing **Phoenix (USENIX Security 2013)** and **SAILR (USENIX Security 2024)** references remain. DREAM and SoK were added to compare limitations, not to replace them. SAILR's compiler-aware patterns do not automatically fit D8/R8. Java's lack of goto is a target-language legality constraint; choosing natural structures versus a legal state machine is a readability/cost tradeoff. Do not conflate them.

### 8.2 Software analysis and programming-language research

| Download ID / paper | Venue / primary source | Main contribution and reading target | Relevance / inapplicability to Diana |
|---|---|---|---|
| c-decompiler-correctness — *How Far We Have Come: Testing Decompilation Correctness of C Decompilers* | **ISSTA 2020**; [DOI](https://doi.org/10.1145/3395363.3397370), [official program](https://conf.researchr.org/track/issta-2020/issta-2020-papers) | Empirical work on decompilation correctness and type/optimization issues; only publication information and abstract were checked. | Supplement risk classification. Do not infer comparator implementation or coverage conclusions from an unobtained full text, or reuse its C corpus as Diana acceptance. |
| decompiler-effectiveness — *Evaluating the Effectiveness of Decompilers* | **ISSTA 2024**; [DOI](https://doi.org/10.1145/3650212.3652144), [official abstract](https://2024.issta.org/details/issta-2024-papers/40/Evaluating-the-Effectiveness-of-Decompilers) | Separately studies semantic consistency and readability; only the abstract and public artifact README were checked, not the full paper or reproduced evaluation. | Corresponds to semantics before beautification. Numbers such as approximately 55% in the abstract apply only to its data/tools/methods, not all C/DEX decompilers, and are not Diana progress. |
| ssa-abstract-interpretation — *SSA Translation is an Abstract Interpretation* | **POPL 2023**; [DOI](https://doi.org/10.1145/3571258), [author's full paper](https://binsec.github.io/assets/publications/papers/2023-popl-full-with-appendices.pdf) | §2 and §4–§6 construct SSA as abstract interpretation, use loop naming/global value graphs to control recursion and discuss incomplete fixpoints; the work has its own soundness/completeness argument. | Helps T07 review domains, joins, loop identity and convergence. Its joint GVN/SCCP/LICM analysis improves precision; it does not prove Diana should jointly solve the entire band, or prove the existing P3 pseudocode. |
| alive2-paper — *Alive2: Bounded Translation Validation for LLVM* | **PLDI 2021**; [DOI](https://doi.org/10.1145/3453483.3454030), [author paper](https://web.ist.utl.pt/nuno.lopes/pubs/alive2-pldi21.pdf) | Bounded translation validation, LLVM refinement/UB model; §3.8 explicitly lists features unsupported at the time. | “No counterexample found” requires a model and bounds; it does not mean general DEX/Java equivalence. Exception/volatile limitations in the 2021 paper do not automatically describe current HEAD. Production does not depend on SMT. |
| csmith — *Finding and Understanding Bugs in C Compilers* | **PLDI 2011**; [DOI](https://doi.org/10.1145/1993498.1993532), [author paper](https://www2.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf) | Generates valid programs avoiding C undefined/unspecified behavior to find wrong-code by differential testing. | Independent input certification precedes large-scale generation. Java/DEX overflow, exceptions and shifts have their own semantics; do not reuse the C generator's exclusion rules. Do not introduce a random-generation platform. |
| emi — *Compiler Validation via Equivalence Modulo Inputs* | **PLDI 2014**; [DOI](https://doi.org/10.1145/2594291.2594334), [author paper](https://people.inf.ethz.ch/suz/publications/emi.pdf) | Generates equivalent variants constrained by coverage for specific test inputs to test compilers; “modulo inputs” is the key qualification. | Supports independent behavioral oracles and targeted variants; passing finite inputs does not prove general equivalence. Do not change frozen required cases or remove their semantic triggers under the guise of optimization/pruning. |
| java-decompiler-study — *The Strengths and Behavioral Quirks of Java Bytecode Decompilers* | **SCAM 2019 (not FSE)**; [DOI](https://doi.org/10.1109/SCAM.2019.00019), [author paper](https://arxiv.org/pdf/1908.06895v1) | §III–§IV distinguish syntactic correctness, syntactic distortion and EMI; javac/ecj differ as input compilers. | Closer to the DEX target: successful compilation, behavior on tested inputs and resemblance to original source are three different things. Source compilers affect input form, supporting case-by-case structure checks in B03; text resemblance is not a gate. |

The existing *Understanding and Finding Java Decompiler Bugs* is **OOPSLA 2024 / PACMPL** ([DOI](https://doi.org/10.1145/3649860)), a more direct Java bug-classification source than general native/C research. Continue using it alongside JADX source and the existing Out-of-SSA/Braun SSA papers; do not create another test methodology.

### 8.3 Influential foundational work

These works define models and invariants. They are not a requirement to read everything before starting and do not automatically confer formal proofs or the papers' reported performance on Diana.

| Download ID / paper | Publication | Reading target and design role |
|---|---|---|
| ssa-cytron — *Efficiently Computing Static Single Assignment Form and the Control Dependence Graph* | **TOPLAS 1991**; [DOI](https://doi.org/10.1145/115372.115320), [public teaching copy](https://rsim.cs.illinois.edu/arch/qual_papers/compilers/toplas91.pdf) | Classical definitions of dominance/frontiers, Phi and renaming; check P3 use/def and control dependencies without forcing the paper's specific construction strategy. |
| abstract-interpretation — *Abstract Interpretation: A Unified Lattice Model for Static Analysis of Programs by Construction or Approximation of Fixpoints* | **POPL 1977**; [DOI](https://doi.org/10.1145/512950.512973), [author page and text](https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml) | Ordered domains, monotone transfers and fixpoints; check P3/P4 monotonicity/finite height instead of saying “a finite number of objects necessarily converges.” |
| java-memory-model — *The Java Memory Model* | **POPL 2005**; [DOI](https://doi.org/10.1145/1040305.1040336), [author paper](https://rsim.cs.illinois.edu/Pubs/popl05.pdf) | Synchronization, volatile, causality, data-race freedom and sequential consistency. The paper explicitly omits detailed final-field treatment; supplement with [JLS 8 §17](https://docs.oracle.com/javase/specs/jls/se8/html/jls-17.html). Do not infer that arbitrary final/initialization reordering is legal. |
| compcert-paper — *A Formally Verified Compiler Back-end* | **JAR 2009**; [DOI](https://doi.org/10.1007/s10817-009-9155-4), [author paper](https://xavierleroy.org/publi/compcert-backend.pdf) | §2 distinguishes semantic-preservation relations including termination/divergence/external events; later uses simulation to decompose transformations. The Cminor→PowerPC proof does not transfer to Java/DEX. Borrow only the organization of phase obligations/trusted bases; do not use Coq. |

### 8.4 Additional mature projects: pinned commits, specific symbols and what not to borrow

The script pins commits and sparse paths; links below also point to those commits and do not depend on main's line numbers. These sources are for review only, not Cargo/JVM/external-process dependencies. Paper versions and code HEAD are distinct.

| Download ID | Pinned source evidence | Source observation | Application to Diana / what not to copy directly |
|---|---|---|---|
| art | Android 15 `android-15.0.0_r1` peeled commit; [method_verifier.cc](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/runtime/verifier/method_verifier.cc#2015), [register_line.cc](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/runtime/verifier/register_line.cc#331) | Copies `saved_line_` before may-throw and explicitly does not carry a successful check-cast type into catch; `PushMonitor/PopMonitor` track lock depth/aliases and there is a `VERIFY_ERROR_LOCKING` category. At method_verifier.cc:2919–2960, constructor handling checks uninitialized state and initializes aliases; the additional same-class/direct-super restriction is commented out. VerifyInvocationArgsFromIterator:3880–3953 instead checks receiver assignability. This is pinned Android 15 source evidence, not an all-version admission guarantee. | Directly supports P2/P3 exceptional old state and T12/T16/T19. Runtime-verifier hard/soft/locking failures and version policy do not mechanically map to Diana Malformed/Unsupported. Do not copy all of ART or automatically require device/ART execution. |
| r8 | 8.3.37 peeled commit; [BasicBlock.java](https://r8.googlesource.com/r8/+/3ccc501316db4344828180e7152a4adb97522bf8/src/main/java/com/android/tools/r8/ir/code/BasicBlock.java#1163), [IRBuilder.java](https://r8.googlesource.com/r8/+/3ccc501316db4344828180e7152a4adb97522bf8/src/main/java/com/android/tools/r8/ir/conversion/IRBuilder.java) | `readCurrentDefinition(..., EdgeType)` distinguishes exceptional edges; `writeCurrentDefinition` saves the definition before a throwing result overwrites it; incomplete Phis wait for complete predecessors before sealing. | Direct engineering evidence for exceptional SSA. Borrow state obligations, not negative-register-index encoding or all optimizations. Matching source version does not mean B02 tool JAR/dependencies are locked. |
| ghidra | [heritage.cc](https://github.com/NationalSecurityAgency/ghidra/blob/6bed26c590229dde4cca7ff5759dc49b5db19d47/Ghidra/Features/Decompiler/src/decompile/cpp/heritage.cc), [blockaction.cc](https://github.com/NationalSecurityAgency/ghidra/blob/6bed26c590229dde4cca7ff5759dc49b5db19d47/Ghidra/Features/Decompiler/src/decompile/cpp/blockaction.cc), [action.cc](https://github.com/NationalSecurityAgency/ghidra/blob/6bed26c590229dde4cca7ff5759dc49b5db19d47/Ghidra/Features/Decompiler/src/decompile/cpp/action.cc#L865) | SSA heritage, block structuring and action/rule execution have distinct responsibilities; `ActionPool` traverses ops to dispatch rules. | Compare responsibilities/rule scheduling with Kuna; maturity is not an algorithm-correctness proof. Do not introduce SLEIGH, native discovery or a complete restart engine. |
| alive2 | [README](https://github.com/AliveToolkit/alive2/blob/adacbd5180ccf95075e9867c4f0c259f59c9fef2/README.md), [tv/tv.cpp](https://github.com/AliveToolkit/alive2/blob/adacbd5180ccf95075e9867c4f0c259f59c9fef2/tv/tv.cpp), [ir/state.cpp](https://github.com/AliveToolkit/alive2/blob/adacbd5180ccf95075e9867c4f0c259f59c9fef2/ir/state.cpp) | IR, symbolic execution, refinement and SMT are layered; the current README explicitly says inter-procedural transformations are unsupported and false counterexamples are possible. | Learn to make validation domains explicit, not directly use LLVM UB/refinement to validate DEX. Do not download LLVM/Z3 or enable a production solver. |
| everparse | [README](https://github.com/project-everest/everparse/blob/43b28b1e5bcf1d8ede6c4ae021cb6e227be4e498/README.md), [src/3d](https://github.com/project-everest/everparse/tree/43b28b1e5bcf1d8ede6c4ae021cb6e227be4e498/src/3d), [doc](https://github.com/project-everest/everparse/tree/43b28b1e5bcf1d8ede6c4ae021cb6e227be4e498/doc) | The current project contains different frontends and C/Rust components, unlike the single-tool state of the 2019 paper; proof objects/generator boundaries vary by component. | Refer to validated views and checked-access design. Manual Rust reimplementation does not inherit upstream proofs. Do not add a format DSL, code-generation platform or F* runtime requirement. |
| wasm-tools | [BinaryReader](https://github.com/bytecodealliance/wasm-tools/blob/6860dc1222ecbc374b8bc12efcbc980bfcc9001f/crates/wasmparser/src/binary_reader.rs#L27), [Validator](https://github.com/bytecodealliance/wasm-tools/blob/6860dc1222ecbc374b8bc12efcbc980bfcc9001f/crates/wasmparser/src/validator.rs#L115) | Borrows input and retains original offsets; Parser/Payload and Validator semantic state are separate. `BinaryReader::new` enables all known features by default, and its documentation explicitly notes panic on extreme offsets/lengths. | Supports original bytes/positions and layered validation; **do not copy the all-features default or panic-prone constructor**. Diana must meet version-gating and malformed-input no-panic obligations itself; WebAssembly rules are not DEX rules. |

## 9. Design choices and evidence boundaries

This section connects sources to concrete implementation rules. DESIGN/ALGORITHMS govern algorithm steps and data definitions; this document records provenance and the scope of evidence only.

### 9.1 Existing supported choices retained

1. **Semantics first; distinguish success from fallback:** D-Helix, empirical Java research and CompCert all indicate that source-like appearance or compilability does not substitute for behavior. Retain Java/Debug/Unavailable with original facts and explicit coverage; do not report Java success with exception stubs.
2. **Three-address CFG → fixed-identity SSA → type/storage → region/Java** remains a reasonable conservative engineering choice. Recent POPL work presents a joint-analysis alternative; it does not prove independent phases wrong or require importing GVN/BDD.
3. **Phi parallel copies, exceptional old state and strict preservation of may-throw operations** have more direct evidence: R8/ART explicitly distinguish exceptional and normal state. Do not give every predecessor the same successful state.
4. **Natural regions and explicit labelled graph transfers** preserve readable control flow without a generated dispatcher under the approved PIPELINE_DESIGN. DREAM/SAILR do not remove type, exception or lock correspondence obligations.
5. **Core correctness cannot be switched off; beautification is off by default.** TUNER can inspire development localization, but disabling a faulty phase cannot count as Java success. Reports/text still have no cross-run stable-order requirement.

### 9.2 Specific adopted design choices

| Source | Adopted engineering rule | Current location |
|---|---|---|
| R01 Cousot / POPL 2023 / R8 | Finite slot-state/liveness domains and exceptional commit positions constrain SSA construction; approved P3 uses pruned SSA and on-demand IDF, not fixed Phis at every block entry. | [PIPELINE_DESIGN §5.3](PIPELINE_DESIGN.md#53-p3异常感知的-pruned-ssa-与按需-idf); no claim to inherit a reference proof. |
| R02 SoK / DREAM / Ghidra | Explicit schemas for Sequence/Diamond/natural loops/restricted switch, check capacity before copying, use an explicit labelled transfer representation on failure; no global optimum sought. | [PIPELINE_DESIGN §5.6](PIPELINE_DESIGN.md#56-p6自然区域优先保留显式转移的-java-like-控制计划); resource quotas are not used. No numeric thresholds are inferred from the papers. |
| R03 verified lifting / CompCert / Alive2 | Raw/value/copy/edge/Java binding validators each read original facts; finite Drivers and coverage are reported separately, not as general proof. | ALGORITHMS validators and independent case evidence. |
| R04 JLS/JMM / ART | Preserve volatile/init/lock order; lock roots require dynamic freshness; avoid constant field initializers on final fields that would inline sget. | [PIPELINE_DESIGN §§5.4–5.7](PIPELINE_DESIGN.md#54-p4值事实与使用解释产出-typedmethod); no new concurrency platform. |
| R05 DEX / ART / EverParse / wasmparser | Explicit checked ranges/ULEB/MUTF16/metadata and 041 physical addressing; distinguish raw-valid/Java-profile/resource/BUG classifications. | [01-input](spec/01-input.md), [00-overview](spec/00-overview.md); do not copy upstream panic or unsafe behavior. |
| R06 Java research / fidelity taxonomy | Generated executable members are not DeclarationOnly; finite behavior is not equivalent to “compilation implies equivalence.” | [00-overview](spec/00-overview.md) and [PIPELINE_DESIGN](PIPELINE_DESIGN.md). |
| R07 R8 lifetime / concrete call chain | Preserve fallible allocation and original-result delivery, release scratch after its consumer, and keep only required result facts; Diana has no cumulative work/allocation ledger. | [00-overview](spec/00-overview.md). |
| R08 Dalvik fill / ART verifier | Filling an existing primitive array can accept null under static type verification; do not remove the NPE of a zero-element operation. Java uses a length read and proven FreshArray stores. | [02-flow](spec/02-flow.md); [bytecode](https://source.android.com/docs/core/runtime/dalvik-bytecode), FILL_ARRAY_DATA branch of pinned ART method_verifier.cc. |
| R09 JVMS 8 format limits | JVMS limits describe compiled artifacts, not Diana readable-output admission. DEX format, platform capacity and configured resource limits remain enforced; production does not qualify source against classfile limits. | [01-input](spec/01-input.md); [JVMS 4.3.3](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.3.3), [4.7.3](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.7.3). |

Independent normative basis for case revisions (R10): [DEX access_flags/encoded_value](https://source.android.com/docs/core/runtime/dex-format) distinguishes the managed declared-synchronized flag, explicit value tags and defaults; [JLS §8.9.3](https://docs.oracle.com/javase/specs/jls/se8/html/jls-8.html#jls-8.9.3) gives enum declaration order; [JLS §12.4.1](https://docs.oracle.com/javase/specs/jls/se8/html/jls-12.html#jls-12.4.1) and [§13.4.9](https://docs.oracle.com/javase/specs/jls/se8/html/jls-13.html#jls-13.4.9) define constant inlining/initialization; [ZIP APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT) specifies entry CRC/size/identity. The historical D069 TargetExists issue concerns the [CLI publication contract](../../apps/diana/README.md#extract); atomic publication relies on native filesystem primitives, not finite concurrent sampling. Revisions are recorded in git history and JSON revisionReason; normative sources do not replace actual input certification or independent review.

## 10. Reproducible inputs and tools

| Type | Required form | Prohibited |
|---|---|---|
| Source and fixtures | Repository-relative paths, public URL+commit | Personal absolute paths, sibling-repository directories, untracked files as required input |
| Pinned tools | Artifact coordinates/version, public download source, size and dependency lock | Automatically finding the author's SDK directory, latest, merely saying “install d8,” a fat JAR with unlocked dependencies |
| Reference download/cache | Current checkout's `.cache/references/`; explicit URL/source commit | Personal preexisting directory as required input; silently overwriting existing changes. Reference material does not require checksums. |
| Temporary run artifacts | System temporary-directory API | Hard-coded OS temporary directories, shared fixed temporary names, actual paths in goldens |

`.cache/` is this repository's explicitly uncommitted generated directory, not a private-machine dependency; **all required content must be reproducible**. Input source, expectations, manifests, recipes and tool versions are checked in; do not resolve missing files with “it exists on my machine.” Scripts locate their command working directories relative to themselves/the checkout; the shell need not start in a particular directory.

Actual reference URLs/commits/sparse paths are maintained only in REFERENCES at the top of tools/fetch_references.py. Network access occurs only on explicit execution; --list does not access the network or write the cache. Reuse existing nonempty TXT, without re-downloading PDFs or overwriting modifications. Check source HEAD/origin/cleanliness, without reference checksums; do not build/test reference projects or run downloaded scripts. Keep papers/slides only as same-name UTF-8 TXT, mark OCR for scanned documents, retain original URLs and revisit originals when formulas/figures are in doubt. E2E tools/fixtures require version/dependency locks and independent semantic evidence for the core E2E cases. SHA-256 records are informational only and are not verified.

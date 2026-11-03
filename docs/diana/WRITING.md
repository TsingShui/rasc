# Diana Documentation Writing Rules

This document governs active Diana documentation: `docs/diana/`, the root and
application/engine READMEs, and their `AGENTS.md` files. Archived reference material
is outside this scope. The intended readers are document authors and agents that
follow the contract.

## 1. Meaning before sentence shape

Authors MUST preserve technical meaning when translating or editing documentation.
A style change MUST NOT change requirement strength, logical conditions, operation
order, capability boundaries, evidence claims or acceptance expectations.

Separate obligations, facts and procedures so that readers can identify each role.

| Role | Form | Purpose |
|---|---|---|
| Requirement | Named actor with `MUST` or `MUST NOT` | Obligation or prohibition |
| Recommendation | Named actor with `SHOULD` | Preferred behavior with justified exceptions |
| Permission | Named actor with `MAY` | An allowed choice, not a capability claim |
| Fact or explanation | Declarative sentence | Definition, observation or reasoning with its scope |
| Procedure | Imperative sentence | An action for the reader |

Prefer subject–verb–object sentences when that structure fits the content.
Definitions, intransitive statements, imperatives, tables, equations and pseudocode
have their own forms. They do not require artificial subjects or objects.
Keep rationale separate from the obligation it explains. Keep necessary conditions
and quantifiers attached to their obligation.

## 2. Normative keywords

Diana uses the following subset of the terminology in
[RFC 2119](https://www.rfc-editor.org/rfc/rfc2119.html) and
[RFC 8174](https://www.rfc-editor.org/rfc/rfc8174.html).

| Keyword | Meaning |
|---|---|
| `MUST` | An absolute requirement of the contract |
| `MUST NOT` | An absolute prohibition of the contract |
| `SHOULD` | A recommendation; a deviation requires an understood and documented justification |
| `MAY` | Permission to choose an option |

Authors MUST use these uppercase forms for explicit normative statements in authored
English prose. State a recommendation's rationale next to the recommendation.
Record a concrete deviation's reason with the decision or implementation that
makes the deviation; the reason need not be invented in advance in the same paragraph.

Lowercase modal verbs retain their ordinary English meanings. `Can` describes
capability; `may` or `might` can describe possibility when the context is explicit.
Prefer a direct description when a modal could be mistaken for permission.
Do not use ordinary modal wording to hide an obligation. Describe uncertain claims
with their evidence boundary under §4.

This keyword subset is a Diana convention. RFC 8174 neither bans lowercase modal
verbs nor requires every normative statement to contain an uppercase keyword.
Diana does not use `SHALL` as an alternative requirement keyword. Quoted standards
retain their original wording, including `shall` or other normative terms.

Translation is not authorization to turn a recommendation, possibility or design
observation into a `MUST` requirement.

## 3. Requirements and logical conditions

Each requirement MUST identify its responsible component or actor.
Prefer one independently checkable obligation per sentence. Split independent
obligations, not a response whose atomicity or ordering is part of the contract.

| Situation | Recommended pattern |
|---|---|
| Always applicable | `The <component> MUST <response>.` |
| Prohibition | `The <component> MUST NOT <forbidden behavior>.` |
| Event | `When <trigger>, the <component> MUST <response>.` |
| Condition | `If <condition>, then the <component> MUST <response>.` |
| State | `While <state>, the <component> MUST <response>.` |
| Explicit optional feature | `Where <feature is enabled>, the <component> MUST <response>.` |
| Recommendation or permission | `The <actor> SHOULD/MAY <action>.` |

In the final pattern, select one keyword; do not write `SHOULD/MAY` in a requirement.
The feature pattern describes an existing option, not a promise of future support.

These patterns adapt [EARS](https://alistairmavin.com/ears/)
(Mavin, Wilkinson, Harwood and Novak, IEEE RE 2009,
[DOI](https://doi.org/10.1109/RE.2009.9)) to Diana's keyword convention.
EARS distinguishes ubiquitous, event-driven, unwanted-behavior, state-driven and
optional-feature requirements. EARS also supports combined conditions.
The prohibition row is a Diana presentation choice, not an additional EARS failure
category or a claim to reproduce the original templates exactly.

Authors MUST preserve conjunctions, alternatives, negation, quantifiers and temporal
conditions. `If A and B, require C` is not equivalent to two requirements that
independently require C under A and under B. Compound conditions are allowed.
Use parentheses, named predicates or decision tables when precedence is unclear.
Distinguish inclusive alternatives from exclusive alternatives when the distinction
changes behavior. Splitting a sentence does not justify weakening its precondition.

## 4. Facts, reasoning and evidence

Facts use declarative sentences rather than normative keywords. Distinguish a
specified design from an implemented capability and from an observed test result.
For example, “The design specifies X” does not claim that X is implemented.

Use complete sentences for explanations:

| Form | Purpose |
|---|---|
| `Because <premise>, <consequence>.` | Explain a consequence from a concrete premise |
| `<Premise>. Therefore, <consequence>.` | State an inference and its basis |

A rationale such as “because this is correct” supplies no evidence.
Authors MUST state the assumptions and scope of a correctness or capability claim.
Finite test observations do not establish general equivalence. A certificate does
not become a proof merely because a producer emitted it.

Label unresolved evidence according to what is actually missing. Use
`Unproven: <missing argument>` for an incomplete proof, `Not run: <execution>` for
an unperformed check, or `Unknown: <missing fact>` for absent information.
These are documentation labels, not new production failure kinds or a requirement
to label every sentence. Keep progress and execution status in README;
keep algorithm assumptions and source-evidence limitations with their technical
explanations.

## 5. Procedures and acceptance

Use imperative steps such as `Run <command>.`, `Open <file>.` or
`Check <condition>.` The reader is the actor unless the procedure names another
actor. Put independently executable actions on separate numbered lines.
Retain explicit ordering, atomicity and shared preconditions when actions depend
on each other.

Write criteria as `Acceptance: <observable condition>.` A criterion describes what
constitutes a pass; it is not a claim that execution has already passed.
Identify the input or scope, observation and required result. Commands, exit codes,
artifacts, semantic traces and public results are evidence sources. Mere file
existence or exit code zero is insufficient when the contract requires semantic
correctness, completeness or integrity.

Authors MUST follow the testing policy in the root AGENTS.md. Writing rules do
not authorize prohibited tests or restoring removed cases. Independent acceptance
values remain independent of engine output.

Batch records MUST use repository-tracked input/test paths, fixed revisions,
reproduction commands and explicit observed outcomes as their evidence references.
Authors MUST NOT cite machine-local directories such as `.cache/`, temporary
paths or private run-log paths in batch records. An unavailable execution log is
not a shared artifact; report its observed result without presenting its local
location as independently accessible evidence.

## 6. Terminology and prose

Use one preferred term for one concept. Do not interchange distinct concepts to
satisfy a vocabulary rule. The definitions in [spec/00](spec/00-overview.md) remain
authoritative; this table guides wording rather than redefining data or interfaces.

| Term | Use it for | Do not use it for |
|---|---|---|
| `origin` | Mapping back to original input | source code, reference attribution |
| `identity` | Original declaration identity | request-local ID, `ClassKey` spelling |
| `scope` | Ownership and lifetime range | `RunContext`, session, context |
| `register` | DEX physical register | analysis interval |
| `slot` | Analysis register-word interval | source span, byte range |
| `definition` | Static definition site | heap write, artifact producer |
| `use` | Read of that definition | heap read, artifact consumer |
| `edge` | CFG transfer | arc, link |
| `handler` | Exception-handler entry | Java catch block |
| `region` | Lexical-structure candidate | block group, Java scope |
| `certificate` | Evidence checked against obligations | proof with stated assumptions |
| `claim` | Ledger reservation operation | allocation, `try_reserve` |
| `held` | Reserved, unconsumed quantity | consumed work |
| `fallback` | Classified one-time read-only raw exit | raw path, degradation path |
| `budget` | Resource allowance or policy | configured bound, ledger struct |
| `limit` | A configured bound | allowance policy |
| `Quota` | The ledger structure | allowance, bound |

`Malformed`, `Unsupported`, `BudgetExceeded`, `InternalInvariant` and `Cancelled`
are failure kinds in [spec/00](spec/00-overview.md). `Unavailable` is a result status there,
not a synonym for those failure kinds. Known invalid semantics also belong under
`Malformed`; the term is not limited to damaged byte structure. Missing evidence
can cause `Unsupported` without proving that the input is semantically valid.
Authors MUST retain these distinctions rather than use a named kind as a generic
word for failure.

Prefer explicit predicates over vague phrases such as “as needed” or “works
normally.” A closed list of supported cases does not end with “etc.” An illustrative
list can be non-exhaustive when the governing rule is stated separately.
Quality claims such as “fast” or “efficient” need a measurement, complexity bound
or other stated criterion. Ordinary descriptions, established technical terms and
quoted titles are not defects merely because they contain a listed word.

Use resolvable section, symbol or file references. A local “above,” “below” or
pronoun is acceptable when its referent is unambiguous. Remove filler that adds no
meaning. Name components instead of an ambiguous “we.” Prefer full forms such as
“do not” and “for example.” Participles and established terms such as “reaching
definitions” are allowed. Preserve the official spelling of DEX, APK, ART, R8 and D8;
use Smali for the fixture assembly language without renaming `smali` commands or
packages. Production debug information is labeled Debug/`dex_debug`, not promised
as standard Smali or reassemblable source.

## 7. Numbers and protected technical content

Each named parameter or contract value has one authoritative definition. Cite its
owning section when repeating the value for explanation. Repeated occurrences of
the same numeral are not automatically duplicate definitions.

Equations, worked examples, boundary cases and independent JSON inputs/expected
values retain their concrete numbers. Authors MUST NOT replace independent expected
values with values derived from engine output or implementation configuration.
A translation MUST NOT introduce numeric product budgets or change existing limits.

In prose, separate numbers from units, for example `16 bytes`. Specify whether a
boundary includes its endpoint, using `at most`, `<` or `<=` as appropriate.
Use exact values for contractual boundaries. Approximate measurements are allowed
when identified as measurements with their method, environment and uncertainty;
approximations do not define hard limits.

Authors MUST preserve code identifiers, signatures, formulas, paths, URLs, pinned
versions, commits, commands, JSON keys and machine-observed values during a
language-only edit. Markdown heading fragments are updated when translated headings
change; update all affected local references together.

Translate comments, labels and descriptive JSON strings when they are explanatory
prose. Preserve executable meaning, escaping, literal bits, field types, case IDs,
invocation IDs and expectations. Code blocks, formulas, literal data and quoted
text are exempt from prose syntax and word-count guidance. Quotes retain their
original wording; an accompanying translation is explicitly identified.

## 8. Boundaries, prohibitions and document ownership

A negation is one of two kinds. The kind decides where it lives and how long it stays.

| Kind | What it states | Where | Lifetime |
|---|---|---|---|
| Behaviour constraint | `MUST NOT` forbids an action, order or access | Beside the behaviour it constrains, in DESIGN or ALGORITHMS | Permanent; removal changes the contract |
| Scope exclusion | A unit of work is not done | the scope of the current change | Deleted when the task completes |

Authors MUST NOT remove a behaviour constraint merely because it describes an absent
behaviour. Examples: preserving raw bytes, forbidding unsafe code, preserving effect
order, rejecting silent successful truncation.

A scope exclusion records work, not behaviour. It describes something that does not
exist, so no reader can verify it and an agent can mistake it for a constraint. Keep
it to one line, in one place, for the life of the task. Delete it at completion.
Document history belongs in git.

A prohibition that does not constrain behaviour is a misplaced scope exclusion.

| Content | Owning document |
|---|---|
| Product/interface boundaries and failure behavior | DESIGN |
| Algorithm preconditions, invariants and rejection rules | ALGORITHMS |
| Source attribution, license restrictions and evidence limitations | SOURCE_MAP |
| Historical plans and execution records | Git history |
| Current product status and document entry points | README |
| Agent authorization, repository boundaries and contribution rules | AGENTS |
| Case inputs and independent expectations | Authorized input evidence and observations |
| Writing conventions | WRITING |

Avoid duplicated historical narratives, speculative roadmaps and “first/final
version” qualifiers in technical contracts. State the current supported and
rejected behavior. README still report unimplemented work and blocked or
unperformed checks honestly. Cross-reference the authoritative rule rather than
copying its full definition into every document.

## 9. Sentence and paragraph guidance

Prefer short sentences and focused paragraphs. The following lengths are editing
review cues, not acceptance thresholds:

| Kind | Review when longer than |
|---|---:|
| Requirement | 40 words |
| Fact or explanation | 60 words |
| Procedure step | 20 words |

Review paragraphs longer than six sentences. Use a list for independent items and
a table for items with shared comparison columns. Length alone does not justify
splitting a compound condition, losing an exception or deleting technical detail.
A clear sentence outside these guidelines does not require an exemption marker.

## 10. Conflicts, exceptions and revision

When a style rule conflicts with technical meaning, preserve the meaning and
identify the conflict during review. A style-only edit MUST NOT silently resolve a
technical contradiction or revise the product contract.

Use `Free-form: <rule and reason>` only when a necessary passage departs from a
mandatory writing rule. Exempt material under §7 needs no repeated marker.
A writing exception does not waive safety, authorization, evidence or acceptance
requirements.

Rule changes record the affected rule, the reason and the case that showed the
problem. Repeated exceptions prompt review; frequency alone does not prove that
the rule is wrong.

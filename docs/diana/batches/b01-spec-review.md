# B01 specification and selection review

Goal: review the complete required execution set, symbolic targets, independent
expectations and mutation observations before fixture production.
Producer: B01; TASKS §3/§4, DESIGN §2/§5/§6 and ALGORITHMS §1/§7/§12.
Expected values remain only in `docs/diana/e2e_cases.json`.

Input: draft-5,
SHA-256 `4517e2f620bc6b481ef2468b06f47655e5d9c6a613ff7663a47f1abe5b1e60ab`;
preflight clarification
SHA-256 `c4a9a71ba3d512fda7daac153bf864ae3e1b9cbc8830103ae5bdc10cc7086485`.
Reviewed output: draft-6,
SHA-256 `f1a95873d730aedcd25305c0e4304d85c7fea5b3b2b88ed91aa9a60e2dec082e`.
These hashes identify reviewed bytes, not an approved freeze.

Changes: explicit symbolic targets for all cases, an evidence-source mapping for
every expected field, case-specific mutation observations, and the derived
`tests/e2e/cases.json` execution view with pointers rather than copied truth.
D055–D058/D060 raw recipes are concrete. Original invocation inputs and expected
values are unchanged. No case is deleted or downgraded.

Reviewer: the coding-assistant session; no independent second-reviewer approval
is claimed. On macOS arm64 with local javac 21.0.12.1, 120 source/mirror invocations
corroborate D001–D054/D071/D072/D073/D079/D082/D083. Eleven Java boundary compilation
probes corroborate the D075–D078 reference-side forms. D027/D046/D073 are source
mirrors only, not execution of original DEX. D079 uses separate processes per
invocation and one JVM per repeated-call group; javap shows getstatic and no
ConstantValue. The local JDK is not the TASKS-pinned toolchain.

Commands: `javac --release 8 -proc:none -g:none`, `java -cp`, `javap -p -c` and
`javap -p -v`. Java8 source/target warnings were retained. Evidence lives in
`.cache/e2e/preflight-draft5/`, including the three observation TSVs, separate
initialization TSVs, classfile projections and `boundaries/`. Cache evidence is
not a checked-in fixture dependency or an E2E harness.

Checks: `git diff --check`, JSON parse/ID/proto/role/assertion review, comparison of
original invocation inputs and expected values, full producer links and
`python3 tools/audit/desensitize_check.py --range HEAD`.

B01 remains unapproved: independent case review and complete concrete oracle /
mutation evidence are still required. B02 sources/recipes, actual platform tool
locks and preparer, B03 original-input certificates/scalar oracle, and B04
comparator/runner/fault-falsification evidence are not accepted. No auxiliary-tool
self-test, production implementation, reference-project build/test or ART run was
performed. TASKS §3 retains the gate and authorization boundary.

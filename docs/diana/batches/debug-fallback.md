# Analysis-oriented debug fallback

Goal: replace the standard-Smali output/reassembly promise with original DEX
information that an Agent can analyze when Java recovery fails. The operator
explicitly approved this product-contract change. Input: commit `530109f` and
specification draft-6. Contracts: DESIGN §2/§6, ALGORITHMS §7–§8, TASKS B01–B04
and T03–T05.

Preserve every case, invocation input, independent Java/behavior expectation,
original identity and hard-failure classification. Revise only non-Java output
classification and its representation-specific assertions. Smali remains a
fixture-authoring format; production output need not assemble. No production
Rust, additional tool downloads, auxiliary self-tests or release work is in scope.
Existing comparator fault checks retain their semantic/raw-information faults.

Reviewed revision: draft-7, identified by `tests/e2e/manifest.json.spec.sha256`.
Reviewer: the parent coding-assistant session. Historical draft-6 records remain
historical evidence; this record approves the operator-requested representation
change without claiming an engine implementation.

Changes: DESIGN result statuses use Debug/`dex_debug`; ALGORITHMS §7 specifies
readable original-fact snapshots and honest excerpts instead of Smali syntax;
TASKS T03–T05 and V6 compare actual debug information, not assembled output.
The comparator parses actual public member source and its rendered slice, checks
original words/PCs/operands/targets/payloads/handlers/references against the input
certificate, and ignores optional assembly-only debug/annotation spelling.
No output assembler remains in materialize/falsify. Smali still authors inputs.

Existing commands were rerun with the already available tools:

```sh
python3 tools/e2e.py prepare --jobs 6
python3 tools/e2e.py verify-inputs
python3 tools/e2e.py verify-harness
```

Results on the previously executed macOS arm64 profile: every required input
certifies; the existing correct reference passes and all 18 existing fault
scenarios reject. No new auxiliary test cases were added. The input/Java
expectation comparison retains all 86 cases and 177 invocation inputs; every
required Java expectation is unchanged. Original fixture bytes are compared with
the prior certified source-snapshot reproduction, not refreshed from engine output.

Evidence: `.cache/e2e/debug-fallback-*.log`, `certification-report.json`,
`harness/verification-report.json` and the regenerated manifest/execution view.
B01–B04 preparation is rebound to draft-7 using those unchanged inputs and the
revised comparator. No new downloads, full JDK traversal, production Rust or ART
execution occurred. The engine still reports not-implemented for the full suite;
T01/T05 production implementation and integration remain open.

# Locked fixture preparation and comparator falsification

Goal: complete B02/B03 input preparation and B04 comparison against the B01-approved
draft-6 bytes, without production Diana implementation. Input: commit `8345dbf`;
specification identity is recorded in the B01 review. Contracts: TASKS B01–B04 and
§4.3–§4.7; DESIGN §2/§5/§6; ALGORITHMS §1/§7/§8/§14.

Authorization: the operator explicitly requested completion of B01–B04. Auxiliary
testing is limited to those tasks' preparer and comparator/runner fault conditions;
it grants no permission for CLI/help/getter tests, other auxiliary tests or ART.

## Executed acceptance

Platform: macOS arm64 (`darwin-aarch64`), Python 3.14.7, Rust 1.98.0, locked
Temurin 21.0.8+9, R8 8.3.37 and Smali/dexlib2 2.5.2. The complete runtime JAR
closure is recorded in `tests/e2e/toolchain.lock.json`. Original fixture javac
uses `--release 8 -proc:none -g:none`, except D029's reviewed `-g:vars` profile.

Commands from the checkout and again from a clean source snapshot/cache:

```sh
python3 tools/e2e.py fetch-tools
python3 tools/e2e.py prepare --jobs 6
python3 tools/e2e.py verify-inputs
python3 tools/e2e.py verify-harness
```

The snapshot lives at `.cache/e2e/reproduction/source checkout with spaces/`.
Its tools ran from a different working directory. All required original invocation
inputs reproduced byte for byte; neither a personal SDK nor an existing input/tool
cache supplied a dependency. Reproduction evidence: `.cache/e2e/reproduction-report.json`.

- B02: every required case/invocation has distributable self-made sources and an
  executable recipe. Drivers remain outside DEX inputs. All required inputs build.
- B03: 86 cases / 177 invocation scenarios have original-input certificates;
  103 original DEX/ZIP assets, 129 isolated source/mirror Driver observations and
  144 pure-register/JVM comparisons support them. Independent dexlib2 and physical
  layout projections agree. Native 041 uses original shared offsets, not an overlay.
  The supplemental H03 AXML/ZIP/source projection is certified without inventing a
  D-case pass. Input and recovered-result predicates retain separate stage bindings.
- B04: the correct reference control covers six cases / eleven invocations.
  Eighteen injected faults are rejected, including every listed semantic/raw/error,
  missing-input/artifact, timeout and empty-selector fault. `TargetExists` does not
  match D069 CRC/budget failures, and an actual empty D070 output does not match
  success. The absent engine produces not-implemented for the full required set,
  with zero accepted cases; shared cases retain all 181 required seams.

Evidence paths under `.cache/e2e/`: `preparation-report.json`,
`prepared/<D-id>/{certificate,classfiles,requirement-witness}.json`,
`prepared/H03/`, `certification-report.json`, `scalar-crosscheck.json`,
`scalar-probe/crosscheck.json`, `harness/verification-report.json` and
`engine-not-implemented.json`. The checked-in `tests/e2e/manifest.json` binds
original identities, sources, tool profiles, artifacts, assertions and certificates;
it contains no copied independent expectations.

Changed functions: `tools/e2e.py` Tools/prepare_case/run; independent projection
entrypoints; scalar.execute; requirements.check_case; manifest.build_manifest;
evaluate.compare_suite; materialize.observations; falsify.verify_harness.
Subprocess deadlines/output bounds are enforced and a real timeout kills the
process group. These development-tool bounds add no production resource defaults.
No repeated full installed-JDK traversal remains.

## Decision and limits

B01–B04 preparation is accepted for the executed platform and approved revision.
The package scaffold fmt/clippy/release-build gates pass. `git diff --check` and
`python3 tools/audit/desensitize_check.py --range HEAD` pass, including new files
through a temporary index without changing the real Git index. No production Rust,
reference-project build/test, CLI black-box test or engine E2E pass is claimed.
Windows, Linux and ART are not-run. Source/mirror evidence is not original DEX
execution on ART; the restricted oracle has no heap, exception, monitor or FP VM.
T05 must connect the prepared adapter to the real public library. T/H acceptance,
including actual recovered Java/raw/rendering/query/file behavior, remains open.

Next: T01 control/resource/cancellation, then T02–T04 safe input/raw/metadata and
T05 library integration. The first Java chain remains D009 → D001 → D002/D003.

# Complete abstract interface unit

Goal: admit D078's complete abstract interface through existing P4 class binding
and P8 unit verification. Baseline: 8efc1ad. Contracts: DESIGN §4/§4.1;
ALGORITHMS §12.4–§12.6; TASKS T21. Preparation reuses accepted macOS arm64
B01–B04 and unchanged draft-7 input/tool certification.

P1 supplies original class flags, superclass, members and checked no-code
method declarations. P4's existing ClassBindings validates source owner/parent
and inherited collisions. P8 admits a public abstract interface with Object
metadata superclass, no fields, interfaces or annotations and only verified
DeclarationOnly members; it does not require or invent a constructor. P9 prints
the sealed source header and original declarations. Existing entries, input
views and construction authority remain unchanged.

The invariant is exact original method identity/prototype/count, no generated
constructor or Code, and no rejection based on estimated constant-pool cost.
D078's independent DEX/classfile projections and source generation supply
existing public core evidence. The fixed full-unit runner compiles recovered
source and independently checks declarations. Existing class/member isolation,
resource and target-bound tests remain the negative/regression coverage.
Traversal and TextBuffer growth use existing charged fallible allocation and
request cancellation. Missing metadata/binding or unsupported source membership
keeps Mixed; malformed and resource errors retain their classifications.

## Execution

P8 class_unit::verify_impl accepts only the stated abstract-interface flags,
verified DeclarationOnly members and the existing complete original member set.
It checks the P4 binding parent is Object, emits interface syntax, and neither
requires nor introduces a constructor. ClassBindings and all stage interfaces
remain unchanged. Field/interface/annotation source profiles remain guarded.
No test function, fixture or independent expected value was added.

Observed on macOS arm64 with unchanged draft-7 inputs and pinned tools:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: 163 passed, zero skips.
- python3 tools/e2e.py run --case D078: its required complete JavaUnit passes
  actual javac and independent classfile member/count/Code checks.
- python3 tools/e2e.py run: 73/86 cases and 148/181 seams pass; all 177 invocation
  scenarios execute. Exit 1 retains outstanding requirements. D078 is the only
  changed verdict and public report. Comparison includes 2,389 original member
  observations after D079 related-owner execution; all other reports are identical.
- git diff --check and python3 tools/audit/desensitize_check.py --range HEAD:
  passed.

D078's fixed complete-interface obligation passes. T21 still requires canonical
Generated bridge/override evidence, remaining declaration profiles and its fixed
boundary cases. EngineCoreRC remains unaccepted; ART/other hosts are not run.

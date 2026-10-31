# Remove SHA-256 verification

Goal: implement the operator's request to remove SHA-256 checks from active
Diana preparation, cache admission and E2E evaluation. This explicitly revises
the corresponding TASKS preparation policy; fixed D inputs, selectors and
semantic expected values remain unchanged.

Existing digest fields remain informational provenance only. Specification
identity uses its explicit reviewed revision. Tool URLs/versions/dependencies,
file existence/size, path confinement, format checks, independent projections,
Java compilation and behavioral comparison remain required. Same-size content
changes no longer have fingerprint-based detection.

Files: root AGENTS, TASKS, SOURCE_MAP, Diana README, tools/e2e.py and the owning
E2E runner modules. Remove the obsolete SHA fault from the existing falsifier;
add no auxiliary-tool tests and do not run harness self-tests under this request.
Archived reference code and format-defined DEX/ZIP integrity fields are unchanged.

Source review removed digest comparisons from tool download/reuse, preparation,
source admission, manifest drift checks, candidate-source admission and result
comparison. Adapter records now identify the specification by `spec_revision`.
The obsolete SHA fault is removed from the existing falsifier; its other faults
are unchanged. No independent semantic expected value or input fixture changed.

Python AST inspection parsed all 15 tool/runner modules. Manual execution of
`Tools(Path('.cache/e2e').resolve()).verify()` accepted the existing locked tool
paths, sizes and dependencies without fingerprint verification. Diff checks and
the desensitization gate pass. These are source/syntax/manual-operation records,
not a rerun of B04 self-tests or Engine E2E acceptance.

Remaining: T03–T05 implementation and its normal semantic acceptance. Future
runs do not require SHA-256 authorization; this policy does not claim file
content authentication.

# B01 specification and selection review

Goal: review every required symbolic target, invocation, independent oracle,
assertion source and mutation obligation before production implementation.
Producer/contracts: B01; TASKS §3/§4, DESIGN §2/§5/§6,
ALGORITHMS §1/§7/§12. Reviewer: the coding-assistant session; no second-person
approval or engine-output oracle is claimed.

Approved revision: draft-6, SHA-256
`f1a95873d730aedcd25305c0e4304d85c7fea5b3b2b88ed91aa9a60e2dec082e`.
The specification bytes, invocation inputs and independent expected values remain
unchanged. Approval is recorded here, not by rewriting the frozen JSON's draft
metadata or manufacturing an engine result.

The review covers all required D001–D086 invocations, full symbolic protos,
physical/logical scope, assertion-source mappings and case-specific mutation
observations. Required Java is not downgraded to raw. Source JVM, restricted
pure-register execution, closed fill mirrors, format-invalid inputs and
raw-integrity evidence retain their distinct scopes. Exception and monitor
behavior is not delegated to a simplified interpreter.

Evidence: local source/JVM preflight under `.cache/e2e/preflight-draft5/`, the
[fixture corrections](fixture-source-review.md), and the locked-tool independent
projections/reference runs in [preparation acceptance](b02-b04-preparation.md).
The latter replaces local-JDK corroboration as the accepted preparation evidence.
All inputRequirements have indexed input-certification or result-validation
bindings; recovered-result predicates remain mandatory when T/H execution exists.

B01 is accepted for the identified revision. B02–B04 acceptance and executed
platform limitations are recorded separately. No production implementation,
reference-project build/test or ART execution is claimed.

# Monitor identity, release and synchronized source

Goal: implement T19 through the ordinary P2–P8 chain. Contracts: DESIGN §4/§3.5;
ALGORITHMS §2/§5/§6/§10/§11. Preparation reuses accepted B01–B04 on macOS arm64,
draft-7 and certified D036–D040/D081/D082 inputs and source-JVM Drivers.

P2 owns monitor effects and the advanced exception-selection PC of monitor-exit;
P3 keeps original pre-write exception inputs; P5 supplies finite reference roots;
P6 supplies existing locals/copies; P7 owns lock-stack, freshness, all-exit and
release-handler certificates; P8 consumes those products before Java admission.
The independent source-JVM holdsLock, release, null and event observations remain
the behavioral oracle. Handwritten D081 remains a classified raw-integrity case.

The operator approved the public MonitorScopeUnproved diagnostic and a verified
JavaUnit class result needed by D040. The baseline has neither monitor operations
in the shared flow vocabulary nor complete JavaUnit results. The current raw
managed DECLARED_SYNCHRONIZED flag does not authorize inventing a lock.

## Execution

P2 decodes monitor and class-literal operations with shared effects; P1 and P2
use the advanced monitor-exit PC for handler selection while retaining the
original instruction identity. P5 supplies sealed None/One/Many reference facts
and P8 rethrow checks consume the same facts. P7 checks acquisition-layer
stacks, dynamic-producer freshness, normal and exceptional exits, and canonical
release/rethrow handlers. The shared protection forest owns nested synchronized
regions and checked natural loops. P8 replays the certificates and original
operations before retaining a render-only region; P9 renders those products.

Implicit method locks retain separate declaring-Class/this facts. The
DECLARED_SYNCHRONIZED flag alone does not add a source lock. Class-level P8
admission checks the original member set, header metadata and P4 source binding.
The implemented JavaUnit profile has an Object superclass, an explicit recovered
constructor, no fields/interfaces/class annotations, and no unproved inherited
method binding. A closed-body counterexample with static notify() remains Mixed;
independent Java8 javac rejects its inherited instance-method collision.

The static-call contract now consumes complete interface ancestry without
requiring an interface-free target. Supplemental ct.sym evidence distinguishes
an API owner's own InnerClasses entry from unrelated member-class entries
(JVMS §4.7.6). Source caller scopes retain their existing conservative checks.
The pinned Thread declaration and javap output establish its top-level name;
no class-name exception or new dependency selects a binding.

Core fixtures under crates/diana-decomp/tests/fixtures/monitor-scopes reuse the
certified self-made D036–D040/D081/D082 DEX inputs and their source/JDK declaration
facts. Locks.smali and UnitCollision.smali are self-made fixed counterexamples,
assembled with the locked Smali 2.5.2 toolchain. Their reviewed declaration facts
retain JVM access bits separately from original DEX method flags. JDK API facts
come from the locked Temurin 21.0.8+9 Java8 ct.sym entries. No expected case value
or required disposition changed.

Evidence lives under .cache/e2e/t19-monitor-scopes/:

- nextest.log: 147 core Rust tests passed, zero skipped. Fmt, workspace Clippy
  with -D warnings, locked release build, diff and desensitization gates pass.
- focused-e2e-second.log: every fixed T19 invocation passes, including D081's
  required classified Debug and D082's zero-trip/null and repeated acquisitions.
- implicit-jvm/: original and recovered Java8 Drivers agree on instance/Class
  holdsLock, normal release, throwing release and the declared-only flag case.
- collision-oracle/javac.log: independent rejection of the inherited-method
  binding counterexample; public core output preserves both available methods.
- Full acceptance report: .cache/e2e/engine/run-t0em168b/report.json. The run still
  exits nonzero for outstanding cards. D020 also gains full-unit acceptance.
- member-comparison.json compares the committed run-2r3kb0zb baseline across
  2,379 original member observations: no identity/availability regression, raw
  source change or passing-seam regression. D050 valueOf stays Unsupported with
  identical Debug, now classified BranchJavaUnsupported after class-literal
  decoding; D081 gains MonitorScopeUnproved.

The evidence remains macOS arm64 source-JVM evidence. EngineCoreRC remains
unaccepted.

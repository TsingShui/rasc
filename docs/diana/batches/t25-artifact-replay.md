# T25 generated-artifact and replay evidence

Baseline: f0aef75 plus the uncommitted, verified D085 dynamic-reference batch.
Contracts remain DESIGN §4/§4.1 and TASKS T25; ALGORITHMS is unchanged.
Preparation reuses accepted macOS arm64 B01–B04 and draft-7 input/oracle evidence.

The next T25 check consumes actual javac output from existing core E2E cases.
The independent classfile reader records pool/UTF-8 lengths, Code length, stack
and locals, but currently skips the exception table and nested Code attributes.
Materialization discards its generated-class projection after comparison, so the
recorded result does not retain these actual artifact observations. This batch
adds those missing observations and retains the projection with the result.
It does not manufacture expected values from engine output.

The invariant is that emitted Java has actual readable Java 8 artifacts with
checked Code/handler ranges and preserved member identities. The existing fixed
Drivers and independent original projections remain the semantic oracle. Reuse
core protected methods, target boundaries D075–D078 and the D086 limit contrast;
no auxiliary-tool tests or synthetic classfile-reader tests are added.

Repeat the full fixed suite with different worker counts and Python hash seeds.
Compare public records by original identity and classified outcome, not generated
text, local names, diagnostic order or completion order. Existing E2E execution
bounds subprocess time/output. No analysis interface, production parallelism,
source sort or numeric default quota is introduced.

The independent Code projection now retains actual handler ranges/types and
nested attribute names, checks instruction-boundary targets and consumes the
complete Code attribute. Materialization checks actual Java 8 major/minor and
abstract/native Code presence. Signature-only ct.sym inputs remain declaration
facts, not executable classfiles. Actual compiled projections, including artifact
sizes, are saved as declaration evidence beside each result and linked from its
record. No compiler cache or new comparator framework is added.

Observed on macOS arm64 with the pinned Temurin 21.0.8+9 target toolchain:

- Input certification passes 86 cases, 177 invocations and 144 scalar/JVM
  cross-checks. The generated manifest updates original classfile-projection
  sizes after the new observations; independent expected values are unchanged.
- The selected D033/D075/D077/D078/D086 run passes all 11 invocations.
- Full runs with PYTHONHASHSEED=1/jobs=1 and PYTHONHASHSEED=2/jobs=12 each pass
  158/181 seams and 79/86 whole cases. Every required seam has the same verdict;
  all 160 public reports preserve member identity, classification and classified
  member failure. Driver results independently satisfy the same fixed oracles;
  source spelling and completion order are not compared.
- Each full run retains 130 generated-class projections. Twenty projected method records
  contain handler tables. D029's method has one handler; D075's admitted
  static/instance methods each have Code length 1577, stack 2 and locals 512.
  D076's admitted MUTF-8 lengths are 65534/65535. D077's method has Code length
  30029 and D078's actual constant pool count is 2206, with all required
  declarations present. D086 retains Java by default and complete classified
  raw under its explicit limit.
- `/usr/bin/time -l` reports 140.08 seconds / 317571072 bytes maximum RSS for the
  serial command and 33.00 seconds / 295993344 bytes for the parallel command.
  These measure the complete self-made release E2E command, including javac/JVM
  and preparation overhead, not engine-only compute or aggregate concurrent
  worker memory. Representative APK/corpus performance is not measured.
- The existing public T25 dynamic regression also exercises valid 039/040
  const-method-handle/type prefixes and complete original references. They remain
  raw-only; invoke-custom has its specific unsupported reason.

The polymorphic slice adds `tests/fixtures/dynamic/Polymorphic.smali`, assembled
with locked smali 2.5.2 at API level 26 into `poly.dex`. The source is original
hand-written smali, not decompiler output. It has a canonical constructor and two
static methods whose invoke-polymorphic register words are checked against the
second (prototype) reference, not the signature-polymorphic declaration. P2 keeps
this invocation raw with InvokePolymorphicUnsupported. The public regression checks
the exact committed words, registers [1,2], both original references and the
rendered spans, then mutates the argument word count and prototype index. Both
mutations stay Malformed with the original six words retained; a wrong count is
not accepted as an unsupported invocation.

Reproduce with the root G0 commands, `python3 tools/e2e.py verify-inputs`, then
`PYTHONHASHSEED=1 python3 tools/e2e.py run --jobs 1` and
`PYTHONHASHSEED=2 python3 tools/e2e.py run`. Add `/usr/bin/time -l` to reproduce
the stated full-command measurement method. Reassemble the polymorphic fixture
with `java -cp <locked smali/dexlib2/util classpath> org.jf.smali.Main assemble -a 26
crates/diana-decomp/tests/fixtures/dynamic/Polymorphic.smali -o poly.dex`.
No auxiliary-tool self-tests ran. Workspace nextest passes 191 tests with zero
skips; the full required run still passes 158/181 seams and 79/86 cases with no
verdict or member change. T25 remains open for remaining mutation and source
license review evidence; this record does not declare EngineCoreRC acceptance.

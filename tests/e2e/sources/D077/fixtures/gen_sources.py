#!/usr/bin/env python3
"""D077 case-local source generator: 10000 ordered straight-line calls.

Emits the oversized part of the self-made Java the specification binds:

    .cache/e2e/D077/fixtures/Case.java   static void f() executing exactly
                                         10000 identical Dep.tick() calls in
                                         sequence and then returning, with no
                                         loop, no handler and no helper wrapper

The 10000-statement file is generated into the ignored execution workspace
instead of being checked in beside its generator: it is 200 KB of identical
lines whose only fact is the exact count, and a hand edit of 10000 literal
lines risks an off-by-one. The small dependency `fixtures/Dep.java` stays a
reviewable checked-in source, because its semantics (a static int starting at
0 and a tick() that only performs ticks++) are the part a reviewer must read
directly rather than regenerate.

The emitted file contains literal statements only, so a pinned javac/D8
compiles it without this script being a tool under test.

Deterministic and standard library only; no network, no subprocess.

Usage: python3 fixtures/gen_sources.py [output-case-root]
Default output-case-root is .cache/e2e/D077/.
"""

from __future__ import annotations

import sys
from pathlib import Path

CALL_COUNT = 10000

DEP_SOURCE = """package fixtures;

/**
 * D077 dependency. ticks starts at the Java default initial value 0 and tick()
 * only performs ticks++. Both are unchanged when the target method is recovered,
 * so the Driver can read the real call effect from the field instead of inferring
 * success from source length or an engine state getter.
 */
public final class Dep {

    public static int ticks;

    public static void tick() {
        ticks = ticks + 1;
    }
}
"""

CASE_HEADER = """package fixtures;

/**
 * D077 original input. f()V executes %d identical Dep.tick() calls in sequence
 * and then returns: no loop, no handler, no helper and no payload. The ordered
 * invoke sites are the input fact, so this shape is generated to keep the count
 * exact. All resource quotas stay None for both invocations.
 */
public final class Case {

    public static void f() {
""" % CALL_COUNT

CASE_FOOTER = """    }
}
"""


def case_source() -> str:
    body = "".join("        Dep.tick();\n" for _ in range(CALL_COUNT))
    return CASE_HEADER + body + CASE_FOOTER


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D077")
    fixtures = root / "fixtures"
    fixtures.mkdir(parents=True, exist_ok=True)

    (fixtures / "Case.java").write_text(case_source(), encoding="utf-8")

    source = case_source()
    calls = source.count("        Dep.tick();\n")
    assert calls == CALL_COUNT, calls
    assert "for (" not in source and "while (" not in source
    print(f"wrote {fixtures / 'Case.java'}")
    print(f"ordered_call_statements={calls} loops=0 handlers=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

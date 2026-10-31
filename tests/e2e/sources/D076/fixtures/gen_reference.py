#!/usr/bin/env python3
"""D076 case-local reference-Java generator for the two legal NUL roles.

The two accepted roles have a compilable Java counterpart; the over-limit role
does not, because a 65536-byte CONSTANT_Utf8 exceeds the JVMS bound and javac
rejects it. Emitting a reference for that role would fabricate an input the
format cannot express, so this script writes only:

    .cache/e2e/D076/reference/nul32767/fixtures/Case.java     32767 NUL units
    .cache/e2e/D076/reference/nul32767-A/fixtures/Case.java   32767 NUL units
                                                             followed by 'A'

Each literal is written as explicit octal NUL escapes plus, for the second
role, one plain 'A'. Octal escapes are used because the JVMS CONSTANT_Utf8
bound is measured in MUTF8 encoded bytes; the six-character Java form `\\uXXXX`
would instead be interpreted by Unicode preprocessing and is not the same
measure. The generated files contain no Unicode preprocessing newline.

The emitted files go to the ignored execution workspace instead of being
checked in beside this generator: each is 66 KB of octal NUL escapes whose
only reviewable facts are the unit count and the optional tail, and both are
recorded in `recipe.json`. The files are plain Java that a pinned javac
compiles without this script being a tool under test.

Deterministic, standard library only.

Usage: python3 fixtures/gen_reference.py [output-case-root]
Default output-case-root is .cache/e2e/D076/.
"""

from __future__ import annotations

import sys
from pathlib import Path

LEGAL_ROLES = [
    ("nul32767", 32767, False),
    ("nul32767-A", 32767, True),
]

CASE_TEMPLATE = """package fixtures;

/**
 * D076 reference Java for the {role} role: f()Ljava/lang/String; returns
 * exactly {units} NUL code units{tail}. The literal uses explicit octal NUL
 * escapes and contains no Unicode preprocessing newline, so the source
 * expresses the same code-unit sequence the DEX const-string carries. This
 * reference is the independent Java form of the accepted input; it is not an
 * execution of the original DEX.
 */
public final class Case {{

    public static String f() {{
        return "{escapes}";
    }}
}}
"""


def case_source(role: str, units: int, tail_ascii: bool) -> str:
    escapes = "".join("\\0" for _ in range(units)) + ("A" if tail_ascii else "")
    tail = " followed by one 'A'" if tail_ascii else ""
    return CASE_TEMPLATE.format(role=role, units=units, tail=tail, escapes=escapes)


def mutf8_length(units: int, tail_ascii: bool) -> int:
    return units * 2 + (1 if tail_ascii else 0)


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D076")
    for role, units, tail_ascii in LEGAL_ROLES:
        target = root / "reference" / role / "fixtures" / "Case.java"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(case_source(role, units, tail_ascii), encoding="utf-8")
        print(f"wrote {target}")
        print(f"  {role}: nul_units={units} tail_ascii={tail_ascii}"
              f" mutf8_bytes={mutf8_length(units, tail_ascii)}")
    print("nul32768: no reference emitted; a 65536-byte CONSTANT_Utf8 has no legal Java form")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""D078 case-local source generator: an interface with 2200 abstract declarations.

Emits the reference Java the specification binds for this case:

    .cache/e2e/D078/fixtures/Case.java   public interface Case declaring exactly
                                         2200 distinct abstract methods
                                         m0000()I .. m2199()I, with no
                                         constructor, no body and no default
                                         method

The declaration count is the point of the case, so the list is generated to
keep it exact instead of being hand-maintained. It is emitted into the ignored
execution workspace rather than checked in beside its generator: the file is
2200 near-identical lines whose only reviewable fact is the count, and the
independent declaration manifest is read from the DEX, not from this Java.
The emitted file is plain Java source that a pinned javac compiles without
this script being a tool under test.

Deterministic and standard library only; no network, no subprocess.

Usage: python3 fixtures/gen_sources.py [output-case-root]
Default output-case-root is .cache/e2e/D078/.
"""

from __future__ import annotations

import sys
from pathlib import Path

DECLARATION_COUNT = 2200

INTERFACE_HEADER = """package fixtures;

/**
 * D078 original input. Case is a public interface with exactly %d distinct
 * abstract declarations m0000()I .. m2199()I. It has no constructor, no default
 * method and no body: every declaration is code-less in DEX and stays
 * DeclarationOnly in Java. All resource quotas stay None, and the complete
 * recovered JavaUnit must keep every declaration so it compiles as a unit.
 */
public interface Case {
""" % DECLARATION_COUNT

INTERFACE_FOOTER = "}\n"


def interface_source() -> str:
    body = "".join("    int m%04d();\n" % index for index in range(DECLARATION_COUNT))
    return INTERFACE_HEADER + body + INTERFACE_FOOTER


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D078")
    fixtures = root / "fixtures"
    fixtures.mkdir(parents=True, exist_ok=True)

    source = interface_source()
    (fixtures / "Case.java").write_text(source, encoding="utf-8")

    declarations = source.count("    int m")
    assert declarations == DECLARATION_COUNT, declarations
    names = ["m%04d" % index for index in range(DECLARATION_COUNT)]
    assert len(set(names)) == DECLARATION_COUNT
    assert "default " not in source and "Case()" not in source
    print(f"wrote {fixtures / 'Case.java'}")
    print(f"abstract_declarations={declarations} distinct_names={len(set(names))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""D075 case-local reference-Java and Driver generator.

The accepted roles need an independent legal Java reference, and the two
executable accepted roles need a Driver that calls the recovered method with
the specification's argsPattern: first int parameter 7 and every remaining
parameter 0, expecting the body's `return p0` to yield 7. A 255-argument call
is not hand-maintainable, so the reference sources and the Driver are generated
once here and checked in as plain Java.

Emitted:

    reference/static255/fixtures/Case.java    static   f(I x 255)I
    reference/instance254/fixtures/Case.java  instance f(I x 254)I
    reference/abstract254/fixtures/Case.java  abstract f(I x 254)I, code-less
                                              declaration plus the canonical
                                              Object constructor
    driver/Driver.java                        calls static-at / instance-at

The three over-limit roles (static256, instance255, abstract255) have no
reference and no Driver: a Java parameter list of 256 words is rejected by
javac ("too many parameters"), so no legal Java baseline exists for them. They
are checked only through DEX integrity and the raw round trip.

The emitted files are plain checked-in Java that a pinned javac compiles
without running this script. Deterministic, standard library only.

Usage: python3 fixtures/gen_reference.py [case-root]
Default case-root is the D075 source directory holding this script's parent
`fixtures/` folder.
"""

from __future__ import annotations

import sys
from pathlib import Path

ACCEPTED = [
    ("static255", "static", 255),
    ("instance254", "instance", 254),
    ("abstract254", "abstract", 254),
]


def params_text(count: int) -> str:
    return ",".join("int p%d" % index for index in range(count))


CASE_TEMPLATE = """package fixtures;

/**
 * D075 independent reference Java for the {role} role: {formals} formal int
 * parameters{this_note}, so the parameter-word count including `this` is
 * {words}. The declaration keeps every parameter; nothing may be deleted or
 * renamed. This reference is the legal Java form of the accepted input and the
 * independent baseline for the recovered method.
 */
public {abstract_word}class Case {{

    public Case() {{
    }}

    public {static_word}int f({params}) {{
        return p0;
    }}
}}
"""

ABSTRACT_TEMPLATE = """package fixtures;

/**
 * D075 independent reference Java for the abstract254 role: an abstract owner
 * that keeps the canonical Object constructor, exactly as the specification
 * requires for every owner, plus one abstract declaration carrying 254 formal
 * int parameters, so the parameter-word count including `this` is 255. Only the
 * declaration is code-less in DEX; the constructor is concrete on both sides
 * and no synthetic body is invented for `f`.
 */
public abstract class Case {{

    public Case() {{
    }}

    public abstract int f({params});
}}
"""

DRIVER_TEMPLATE = r"""package driver;

import fixtures.Case;

/**
 * D075 behavior Driver for the @ROLE@ role. Each role owns its own
 * `fixtures/Case` class, so this Driver is compiled against that role's single
 * input asset in an isolated classpath; it never links two roles at once. It
 * runs exactly one invocation id and prints the observations of that real
 * execution. Every argument after the first is 0 and the first is 7, matching
 * the specification's argsPattern; the body returns p0, so a correct input and
 * a correct recovery both observe 7. The Driver never reads an engine state
 * getter and never copies an expected value.
 */
public final class Driver {

    private static final String CASE = "D075";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        int value;
        if ("@INVOCATION@".equals(invocation)) {
            value = @CALL@;
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"result\":{\"kind\":\"int\",\"value\":").append(value).append('}');
        out.append(",\"exception\":null,\"completed\":true");
        out.append('}');
        System.out.println(out.toString());
    }

    private static String quote(String value) {
        StringBuilder out = new StringBuilder();
        out.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '"' || c == '\\') {
                out.append('\\').append(c);
            } else if (c < 0x20) {
                out.append(String.format("\\u%04x", (int) c));
            } else {
                out.append(c);
            }
        }
        return out.append('"').toString();
    }
}
"""

def call_args(formals: int) -> str:
    return ",".join(["7"] + ["0"] * (formals - 1))


def case_source(role: str, kind: str, formals: int) -> str:
    if kind == "abstract":
        return ABSTRACT_TEMPLATE.format(params=params_text(formals))
    words = formals + (1 if kind == "instance" else 0)
    return CASE_TEMPLATE.format(
        role=role,
        formals=formals,
        this_note=" plus `this`" if kind == "instance" else "",
        words=words,
        abstract_word="",
        static_word="static " if kind == "static" else "",
        params=params_text(formals),
    )


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent

    for role, kind, formals in ACCEPTED:
        target = root / "reference" / role / "fixtures" / "Case.java"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(case_source(role, kind, formals), encoding="utf-8")
        print(f"wrote {target}")
        print(f"  {role}: formals={formals}"
              f" parameter_words={formals + (0 if kind == 'static' else 1)} kind={kind}")

    # Each accepted executable role owns its own fixtures.Case, so each gets its
    # own Driver compiled in an isolated classpath. The abstract role has no
    # body and therefore no Driver.
    for role, invocation, formals, call in (
        ("static255", "static-at", 255, "Case.f(%s)" % call_args(255)),
        ("instance254", "instance-at", 254, "new Case().f(%s)" % call_args(254)),
    ):
        driver = root / "driver" / role / "Driver.java"
        driver.parent.mkdir(parents=True, exist_ok=True)
        driver.write_text(
            DRIVER_TEMPLATE
            .replace("@ROLE@", role)
            .replace("@INVOCATION@", invocation)
            .replace("@CALL@", call),
            encoding="utf-8",
        )
        print(f"wrote {driver}")
        print(f"  {role}: invocation={invocation} arguments={formals} first=7 rest=0")

    for role, kind, formals in (("static256", "static", 256),
                                ("instance255", "instance", 255),
                                ("abstract255", "abstract", 255)):
        words = formals + (0 if kind == "static" else 1)
        print(f"{role}: no reference and no Driver; {words} parameter words exceed the"
              f" Java limit, so only DEX integrity and the raw round trip apply")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

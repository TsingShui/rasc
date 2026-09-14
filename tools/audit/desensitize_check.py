#!/usr/bin/env python3
"""Desensitization gate: the repository must not name a corpus that is not public.

Only *added or modified* lines are checked, so content already in history is not
re-litigated here - what the gate guarantees is that no new leak enters the tree.
That is the rule the project agreed to: new and changed lines are clean, the
pre-existing mentions are reported by `--tree` and cleaned up separately.

    tools/audit/desensitize_check.py                  # staged changes (pre-commit)
    tools/audit/desensitize_check.py --range HEAD     # working tree vs HEAD
    tools/audit/desensitize_check.py --range HEAD~3   # the last three commits
    tools/audit/desensitize_check.py --tree           # the tokens still in tracked files

`--tree` is the deferred baseline: it reports what the tracked files still contain, so it shrinks
as the old mentions are cleaned up, and it always exits 0.

Install it as a local pre-commit hook (the hook itself is not committed):

    printf '#!/bin/sh\\nexec "$(git rev-parse --show-toplevel)/.venv/bin/python" \\\\\n  "$(git rev-parse --show-toplevel)/tools/audit/desensitize_check.py" --staged\\n' \\
        > "$(git rev-parse --show-toplevel)/.git/hooks/pre-commit" && chmod +x "$(git rev-parse --show-toplevel)/.git/hooks/pre-commit"

Exit status: 0 clean, 1 a token was found, 2 the check could not run.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
WORDS = pathlib.Path(__file__).resolve().parent / "desensitize_words.txt"
# The word list is the one file whose job is to contain the tokens.
EXEMPT_NAMES = {WORDS.name}

HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")


def tokens() -> list[str]:
    if not WORDS.is_file():
        raise SystemExit(f"word list missing: {WORDS}")
    out: list[str] = []
    for raw in WORDS.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            out.append(line.lower())
    return out


def git(*args: str) -> str:
    # `errors="replace"`: a diff of a deleted binary file (a corpus fixture, an image) is
    # not UTF-8, and decoding it strictly crashed the gate with UnicodeDecodeError instead
    # of reporting on the text. Banned tokens are ASCII, so replacement cannot hide one.
    proc = subprocess.run(
        ["git", *args], cwd=REPO, capture_output=True, text=True, errors="replace"
    )
    if proc.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed:\n{proc.stderr.strip()}")
    return proc.stdout


def added_lines(diff_args: list[str]):
    """Yield (path, new_line_number, text) for every added line, `--unified=0` diff."""
    path: str | None = None
    number = 0
    for line in git("diff", "--unified=0", "--no-color", *diff_args).splitlines():
        if line.startswith("+++ "):
            raw = line[4:].strip()
            if raw == "/dev/null":
                path = None
            else:
                path = raw[2:] if raw.startswith(("b/", "a/")) else raw
            continue
        match = HUNK.match(line)
        if match:
            number = int(match.group(1))
            continue
        if path is None or line.startswith("--- "):
            continue
        if line.startswith("+"):
            yield path, number, line[1:]
            number += 1


def is_exempt(path: str) -> bool:
    """The word list is the one file whose job is to contain the tokens."""
    return pathlib.Path(path).name in EXEMPT_NAMES


def scan(diff_args: list[str]) -> list[tuple[str, int, str, str]]:
    """Every (path, line, token, excerpt) an added line or a path violates."""
    words = tokens()
    hits: list[tuple[str, int, str, str]] = []
    paths: list[str] = []
    for path, number, text in added_lines(diff_args):
        if path not in paths:
            paths.append(path)
        if is_exempt(path):
            continue
        low = text.lower()
        for word in words:
            if word in low:
                hits.append((path, number, word, text.strip()[:140]))
    for path in paths:
        if is_exempt(path):
            continue
        low = path.lower()
        for word in words:
            if word in low:
                hits.append((path, 0, word, "(file name)"))
    return hits


def tree_report() -> int:
    """The deferred baseline: the tokens still present in the tracked files."""
    words = tokens()
    per_word: dict[str, list[str]] = {}
    stem = REPO
    for rel in git("ls-files").splitlines():
        if is_exempt(rel):
            continue
        low_path = rel.lower()
        for word in words:
            if word in low_path:
                per_word.setdefault(word, []).append(f"{rel} (name)")
        try:
            text = (stem / rel).read_text(errors="replace")
        except OSError:
            continue
        for number, line in enumerate(text.splitlines(), 1):
            low = line.lower()
            for word in words:
                if word in low:
                    per_word.setdefault(word, []).append(f"{rel}:{number}")
    if not per_word:
        print("baseline: HEAD is clean")
        return 0
    print("baseline (still in the tracked files, deferred by decision):")
    total = 0
    for word in sorted(per_word):
        places = per_word[word]
        total += len(places)
        shown = ", ".join(places[:4]) + (" …" if len(places) > 4 else "")
        print(f"  {word:20} {len(places):3}  {shown}")
    print(f"  {'TOTAL':20} {total:3}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--staged", action="store_true",
                      help="check the staged diff (default; what a pre-commit hook runs)")
    mode.add_argument("--range", metavar="REV", type=str,
                      help="check the diff against REV, e.g. HEAD or HEAD~3")
    mode.add_argument("--tree", action="store_true",
                      help="report the tokens still present in tracked files (the deferred "
                           "baseline), always exits 0")
    args = ap.parse_args()

    if args.tree:
        return tree_report()

    diff_args = ["--cached"] if args.range is None else [args.range]
    where = "staged changes" if args.range is None else f"diff vs {args.range}"
    hits = scan(diff_args)
    if hits:
        print(f"desensitization gate: {len(hits)} hit(s) in {where}\n")
        for path, number, word, excerpt in hits:
            at = f"{path}:{number}" if number else path
            print(f"  {at}\n    token: {word}\n    line:  {excerpt}")
        print("\nKeep real corpus identity out of the tree: use the neutral alias, an"
              "\nanonymous handle, a self-made fixture, or - if it truly must be written\n"
              "down - the untracked .cache/ directory. See AGENT.md, \"Bug records\".")
        return 1
    print(f"desensitization gate: clean ({where}, {len(tokens())} tokens)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

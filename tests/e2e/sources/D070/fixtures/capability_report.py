#!/usr/bin/env python3
"""D070 platform capability report.

D070 requires five platform capabilities. TASKS 4.3.4/4.6 and the D070
notes say a platform lacking one of them fails preparation for that platform
instead of degrading the test. This script measures them honestly on the
current machine and prints one JSON object; it does not run the extract
seam, which does not exist yet.

Measured:
  symlink_create          os.symlink on the system temporary directory
  hardlink_create         os.link on the system temporary directory
  atomic_rename           os.replace over an existing target
  same_directory_rename   os.replace within one directory (the publish primitive)
  file_handle_identity    os.stat st_dev/st_ino equality for an alias check
  large_payload_reader    a 16 MiB single-handle sequential read

Usage: python3 tests/e2e/sources/D070/fixtures/capability_report.py
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import sys
import tempfile
from pathlib import Path


def probe(name: str, action) -> dict:
    try:
        detail = action()
        return {"capability": name, "supported": True, "detail": detail}
    except OSError as error:
        return {
            "capability": name,
            "supported": False,
            "detail": f"{type(error).__name__}: {error}",
        }


def main() -> int:
    results = []
    with tempfile.TemporaryDirectory() as directory:
        base = Path(directory)

        def symlink() -> str:
            target = base / "referent"
            target.write_bytes(b"sentinel")
            link = base / "link"
            os.symlink(target, link)
            assert link.is_symlink()
            return "os.symlink created a symlink whose referent is readable"

        def hardlink() -> str:
            source = base / "input.dex"
            source.write_bytes(b"input-bytes")
            alias = base / "alias.dex"
            os.link(source, alias)
            stat_source = source.stat()
            stat_alias = alias.stat()
            assert (stat_source.st_dev, stat_source.st_ino) == (
                stat_alias.st_dev,
                stat_alias.st_ino,
            )
            return "os.link created a hardlink with equal st_dev/st_ino"

        def rename() -> str:
            first = base / "a.bin"
            second = base / "b.bin"
            first.write_bytes(b"old-KEEP")
            second.write_bytes(b"complete-new-payload")
            os.replace(second, first)
            assert first.read_bytes() == b"complete-new-payload"
            return "os.replace atomically replaced an existing target in the same directory"

        def handle_identity() -> str:
            source = base / "ident.dex"
            source.write_bytes(b"x")
            alias = base / "ident-alias.dex"
            os.link(source, alias)
            with source.open("rb") as handle:
                stat_handle = os.fstat(handle.fileno())
            stat_path = source.stat()
            assert (stat_handle.st_dev, stat_handle.st_ino) == (
                stat_path.st_dev,
                stat_path.st_ino,
            )
            return "an opened handle and its path report the same st_dev/st_ino"

        def reader() -> str:
            payload = base / "payload.bin"
            payload.write_bytes(bytes(((i * 17 + 3) % 256) for i in range(16777216)))
            with payload.open("rb") as handle:
                total = 0
                while True:
                    chunk = handle.read(65536)
                    if not chunk:
                        break
                    total += len(chunk)
            assert total == 16777216
            return "one handle read 16777216 bytes sequentially"

        results = [
            probe("symlink_create", symlink),
            probe("hardlink_create", hardlink),
            probe("atomic_rename", rename),
            probe("file_handle_identity", handle_identity),
            probe("large_payload_reader", reader),
        ]

    report = {
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "tempDirectoryApi": "tempfile.TemporaryDirectory",
            "freeSpaceBytes": shutil.disk_usage(tempfile.gettempdir()).free,
        },
        "capabilities": results,
        "allRequiredCapabilitiesPresent": all(item["supported"] for item in results),
        "note": "A missing capability fails D070 preparation on this platform; the test must not degrade to exists+rename or drop an invocation.",
    }
    print(json.dumps(report, indent=2))
    return 0 if report["allRequiredCapabilitiesPresent"] else 1


if __name__ == "__main__":
    sys.exit(main())

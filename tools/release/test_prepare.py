import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.11 or newer is required by the release workflow.
    raise SystemExit("release tests require Python 3.11 or newer")


SCRIPTS = Path(__file__).resolve().parent


def command(args, cwd, env=None, check=True):
    return subprocess.run(
        args, cwd=cwd, env=env, text=True, capture_output=True, check=check
    )


class PrepareReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.repo = self.base / "repo"
        self.remote = self.base / "remote.git"
        self.repo.mkdir()
        (self.repo / "src").mkdir()
        (self.repo / "src/main.rs").write_text("fn main() {}\n")
        (self.repo / "Cargo.toml").write_text(
            '[package]\nname = "rasc"\nversion = "1.0.0"\nedition = "2021"\n'
            '[workspace]\nmembers = ["crates/dexdec", "crates/rusty-dex"]\n'
            'resolver = "2"\n[dependencies]\ndexdec = { path = "crates/dexdec" }\n'
        )
        for crate in ("dexdec", "rusty-dex"):
            directory = self.repo / "crates" / crate
            (directory / "src").mkdir(parents=True)
            (directory / "src/lib.rs").write_text("")
            extra = (
                '[dependencies]\nrusty-dex = { path = "../rusty-dex", version = "1.0.0" }\n'
                if crate == "dexdec"
                else ""
            )
            (directory / "Cargo.toml").write_text(
                f'[package]\nname = "{crate}"\nversion = "1.0.0"\nedition = "2021"\n'
                + extra
            )
        scripts = self.repo / "tools/release"
        scripts.mkdir(parents=True)
        for name in ("prepare.sh", "prepare_version.py"):
            shutil.copy(SCRIPTS / name, scripts / name)
        command(["cargo", "generate-lockfile", "--offline"], self.repo)
        command(["git", "init", "-q", "--bare", str(self.remote)], self.base)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("add", ".")
        self.git("commit", "-qm", "initial")
        self.git("remote", "add", "origin", str(self.remote))
        self.git("push", "-qu", "origin", "main")
        self.git("checkout", "-q", "--detach")

        self.bin = self.base / "bin"
        self.bin.mkdir()
        gh = self.bin / "gh"
        gh.write_text(
            '#!/bin/sh\ncase "${MOCK_RELEASE_STATE:-missing}" in\n'
            '  missing) echo "gh: Not Found (HTTP 404)" >&2; exit 1;;\n'
            '  draft) echo true;;\n'
            '  published) echo false;;\n'
            '  error) echo "gh: Server Error (HTTP 500)" >&2; exit 1;;\n'
            'esac\n'
        )
        gh.chmod(0o755)
        self.output = self.base / "output"

    def git(self, *args):
        return command(["git", *args], self.repo).stdout.strip()

    def prepare(self, version="1.0.1", **extra_env):
        self.output.write_text("")
        env = os.environ.copy()
        env.update(
            {
                "PATH": str(self.bin) + os.pathsep + env["PATH"],
                "RELEASE_VERSION": version,
                "GITHUB_REF": "refs/heads/main",
                "GITHUB_REPOSITORY": "example/rasc",
                "GITHUB_OUTPUT": str(self.output),
                **extra_env,
            }
        )
        result = command(["bash", "tools/release/prepare.sh"], self.repo, env, False)
        outputs = dict(line.split("=", 1) for line in self.output.read_text().splitlines())
        return result, outputs

    def test_new_release_commits_versions_and_pushes_without_tag(self):
        old = self.git("rev-parse", "HEAD")
        result, outputs = self.prepare()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["version"], "1.0.1")
        self.assertEqual(outputs["tag"], "rasc-v1.0.1")
        self.assertNotEqual(outputs["commit"], old)
        self.assertEqual(outputs["commit"], self.git("rev-parse", "origin/main"))
        self.assertEqual(self.git("tag", "-l"), "")
        for path in ("Cargo.toml", "crates/dexdec/Cargo.toml", "crates/rusty-dex/Cargo.toml"):
            manifest = tomllib.loads((self.repo / path).read_text())
            self.assertEqual(manifest["package"]["version"], "1.0.1")
        dexdec = tomllib.loads((self.repo / "crates/dexdec/Cargo.toml").read_text())
        self.assertEqual(dexdec["dependencies"]["rusty-dex"]["version"], "1.0.1")
        lock = tomllib.loads((self.repo / "Cargo.lock").read_text())
        self.assertEqual({p["version"] for p in lock["package"]}, {"1.0.1"})
        changed = set(self.git("diff", "--name-only", old, "HEAD").splitlines())
        self.assertEqual(changed, {"Cargo.toml", "Cargo.lock", "crates/dexdec/Cargo.toml", "crates/rusty-dex/Cargo.toml"})

    def test_existing_unpublished_tag_retries_exact_commit(self):
        result, outputs = self.prepare()
        self.assertEqual(result.returncode, 0, result.stderr)
        release_commit = outputs["commit"]
        self.git("tag", outputs["tag"])
        self.git("push", "origin", outputs["tag"])
        (self.repo / "later.txt").write_text("later change\n")
        self.git("add", "later.txt")
        self.git("commit", "-qm", "later change")
        self.git("push", "origin", "HEAD:refs/heads/main")
        later_commit = self.git("rev-parse", "HEAD")
        result, outputs = self.prepare(MOCK_RELEASE_STATE="draft")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["commit"], release_commit)
        self.assertEqual(self.git("rev-parse", "origin/main"), later_commit)

    def test_published_or_unreachable_release_is_not_changed(self):
        old = self.git("rev-parse", "HEAD")
        for state in ("published", "error"):
            with self.subTest(state=state):
                result, outputs = self.prepare(MOCK_RELEASE_STATE=state)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(outputs, {})
                self.assertEqual(self.git("rev-parse", "origin/main"), old)
                self.assertEqual(self.git("status", "--porcelain"), "")

    def test_invalid_and_older_versions_do_not_change_main(self):
        old = self.git("rev-parse", "HEAD")
        for version in ("v1.2.0", "1.02.0", "1.2.0-rc.1", "1.2", "0.9.9"):
            with self.subTest(version=version):
                result, outputs = self.prepare(version)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(outputs, {})
                self.assertEqual(self.git("rev-parse", "origin/main"), old)
                self.assertEqual(self.git("status", "--porcelain"), "")

    def test_unexpected_workspace_version_does_not_partially_update_files(self):
        manifest = self.repo / "crates/rusty-dex/Cargo.toml"
        manifest.write_text(manifest.read_text().replace('version = "1.0.0"', 'version = "0.9.0"'))
        self.git("add", str(manifest))
        self.git("commit", "-qm", "mismatched version")
        self.git("push", "origin", "HEAD:refs/heads/main")
        before = (self.repo / "Cargo.toml").read_text()
        result, _ = self.prepare()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("expected package rusty-dex", result.stderr)
        self.assertEqual((self.repo / "Cargo.toml").read_text(), before)
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_existing_tag_with_wrong_version_is_rejected(self):
        self.git("tag", "rasc-v1.0.1")
        self.git("push", "origin", "rasc-v1.0.1")
        result, _ = self.prepare()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("has Cargo version 1.0.0", result.stderr)

    def test_publication_retries_a_partially_uploaded_draft(self):
        result, outputs = self.prepare()
        self.assertEqual(result.returncode, 0, result.stderr)
        workflow = (SCRIPTS.parents[1] / ".github/workflows/release.yml").read_text()
        publish = workflow.split("      - name: Tag and publish release\n", 1)[1]
        lines = publish.split("        run: |\n", 1)[1].splitlines()
        script_lines = []
        for line in lines:
            if line and not line.startswith("          "):
                break
            script_lines.append(line[10:] if line else "")
        script = "\n".join(script_lines)

        gh = self.bin / "gh"
        gh.write_text(
            '#!/bin/sh\nset -eu\nprintf "%s\\n" "$*" >> "$MOCK_LOG"\n'
            'if [ "$1" = api ]; then\n'
            '  if [ -f "$MOCK_STATE" ]; then cat "$MOCK_STATE"; exit 0; fi\n'
            '  echo "gh: Not Found (HTTP 404)" >&2; exit 1\n'
            'fi\n'
            'case "$2" in\n'
            '  create) echo true > "$MOCK_STATE";;\n'
            '  upload) [ "$(cat "$MOCK_STATE")" = true ]; '
            'if [ "${MOCK_UPLOAD_FAIL:-0}" = 1 ]; then exit 1; fi;;\n'
            '  edit) [ "$(cat "$MOCK_STATE")" = true ]; echo false > "$MOCK_STATE";;\n'
            '  *) exit 1;;\n'
            'esac\n'
        )
        artifacts = self.repo / "artifacts"
        artifacts.mkdir()
        (artifacts / "rasc.tar.gz").write_text("fixture")
        state = self.base / "state"
        log = self.base / "gh.log"
        env = os.environ.copy()
        env.update(
            {
                "PATH": str(self.bin) + os.pathsep + env["PATH"],
                "GITHUB_REPOSITORY": "example/rasc",
                "RELEASE_TAG": outputs["tag"],
                "RELEASE_COMMIT": outputs["commit"],
                "MOCK_STATE": str(state),
                "MOCK_LOG": str(log),
                "MOCK_UPLOAD_FAIL": "1",
            }
        )
        failed = command(["bash", "-c", script], self.repo, env, False)
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual(state.read_text().strip(), "true")
        self.git("fetch", "-q", "--tags", "origin")
        self.assertEqual(self.git("rev-parse", outputs["tag"]), outputs["commit"])

        env["MOCK_UPLOAD_FAIL"] = "0"
        retried = command(["bash", "-c", script], self.repo, env, False)
        self.assertEqual(retried.returncode, 0, retried.stderr)
        self.assertEqual(state.read_text().strip(), "false")
        calls = log.read_text().splitlines()
        self.assertEqual(sum(line.startswith("release create ") for line in calls), 1)
        self.assertEqual(sum(line.startswith("release upload ") for line in calls), 2)
        self.assertTrue(all("--clobber" in line for line in calls if line.startswith("release upload ")))
        self.assertEqual(sum(line.startswith("release edit ") for line in calls), 1)

        published = command(["bash", "-c", script], self.repo, env, False)
        self.assertNotEqual(published.returncode, 0)
        self.assertIn("already published", published.stderr)
        self.assertEqual(state.read_text().strip(), "false")

    def test_only_current_main_can_be_released(self):
        result, _ = self.prepare(GITHUB_REF="refs/heads/feature")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("main branch", result.stderr)
        other = self.base / "other"
        command(["git", "clone", "-q", "--branch", "main", str(self.remote), str(other)], self.base)
        command(["git", "config", "user.name", "Test"], other)
        command(["git", "config", "user.email", "test@example.invalid"], other)
        command(["git", "config", "commit.gpgsign", "false"], other)
        (other / "later.txt").write_text("concurrent change\n")
        command(["git", "add", "later.txt"], other)
        command(["git", "commit", "-qm", "concurrent change"], other)
        command(["git", "push", "-q", "origin", "HEAD:main"], other)
        result, _ = self.prepare()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("main changed", result.stderr)


if __name__ == "__main__":
    unittest.main()

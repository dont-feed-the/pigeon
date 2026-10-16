#!/usr/bin/env python3
"""A publication retry resolves the original tag, never its workflow branch."""
import importlib.machinery
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/release-target"
loader = importlib.machinery.SourceFileLoader("release_target", str(SCRIPT))
spec = importlib.util.spec_from_loader(loader.name, loader)
target = importlib.util.module_from_spec(spec)
loader.exec_module(target)


class Target(unittest.TestCase):
    def test_tag_source_survives_a_newer_workflow_and_wrong_versions_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            previous = Path.cwd()
            os.chdir(directory)
            try:
                def git(*args):
                    return subprocess.check_output(["git", *args], stderr=subprocess.DEVNULL, text=True).strip()
                git("init", "-q")
                git("config", "user.name", "Fixture")
                git("config", "user.email", "fixture@example.invalid")
                manifest = Path("Cargo.toml")
                manifest.write_text('[workspace.package]\nversion = "0.4.0"\n')
                git("add", "Cargo.toml")
                git("commit", "-qm", "original release")
                old = git("rev-parse", "HEAD")
                git("tag", "-a", "v0.4.0", "-m", "original tag")
                manifest.write_text('[workspace.package]\nversion = "0.4.1"\n')
                git("commit", "-qam", "corrected workflow")
                self.assertEqual(target.resolve("v0.4.0"), old)
                self.assertEqual(git("rev-parse", "v0.4.0^{commit}"), old)
                git("tag", "v0.4.7", old)
                with self.assertRaises(ValueError):
                    target.resolve("v0.4.7")
                for invalid in ["main", "--all", "v0.4.0\nsha=main", "v0.4.0/other"]:
                    with self.assertRaises(ValueError):
                        target.resolve(invalid)
                manifest.write_text('[workspace.package]\nversion = "1.0.0-rc.1"\n')
                git("commit", "-qam", "release candidate")
                git("tag", "v1.0.0-rc.1")
                self.assertEqual(target.resolve("v1.0.0-rc.1"), git("rev-parse", "HEAD"))
            finally:
                os.chdir(previous)


if __name__ == "__main__":
    unittest.main()

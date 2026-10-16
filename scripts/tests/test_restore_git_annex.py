"""Dependency fallback provenance, archive isolation and binary checks."""

import hashlib
import importlib.machinery
import importlib.util
import io
import os
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
LOADER = importlib.machinery.SourceFileLoader("restore_annex", str(ROOT / "scripts/restore-git-annex-build"))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)


def elf(machine=62):
    data = bytearray(64)
    data[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<H", data, 18, machine)
    return bytes(data)


def packed(name="bin/git-annex", data=None, link=None):
    tar = io.BytesIO()
    with tarfile.open(fileobj=tar, mode="w:gz") as archive:
        member = tarfile.TarInfo(name)
        member.mode = 0o755
        if link is None:
            data = elf() if data is None else data
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
        else:
            member.type = tarfile.SYMTYPE
            member.linkname = link
            archive.addfile(member)
    zipped = io.BytesIO()
    with zipfile.ZipFile(zipped, "w") as archive:
        archive.writestr("git-annex-pigeon-ubuntu-24.04.tar.gz", tar.getvalue())
    result = zipped.getvalue()
    return result, "sha256:" + hashlib.sha256(result).hexdigest()


class Restore(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        previous = Path.cwd()
        os.chdir(self.root)
        self.addCleanup(os.chdir, previous)
        (self.root / "third_party/git-annex").mkdir(parents=True)
        (self.root / "third_party/git-annex/VERSION").write_text("PIGEON_VERSION=fixture-pigeon1\n")
        self.data, self.digest = packed()
        self.api = Mock()
        self.api.read.return_value = self.data
        self.run = {"id": 123, "event": "push", "head_sha": "a" * 40,
                    "head_repository": {"full_name": "owner/pigeon"}}
        self.conclusion = "success"
        self.comparison = "ahead"

        def json(path):
            if "/compare/" in path:
                return {"status": self.comparison}
            if "/jobs?" in path:
                return {"jobs": [{"name": "git-annex / git-annex (ubuntu-24.04)", "conclusion": self.conclusion}]}
            if "/artifacts?" in path:
                return {"artifacts": [{"id": 456, "name": "git-annex-pigeon-ubuntu-24.04",
                                       "expired": False, "size_in_bytes": len(self.data), "digest": self.digest}]}
            self.fail("unexpected API request")
        self.api.json.side_effect = json

    def restore(self, source=True, version="fixture-pigeon1"):
        with patch.object(MODULE, "runs", return_value=[self.run]), \
             patch.object(MODULE, "git", return_value=SimpleNamespace(stdout="a" * 40)), \
             patch.object(MODULE, "matching_source", return_value=source), \
             patch.object(MODULE.subprocess, "run", return_value=SimpleNamespace(stdout=version)):
            return MODULE.restore(self.api, "owner/pigeon", "Linux", "X64", self.root / "destination")

    def test_successful_matching_producer_restores_its_verified_binary(self):
        self.assertTrue(self.restore())
        self.assertEqual((self.root / "destination/bin/git-annex").read_bytes(), elf())

    def test_another_repository_cannot_supply_a_binary(self):
        self.run["head_repository"]["full_name"] = "fork/pigeon"
        self.assertFalse(self.restore())
        self.api.read.assert_not_called()

    def test_an_unmerged_commit_cannot_supply_a_binary(self):
        self.run["head_sha"] = "b" * 40
        self.comparison = "diverged"
        self.assertFalse(self.restore())
        self.api.read.assert_not_called()

    def test_changed_sources_cannot_supply_a_binary(self):
        self.assertFalse(self.restore(source=False))
        self.api.read.assert_not_called()

    def test_a_failed_producer_cannot_supply_a_binary(self):
        self.conclusion = "failure"
        self.assertFalse(self.restore())
        self.api.read.assert_not_called()

    def test_a_pull_request_run_cannot_supply_a_binary(self):
        self.run["event"] = "pull_request"
        self.assertFalse(self.restore())
        self.api.read.assert_not_called()

    def test_missing_or_changed_digest_refuses_before_extraction(self):
        for digest in (None, "sha256:" + "0" * 64):
            with self.assertRaises(ValueError):
                MODULE.extract(self.data, digest, self.root / "extracted")
        self.assertFalse((self.root / "extracted").exists())

    def test_path_escape_and_escaping_symlink_are_rejected(self):
        for name, link in (("bin/../../escaped", None), ("bin/escape", "../../escaped")):
            data, digest = packed(name=name, link=link)
            with self.assertRaises((ValueError, tarfile.TarError)):
                MODULE.extract(data, digest, self.root / "extracted")
        self.assertFalse((self.root / "escaped").exists())

    def test_wrong_architecture_and_version_are_not_installed(self):
        self.data, self.digest = packed(data=elf(183))
        self.api.read.return_value = self.data
        self.assertFalse(self.restore())
        self.assertFalse((self.root / "destination").exists())
        self.data, self.digest = packed()
        self.api.read.return_value = self.data
        with self.assertRaises(ValueError):
            self.restore(version="stock")
        self.assertFalse((self.root / "destination").exists())

    def test_signed_download_redirect_drops_github_authorization(self):
        request = urllib.request.Request("https://api.github.com/artifact", headers={"Authorization": "Bearer fixture"})
        redirected = MODULE.Redirect().redirect_request(request, None, 302, "found", {}, "https://storage.example/signed")
        self.assertIsNone(redirected.get_header("Authorization"))
        with self.assertRaises(ValueError):
            MODULE.Redirect().redirect_request(request, None, 302, "found", {}, "http://storage.example/signed")

    def test_native_macos_cpu_type_is_checked(self):
        binary = self.root / "macho"
        binary.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", 0x100000c) + bytes(24))
        self.assertTrue(MODULE.architecture(binary, "ARM64"))
        self.assertFalse(MODULE.architecture(binary, "X64"))

    def test_source_fingerprint_covers_every_build_input_only(self):
        def git(*args):
            return subprocess.run(["/usr/bin/git", *args], check=True, capture_output=True,
                                  text=True).stdout.strip()

        git("init", "-q")
        git("config", "user.name", "Fixture")
        git("config", "user.email", "fixture@example.invalid")
        build = self.root / "third_party/git-annex/build.sh"
        patch_file = self.root / "third_party/git-annex/patches/fix.patch"
        patch_file.parent.mkdir()
        build.write_text("fixture build\n")
        patch_file.write_text("fixture patch\n")
        git("add", ".")
        git("commit", "-qm", "fixture")
        commit = git("rev-parse", "HEAD")
        self.assertTrue(MODULE.matching_source(commit))
        for path in (build, patch_file, self.root / MODULE.INPUTS[0]):
            original = path.read_bytes()
            path.write_bytes(original + b"changed\n")
            self.assertFalse(MODULE.matching_source(commit))
            path.write_bytes(original)
        patch_file.unlink()
        self.assertFalse(MODULE.matching_source(commit))
        git("checkout", "--", str(patch_file))
        (self.root / "unrelated.md").write_text("not a dependency input\n")
        self.assertTrue(MODULE.matching_source(commit))
        self.assertFalse(MODULE.matching_source("not-a-commit"))


if __name__ == "__main__":
    unittest.main()

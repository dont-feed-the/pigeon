"""Check that the tap uses the release's exact binaries and checksums."""

import importlib.machinery
import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "homebrew-formula"
LOADER = importlib.machinery.SourceFileLoader("homebrew_formula", str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


class Formula(unittest.TestCase):
    """An incomplete or ambiguous release never produces a formula."""

    def setUp(self):
        self.sums = (
            f"{'a' * 64}  pigeon-v0.3.2-macos-arm64.tar.gz\n"
            f"{'b' * 64}  git-annex-pigeon-macos-arm64.tar.gz\n"
        )

    def test_exact_release_and_annex_are_pinned(self):
        result = MODULE.formula("v0.3.2", self.sums, "dont-feed-the/pigeon")
        self.assertIn('version "0.3.2"', result)
        self.assertIn('/download/v0.3.2/pigeon-v0.3.2-macos-arm64.tar.gz', result)
        self.assertIn(f'sha256 "{"b" * 64}"', result)
        self.assertIn('process_type :background', result)
        self.assertNotIn('/latest/', result)

    def test_roadmap_versions_upgrade_past_the_legacy_alpha(self):
        current = MODULE.formula("v0.3.2", self.sums, "dont-feed-the/pigeon")
        legacy_sums = self.sums.replace("v0.3.2-macos", "v2.0.0-alpha.0-macos")
        legacy = MODULE.formula("v2.0.0-alpha.0", legacy_sums, "dont-feed-the/pigeon")
        self.assertIn("version_scheme 1", current)
        self.assertIn("version_scheme 0", legacy)

    def test_missing_or_duplicate_assets_are_refused(self):
        with self.assertRaises(KeyError):
            MODULE.formula("v0.3.3", self.sums, "dont-feed-the/pigeon")
        with self.assertRaises(ValueError):
            MODULE.formula("v0.3.2", self.sums * 2, "dont-feed-the/pigeon")

    def test_untrusted_identifiers_cannot_become_ruby(self):
        for tag, repo in (("v0.3.2\nend", "dont-feed-the/pigeon"), ("v0.3.2", 'bad/"repo')):
            with self.subTest(tag=tag, repo=repo), self.assertRaises(ValueError):
                MODULE.formula(tag, self.sums, repo)


if __name__ == "__main__":
    unittest.main()

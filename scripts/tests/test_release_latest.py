"""An advancing main never starves publication; old retries stay older."""

import importlib.machinery
import importlib.util
from pathlib import Path
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "release-latest"
LOADER = importlib.machinery.SourceFileLoader("release_latest", str(SCRIPT))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)


class Latest(unittest.TestCase):
    def test_new_publication_becomes_latest_without_consulting_unreleased_main(self):
        self.assertTrue(MODULE.should_latest("v0.5.5", "v0.3.6"))
        self.assertTrue(MODULE.should_latest("v0.5.5", "v0.5.5"))
        self.assertTrue(MODULE.should_latest("v0.10.0", "v0.9.9"))
        self.assertTrue(MODULE.should_latest("v1.0.0", None))

    def test_prereleases_and_old_retries_cannot_replace_a_later_stable_release(self):
        self.assertFalse(MODULE.should_latest("v0.5.5", "v0.6.0"))
        self.assertFalse(MODULE.should_latest("v1.0.0-rc.1", "v0.7.0"))
        self.assertFalse(MODULE.should_latest("v1.0.0-rc.1", None))
        with self.assertRaises(ValueError):
            MODULE.should_latest("main", "v0.3.6")


if __name__ == "__main__":
    unittest.main()

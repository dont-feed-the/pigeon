"""An unrelated green run never authorizes publication of a release."""

import importlib.machinery
import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/wait-release-checks"
LOADER = importlib.machinery.SourceFileLoader("release_checks", str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


def run(name, **overrides):
    """One completed run on the candidate tag."""
    return dict(id=1, name=name, head_sha="abc", head_branch="v0.3.2", event="push",
                status="completed", conclusion="success") | overrides


class Publication(unittest.TestCase):
    """All four exact-tag checks are required, and failures stop the release."""

    def test_only_the_same_commit_tag_and_event_count(self):
        runs = [run(n) for n in MODULE.REQUIRED]
        self.assertTrue(MODULE.assess(runs, "abc", "v0.3.2")[0])
        for change in ({"head_sha": "old"}, {"head_branch": "main"}, {"event": "pull_request"}):
            self.assertFalse(MODULE.assess([r | change for r in runs], "abc", "v0.3.2")[0])

    def test_missing_and_pending_checks_wait(self):
        self.assertFalse(MODULE.assess([], "abc", "v0.3.2")[0])
        self.assertFalse(MODULE.assess([run(n, status="queued") for n in MODULE.REQUIRED], "abc", "v0.3.2")[0])

    def test_failure_is_not_hidden_by_an_older_success(self):
        runs = [run(n) for n in MODULE.REQUIRED] + [run("slice", id=2, conclusion="failure")]
        with self.assertRaisesRegex(RuntimeError, "slice did not pass"):
            MODULE.assess(runs, "abc", "v0.3.2")


if __name__ == "__main__":
    unittest.main()

"""An unrelated green run never authorizes publication of a release."""

import importlib.machinery
import importlib.util
import json
import re
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
    """Exact-tag Linux checks gate publication; macOS runs in the background."""

    def test_each_new_slice_partition_is_mandatory_and_matches_the_workflow(self):
        text = (SCRIPT.parents[1] / ".github/workflows/slice.yml").read_text()
        partitions = json.loads(re.search(r"scenarios:.*\|\| '([^']+)'", text)[1])
        expected = {f"slice (ubuntu-24.04, {part})" for part in partitions}
        self.assertEqual({n for n in MODULE.LINUX_JOBS["slice"] if n.startswith("slice (")}, expected)
        jobs = [{"name": n, "status": "completed", "conclusion": "success"}
                for n in MODULE.LINUX_JOBS["slice"]]
        self.assertEqual(MODULE.linux_state("slice", jobs), "passed")
        for i in range(len(jobs)):
            partial = jobs[:i] + jobs[i+1:]
            self.assertNotEqual(MODULE.linux_state("slice", partial), "passed")
            for conclusion in ("failure", "cancelled", "skipped"):
                with self.assertRaisesRegex(RuntimeError, "Linux job failed"):
                    MODULE.linux_state("slice", partial + [jobs[i] | {"conclusion": conclusion}])

    def test_legacy_rest_is_complete_but_cannot_hide_missing_new_partitions(self):
        jobs = [{"name": n, "status": "completed", "conclusion": "success"}
                for n in MODULE.LEGACY_SLICE]
        self.assertEqual(MODULE.linux_state("slice", jobs), "passed")
        self.assertNotEqual(MODULE.linux_state("slice", jobs + [{
            "name": "slice (ubuntu-24.04, core)", "status": "completed", "conclusion": "success"
        }]), "passed")

    def test_old_random_family_cannot_hide_any_new_random_shard(self):
        jobs = [{"name": n, "status": "completed", "conclusion": "success"}
                for n in MODULE.FIXED_SLICE | MODULE.OLD_RANDOM]
        self.assertEqual(MODULE.linux_state("slice", jobs), "passed")
        for name in MODULE.CURRENT_RANDOM:
            partial = jobs + [{"name": name, "status": "completed", "conclusion": "success"}]
            self.assertNotEqual(MODULE.linux_state("slice", partial), "passed")
        for name in MODULE.OLD_RANDOM:
            self.assertNotEqual(MODULE.linux_state("slice", [j for j in jobs if j["name"] != name]), "passed")

    def test_random_shards_cover_fifty_cases_and_match_every_required_job(self):
        text = (SCRIPT.parents[1] / ".github/workflows/slice.yml").read_text().split("  s9-random:", 1)[1]
        shards = json.loads(re.search(r"shard:.*\|\| '([^']+)'", text)[1])
        cases = json.loads(re.search(r"cases:.*\|\| '([^']+)'", text)[1])[0]
        self.assertEqual(len(shards), len(set(shards)))
        self.assertEqual(len(shards) * cases, 50)
        self.assertEqual(MODULE.CURRENT_RANDOM, {
            f"S9, {cases} random crash schedules (ubuntu-24.04, shard {n})" for n in shards})
        self.assertEqual({shard * 2 + worker for shard in shards for worker in range(2)}, set(range(10)))
        self.assertIn('name: S9, ${{ matrix.cases }} random crash schedules', text)
        self.assertIn('S9_CASES: ${{ matrix.cases }}', text)
        stride = int(re.search(r"S9_SEED_STRIDE:.*&& '([^']+)'", text)[1])
        streams = {shard * 2 + worker for shard in shards for worker in range(2)}
        self.assertGreaterEqual(stride, max(streams) + 1)
        self.assertIn('--cases "$S9_CASES" --jobs "$S9_JOBS"', text)
        self.assertIn('RUN_ID * S9_SEED_STRIDE + SHARD * 2', text)

    def test_old_tags_can_use_complete_linux_jobs_while_macos_is_queued_or_failed(self):
        runs = [run(n, id=i, status="queued", conclusion=None) for i, n in enumerate(sorted(MODULE.REQUIRED), 1)]
        jobs = {r["id"]: [{"name": n, "status": "completed", "conclusion": "success"}
                          for n in MODULE.LINUX_JOBS[r["name"]]]
                + [{"name": "Rust workspace (macos-latest)", "status": "completed", "conclusion": "failure"},
                   {"name": "slice (macos-latest, rest)", "status": "queued", "conclusion": None}]
                for r in runs}
        self.assertTrue(MODULE.assess(runs, "abc", "v0.3.2", jobs)[0])
        jobs[runs[0]["id"]].pop(0)
        self.assertFalse(MODULE.assess(runs, "abc", "v0.3.2", jobs)[0])

    def test_linux_failure_cancellation_or_skipped_required_job_never_passes(self):
        for conclusion in ("failure", "cancelled", "skipped"):
            jobs = [{"name": n, "status": "completed", "conclusion": "success"} for n in MODULE.LINUX_JOBS["ci"]]
            jobs[0]["conclusion"] = conclusion
            with self.assertRaisesRegex(RuntimeError, "Linux job failed"):
                MODULE.linux_state("ci", jobs)

    def test_running_jobs_do_not_hide_an_already_failed_linux_job(self):
        jobs = [{"name": "Rust workspace (ubuntu-24.04)", "status": "queued", "conclusion": None},
                {"name": "repository rules", "status": "completed", "conclusion": "failure"}]
        with self.assertRaisesRegex(RuntimeError, "repository rules"):
            MODULE.linux_state("ci", jobs)

    def test_only_the_same_commit_tag_and_event_count(self):
        runs = [run(n) for n in MODULE.REQUIRED]
        self.assertTrue(MODULE.assess(runs, "abc", "v0.3.2")[0])
        for change in ({"head_sha": "old"}, {"head_branch": "main"}, {"event": "pull_request"}, {"event": "workflow_dispatch"}):
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

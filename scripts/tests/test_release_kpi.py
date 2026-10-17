"""Publication, pagination and unfinished-workflow KPI regression evidence."""

from datetime import datetime, timezone
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "release-kpi"
LOADER = importlib.machinery.SourceFileLoader("release_kpi", str(SCRIPT))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)
NOW = datetime(2026, 10, 16, 13, tzinfo=timezone.utc)


def release(tag="v0.3.6", at="2026-10-16T06:40:24Z", **changes):
    return {"tag_name": tag, "published_at": at, "draft": False, "prerelease": False,
            "html_url": "https://example.test/release", "assets": [{"name": n} for n in MODULE.required_assets(tag)]} | changes


def run(tag, number=1, **changes):
    return {"id": number, "head_branch": tag, "created_at": "2026-10-16T03:03:41Z",
            "updated_at": "2026-10-16T06:40:27Z", "status": "completed", "conclusion": "success",
            "html_url": "https://example.test/run"} | changes


class PublicationKPI(unittest.TestCase):
    def test_completed_candidate_duration_stops_while_publication_age_keeps_growing(self):
        report = MODULE.build_report([release()], [run("v0.3.6")], NOW)
        candidate = report["latest_candidate"]
        self.assertEqual(candidate["elapsed_seconds"], 13006)
        self.assertGreater(candidate["age_seconds"], candidate["elapsed_seconds"])
        self.assertIn("success, elapsed 3h 36m", MODULE.markdown(report))
        running = MODULE.build_report([], [run("v0.3.6", status="in_progress", conclusion=None)], NOW)
        self.assertEqual(running["latest_candidate"]["elapsed_seconds"], candidate["age_seconds"])

    def test_completed_gap_miss_survives_the_new_publication_resetting_the_clock(self):
        published = [release("v0.5.9", "2026-10-16T10:00:00Z"),
                     release("v0.5.12", "2026-10-16T13:00:00Z"),
                     release("v0.5.10", "2026-10-16T11:00:00Z", draft=True),
                     release("v0.5.11", "2026-10-16T12:00:00Z", assets=[])]
        report = MODULE.build_report(published, [run("v0.5.13")], NOW)
        self.assertEqual(report["publication_gap_seconds"], 0)
        self.assertFalse(report["publication_gap_target_breached"])
        self.assertEqual(report["completed_publication_gap_sample_count"], 1)
        self.assertEqual(report["completed_publication_gaps"], [{
            "previous_tag": "v0.5.9", "tag": "v0.5.12", "from": "2026-10-16T10:00:00Z",
            "to": "2026-10-16T13:00:00Z", "seconds": 10800, "target_breached": True}])
        self.assertIn("v0.5.9 → v0.5.12: 3h 0m; target missed", MODULE.markdown(report))

    def test_one_publication_has_no_completed_interval_to_invent(self):
        report = MODULE.build_report([release()], [], NOW)
        self.assertEqual(report["completed_publication_gaps"], [])
        self.assertEqual(report["completed_publication_gap_sample_count"], 0)
        self.assertIn("0 samples", MODULE.markdown(report))

    def test_tags_drafts_and_incomplete_assets_cannot_reset_publication_gap(self):
        releases = [release(), release("v0.5.0", "2026-10-16T12:00:00Z", draft=True),
                    release("v0.5.1", "2026-10-16T12:00:00Z", assets=[])]
        runs = [run("v0.3.6"), run("v0.5.5", 2, conclusion=None, status="queued",
                                  created_at="2026-10-16T12:28:22Z", updated_at="2026-10-16T12:30:00Z")]
        report = MODULE.build_report(releases, runs, NOW)
        self.assertEqual(report["last_installable_release"]["tag"], "v0.3.6")
        self.assertEqual(report["publication_gap_seconds"], 6 * 3600 + 19 * 60 + 36)
        self.assertTrue(report["publication_gap_target_breached"])
        self.assertEqual(report["publication_lead_times"], [{"tag": "v0.3.6", "seconds": 13003}])
        self.assertEqual(report["published_releases_missing_assets"][0]["tag"], "v0.5.1")

    def test_published_prerelease_counts_with_its_status_preserved(self):
        report = MODULE.build_report([release("v1.0.0-rc.1", "2026-10-16T12:45:00Z", prerelease=True)], [], NOW)
        self.assertTrue(report["last_installable_release"]["prerelease"])
        self.assertEqual(report["publication_gap_seconds"], 900)
        self.assertFalse(report["publication_gap_target_breached"])
        self.assertEqual(report["lead_time_sample_count"], 0)

    def test_overlapping_failure_and_retries_are_one_unpublished_candidate(self):
        runs = [run("v0.4.0", conclusion="failure", updated_at="2026-10-16T07:01:00Z"),
                run("v0.4.0", 2, conclusion="cancelled", updated_at="2026-10-16T07:15:00Z"),
                run("v0.3.5", 3, conclusion="cancelled", updated_at="2026-10-16T03:00:00Z"),
                run("main", 4)]
        report = MODULE.build_report([release()], runs, NOW)
        self.assertEqual(len(report["unpublished_candidates_since_last_publication"]), 1)
        self.assertEqual(report["unpublished_candidates_since_last_publication"][0]["state"], "cancelled")
        self.assertEqual(report["unattributed_non_tag_runs"], 1)

    def test_failed_jobs_in_unfinished_workflows_and_waits_are_visible(self):
        jobs = {"ci": [{"name": "macOS", "status": "completed", "conclusion": "failure",
                        "created_at": "2026-10-16T12:00:00Z", "started_at": "2026-10-16T12:15:00Z",
                        "steps": [{"name": "verified dependency", "conclusion": "failure"}]}]}
        report = MODULE.build_report([release()], [], NOW, jobs=jobs)
        job = report["latest_candidate_jobs_needing_attention"][0]
        self.assertEqual(job["failed_step"], "verified dependency")
        self.assertEqual(job["wait_before_start_seconds"], 900)

    def test_paginated_gh_responses_work_without_newer_slurp_flag(self):
        for collection, pages in [(None, [[1], [2]]), ("jobs", [{"jobs": [1]}, {"jobs": [2]}])]:
            with patch.object(MODULE.subprocess, "check_output", return_value="\n".join(map(json.dumps, pages))) as command:
                self.assertEqual(MODULE.api("owner/repo", "path", collection), [1, 2])
                self.assertNotIn("--slurp", command.call_args.args[0])

    def test_queued_job_started_at_placeholder_does_not_hide_its_wait(self):
        jobs = {"release": [{"name": "Mac archive", "status": "queued", "conclusion": None,
                             "created_at": "2026-10-16T12:38:08Z", "started_at": "2026-10-16T12:38:08Z"}]}
        report = MODULE.build_report([release()], [], NOW, jobs=jobs)
        self.assertEqual(report["latest_candidate_jobs_needing_attention"][0]["wait_before_start_seconds"], 1312)

    def test_dispatch_retry_is_attributed_to_its_original_tag_and_commit(self):
        retry = run("main", 9, display_title="release v0.5.5", head_sha="new-main")
        self.assertEqual(MODULE.candidate(retry), "v0.5.5")
        objects = [{"object": {"type": "tag", "sha": "annotated"}},
                   {"object": {"type": "commit", "sha": "original-tag-commit"}}]
        with patch.object(MODULE.subprocess, "check_output", side_effect=list(map(json.dumps, objects))):
            self.assertEqual(MODULE.target_sha("owner/repo", retry), "original-tag-commit")
        report = MODULE.build_report([release()], [run("v0.5.5", 1), retry], NOW)
        self.assertEqual(len(report["unpublished_candidates_since_last_publication"]), 1)
        self.assertEqual(report["latest_candidate"]["run_id"], 9)


if __name__ == "__main__":
    unittest.main()

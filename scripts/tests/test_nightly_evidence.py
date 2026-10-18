"""Actual calendar nights and complete Linux execution are required evidence."""
from datetime import datetime, timedelta, timezone
import importlib.machinery
import importlib.util
from pathlib import Path
import unittest

PATH = Path(__file__).resolve().parents[1] / 'nightly-evidence'
LOADER = importlib.machinery.SourceFileLoader('nightly_evidence', str(PATH))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
M = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(M)
NOW = datetime(2026, 10, 18, 5, 30, tzinfo=timezone.utc)


class ScheduledNights(unittest.TestCase):
    def fixture(self):
        runs = [dict(id=n + 1, event='schedule', head_branch='main', status='completed',
                     conclusion='success', run_attempt=1, head_sha='a' * 40,
                     created_at=(NOW - timedelta(days=n)).isoformat()) for n in range(7)]
        jobs = {r['id']: [dict(name=name, status='completed', conclusion='success')
                         for name in M.required_jobs()] for r in runs}
        return runs, jobs

    def test_seven_executed_calendar_nights_are_recorded(self):
        runs, jobs = self.fixture()
        self.assertTrue(M.report(runs, jobs, NOW)['seven_nights_recorded'])

    def test_tags_and_manual_runs_do_not_count(self):
        for event in ['push', 'workflow_dispatch']:
            runs, jobs = self.fixture()
            for r in runs:
                r['event'] = event
            self.assertEqual(M.report(runs, jobs, NOW)['consecutive_green_linux_nights'], 0)

    def test_duplicate_runs_cannot_replace_missing_days(self):
        runs, jobs = self.fixture()
        for r in runs:
            r['created_at'] = NOW.isoformat()
        self.assertEqual(M.report(runs, jobs, NOW)['consecutive_green_linux_nights'], 1)

    def test_any_failed_duplicate_breaks_the_day(self):
        runs, jobs = self.fixture()
        extra = dict(runs[0], id=99)
        runs.append(extra)
        jobs[99] = [dict(j) for j in jobs[1]]
        jobs[99][0]['conclusion'] = 'failure'
        self.assertFalse(M.report(runs, jobs, NOW)['nights'][0]['green'])

    def test_every_required_family_must_run_successfully(self):
        for name in M.required_jobs():
            for state in ['failure', 'cancelled', 'skipped', None]:
                runs, jobs = self.fixture()
                next(j for j in jobs[1] if j['name'] == name)['conclusion'] = state
                self.assertFalse(M.report(runs, jobs, NOW)['seven_nights_recorded'])
            runs, jobs = self.fixture()
            jobs[1] = [j for j in jobs[1] if j['name'] != name]
            self.assertFalse(M.report(runs, jobs, NOW)['seven_nights_recorded'])

    def test_a_missing_day_breaks_the_current_streak(self):
        runs, jobs = self.fixture()
        runs = [r for r in runs if r['id'] != 4]
        self.assertEqual(M.report(runs, jobs, NOW)['consecutive_green_linux_nights'], 3)

    def test_a_retaken_night_is_not_a_first_attempt_green(self):
        runs, jobs = self.fixture()
        runs[0]['run_attempt'] = 2
        self.assertFalse(M.report(runs, jobs, NOW)['seven_nights_recorded'])

    def test_mac_failure_is_retained_without_gating_linux(self):
        runs, jobs = self.fixture()
        jobs[1].append(dict(name='blocked slice scenarios (macos-latest)', status='completed', conclusion='failure'))
        runs[0]['conclusion'] = 'failure'
        result = M.report(runs, jobs, NOW)
        self.assertTrue(result['seven_nights_recorded'])
        self.assertEqual(result['nights'][0]['runs'][0]['macos_background'][0]['conclusion'], 'failure')

    def test_before_cron_the_current_night_is_not_yet_expected(self):
        runs, jobs = self.fixture()
        early = NOW.replace(hour=0)
        for r in runs:
            r['created_at'] = (M.instant(r['created_at']) - timedelta(days=1)).isoformat()
        self.assertTrue(M.report(runs, jobs, early)['seven_nights_recorded'])

    def test_weekly_skips_are_allowed_but_other_skips_are_not(self):
        runs, jobs = self.fixture()
        jobs[1].append(dict(name='independent TLC audits of proved configurations (shard ${{ matrix.shard }})',
                            status='completed', conclusion='skipped'))
        self.assertTrue(M.report(runs, jobs, NOW)['seven_nights_recorded'])
        jobs[1].append(dict(name='new Linux validation', status='completed', conclusion='skipped'))
        self.assertFalse(M.report(runs, jobs, NOW)['seven_nights_recorded'])


if __name__ == '__main__':
    unittest.main()

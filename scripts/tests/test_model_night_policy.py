"""Qualification dates select work, never manufacture a green model-night result."""
import importlib.machinery
import importlib.util
from datetime import datetime, timezone, timedelta
from pathlib import Path
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'ci-model-night'
LOADER = importlib.machinery.SourceFileLoader('model_night_policy', str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


class ModelNightPolicy(unittest.TestCase):
    def setUp(self):
        self.now = datetime(2026, 10, 18, 1, 17, tzinfo=timezone.utc)
        self.old = int((self.now - timedelta(days=10)).timestamp())

    def test_routine_schedule_still_skips_unchanged_main_after_25_hours(self):
        epoch = int(self.now.timestamp())
        self.assertTrue(MODULE.select('schedule', epoch - 89_999, self.now)['run'])
        self.assertFalse(MODULE.select('schedule', epoch - 90_000, self.now)['run'])
        self.assertFalse(MODULE.select('schedule', self.old, self.now)['run'])

    def test_campaign_runs_unchanged_main_until_exclusive_utc_midnight(self):
        self.assertTrue(MODULE.select('schedule', self.old, self.now, '2026-10-19')['run'])
        midnight = datetime(2026, 10, 19, tzinfo=timezone.utc)
        self.assertTrue(MODULE.select('schedule', self.old, midnight - timedelta(microseconds=1), '2026-10-19')['run'])
        self.assertFalse(MODULE.select('schedule', self.old, midnight, '2026-10-19')['run'])

    def test_malformed_campaign_dates_fail_instead_of_silently_selecting_work(self):
        for value in ['forever', '2026-02-30', '20261019', '2026-10-19T00:00:00Z']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                MODULE.select('schedule', self.old, self.now, value)

    def test_manual_and_tag_runs_and_monday_audits_keep_their_existing_tiers(self):
        self.assertEqual(MODULE.select('push', self.old, self.now), dict(run=True, audit=False))
        self.assertEqual(MODULE.select('workflow_dispatch', self.old, self.now), dict(run=True, audit=True))
        monday = datetime(2026, 10, 19, 1, 17, tzinfo=timezone.utc)
        self.assertEqual(MODULE.select('schedule', self.old, monday), dict(run=False, audit=True))

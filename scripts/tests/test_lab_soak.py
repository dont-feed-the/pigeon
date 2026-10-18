"""Elapsed-soak evidence rejects smoke, interruptions and absent fault coverage."""
from pathlib import Path
import runpy
import unittest

SOAK = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'lab-soak'))


class Qualification(unittest.TestCase):
    def report(self, **changes):
        result = dict(smoke=False, stage='completed', elapsed_seconds=72 * 3600,
                      cycles=864, backup_restarts=172, max_heartbeat_gap_seconds=2)
        result.update(changes)
        return result

    def test_smoke_cannot_qualify_even_with_a_long_elapsed_counter(self):
        self.assertFalse(SOAK['qualified'](self.report(smoke=True)))

    def test_elapsed_boundary_requires_all_real_seconds(self):
        self.assertFalse(SOAK['qualified'](self.report(elapsed_seconds=72 * 3600 - .001)))
        self.assertTrue(SOAK['qualified'](self.report()))

    def test_interrupted_or_failed_observation_cannot_qualify(self):
        for changes in [dict(stage='failed'), dict(error='KeyboardInterrupt'),
                        dict(max_heartbeat_gap_seconds=31), dict(backup_restarts=0),
                        dict(cycles=1)]:
            with self.subTest(changes=changes):
                self.assertFalse(SOAK['qualified'](self.report(**changes)))

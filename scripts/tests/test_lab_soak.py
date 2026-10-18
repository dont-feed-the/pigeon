"""Elapsed-soak evidence rejects smoke, interruptions and absent fault coverage."""
from pathlib import Path
import runpy
import unittest
from unittest.mock import patch

SOAK = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'lab-soak'))


class ProofDeadline(unittest.TestCase):
    def verify_after_hot_wait(self, seconds):
        """The backup pointer follows a valid ten-minute hot-path wait."""
        now = [0.0]
        heartbeats = []

        def tree(*_args):
            if now[0] < 610:
                raise RuntimeError('backup catalogue still has the prior version')

        with patch.dict(SOAK['REFERENCE'], verify_tree=tree,
                        objects=lambda *_args, **_kwargs: 1), \
                patch.object(SOAK['time'], 'monotonic', side_effect=lambda: now[0]), \
                patch.object(SOAK['time'], 'sleep', side_effect=lambda delay: now.__setitem__(0, now[0] + delay)):
            SOAK['wait_verified']([], ['repository'], {'file': 'hash'}, {'hash': 'hash'},
                                  seconds, lambda: heartbeats.append(now[0]))
        return now[0], heartbeats

    def test_specified_hot_path_wait_can_finish_its_independent_proof(self):
        elapsed, heartbeats = self.verify_after_hot_wait(SOAK['CYCLE_PROOF_SECONDS'])
        self.assertEqual(elapsed, 610)
        self.assertGreater(len(heartbeats), 1000)

    def test_original_five_minute_deadline_rejects_the_valid_hot_path(self):
        with self.assertRaisesRegex(RuntimeError, 'proof deadline expired'):
            self.verify_after_hot_wait(300)


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

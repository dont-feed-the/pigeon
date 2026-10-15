"""E25's stock hang budget must bound waits without hiding fetch failures.

The patched build must retain its requested trial count even when given a
stock-only budget. Mock the network so regression checks take no timeout waits.
"""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    'e25', Path(__file__).resolve().parents[1] / 'ga-e25-relay-stress.py')
e25 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e25)


class RelayBudgetTests(unittest.TestCase):
    def exercise(self, outcomes, budget):
        def fetch(*args, **kwargs):
            result = next(outcomes)
            if result == 'hang':
                raise subprocess.TimeoutExpired('git fetch', 20)
            return subprocess.CompletedProcess([], result, '', 'fetch failed' if result else '')

        with patch.object(e25, 'commit', return_value='wanted'), \
                patch.object(e25, 'run', side_effect=fetch), \
                patch.object(e25, 'git', return_value='wanted'):
            return e25.fetches('C', 'P', 500, budget)

    def test_nonconsecutive_hangs_stop_at_budget(self):
        hangs, wrong, failed, _, done = self.exercise(iter(['hang', 0, 'hang', 0, 'hang']), 3)
        self.assertEqual((hangs, wrong, failed, done), ([0, 2, 4], [], [], 5))

    def test_successful_fetches_keep_full_trial_count(self):
        self.assertEqual(self.exercise(iter([0] * 500), 3)[4], 500)

    def test_fetch_errors_survive_early_stop(self):
        hangs, wrong, failed, _, done = self.exercise(iter([1, 'hang']), 1)
        self.assertEqual((hangs, wrong, done), ([1], [], 2))
        self.assertEqual(failed, [(0, 'fetch failed')])

    def test_patched_build_ignores_stock_budget(self):
        daemon = Mock(pid=123)
        with patch.object(sys, 'argv', ['e25', '500', '--stop-after-hangs', '3']), \
                patch.object(e25, 'setup', return_value=('C', 'P')), \
                patch.object(e25, 'git', return_value='10-pigeon'), \
                patch.object(e25.os.path, 'lexists', return_value=True), \
                patch.object(e25.subprocess, 'Popen', return_value=daemon), \
                patch.object(e25.os, 'killpg'), \
                patch.object(e25, 'run', return_value=Mock(stdout='p2p-annex::address')), \
                patch.object(e25, 'fetches', return_value=([], [], [], 1, 500)) as fetches, \
                contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(SystemExit) as result:
                e25.main()
            self.assertEqual(result.exception.code, 0)
            fetches.assert_called_once_with('C', 'P', 500, None)


if __name__ == '__main__':
    unittest.main()

"""Real recovery retries replay quickly; corrupted observations still fail TLC."""

import copy
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
LOADER = importlib.machinery.SourceFileLoader('install_witness', str(ROOT / 'scripts/check-install-traces'))
CHECKER = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(CHECKER)


class InstallWitness(unittest.TestCase):
    def test_hidden_writer_retries_full_model_and_accepts(self):
        path = ROOT / 'models/tla/daemon-fixtures/release-hidden-writer.ndjson'
        events = [json.loads(line) for line in path.read_text().splitlines()]
        tlc = CHECKER.load('run-tlc')
        actual_run, actual_load = tlc.run, CHECKER.load
        verdicts = []

        def run(*args, **kwargs):
            result = actual_run(*args, **kwargs)
            verdicts.append(result[0])
            return result

        tlc.run = run
        with patch.object(CHECKER, 'load', side_effect=lambda name: tlc if name == 'run-tlc' else actual_load(name)), \
                patch.object(CHECKER, 'WITNESS_MIN_TRANSITIONS', 0):
            self.assertEqual(CHECKER.check(events, 'java', 4, diagnostics=False), 'trace:accepted')
        self.assertEqual(len(verdicts), 2)
        self.assertTrue(verdicts[0].startswith('trace:rejected:'), verdicts)
        self.assertEqual(verdicts[1], 'trace:accepted')

    def test_recorded_long_recovery_uses_a_full_bound_eager_witness(self):
        for suffix in ['a', 'b']:
            with self.subTest(path=suffix):
                events = self.events(suffix)
                tlc = CHECKER.load('run-tlc')
                actual_run, actual_load = tlc.run, CHECKER.load
                attempted = []

                def run(*args, models, **kwargs):
                    config = (models / 'CaptureInstallTrace_good.cfg').read_text()
                    attempted.append(config)
                    # A fallback here would hide a performance regression and
                    # run the original many-minute search during a unit test.
                    self.assertIn('SPECIFICATION EagerTraceSpec', config)
                    self.assertIn('EnvBudget = 4\n', config)
                    return actual_run(*args, models=models, **kwargs)

                tlc.run = run
                with patch.object(CHECKER, 'load', side_effect=lambda name: tlc if name == 'run-tlc' else actual_load(name)):
                    self.assertEqual(CHECKER.check(events, 'java', 4), 'trace:accepted')
                self.assertEqual(len(attempted), 1)

    def test_corruption_cannot_pass_either_search(self):
        for suffix in ['a', 'b']:
            with self.subTest(path=suffix):
                events = copy.deepcopy(self.events(suffix))
                first = next(i for i, event in enumerate(events)
                             if event['action'] == 'Capture' and event['vars']['edge'] == 'after')
                events[first]['vars']['state']['signed'] = events[first - 1]['vars']['state']['signed']
                tlc = CHECKER.load('run-tlc')
                actual_run, actual_load = tlc.run, CHECKER.load
                attempted = []

                def run(*args, models, **kwargs):
                    result = actual_run(*args, models=models, **kwargs)
                    self.assertTrue(result[0].startswith('trace:rejected:'), result)
                    attempted.append(result[0])
                    return result

                tlc.run = run
                with patch.object(CHECKER, 'load', side_effect=lambda name: tlc if name == 'run-tlc' else actual_load(name)):
                    got = CHECKER.check(events, 'java', 4, diagnostics=False)
                self.assertTrue(got.startswith('trace:rejected:'), got)
                self.assertEqual(len(attempted), 2)

    @staticmethod
    def events(suffix):
        path = ROOT / f'models/tla/daemon-fixtures/keep-recovery-retries-{suffix}.ndjson'
        return [json.loads(line) for line in path.read_text().splitlines()]


if __name__ == '__main__':
    unittest.main()

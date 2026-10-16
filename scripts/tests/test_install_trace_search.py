"""Witness shortcuts cannot turn a bounded rejection or timeout into success."""

import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    loader = importlib.machinery.SourceFileLoader(name.replace('-', '_'), str(ROOT / 'scripts' / name))
    module = importlib.util.module_from_spec(importlib.util.spec_from_loader(loader.name, loader))
    loader.exec_module(module)
    return module


CHECKER = load('check-install-traces')
EVENTS = [json.loads(line) for line in
          (ROOT / 'models/tla/daemon-fixtures/keep-restart-visible.ndjson').read_text().splitlines()]


class WitnessSearch(unittest.TestCase):
    def search(self, verdicts, budget=4, shortcut=True):
        configs, deadlines = [], []
        actual_load = CHECKER.load

        def run(*args, models, timeout):
            configs.append((models / 'CaptureInstallTrace_good.cfg').read_text())
            deadlines.append(timeout)
            return verdicts[len(configs) - 1], 'diagnostics'

        tlc = SimpleNamespace(run=run, tlc_jar=lambda: 'jar', excerpt=lambda text: text)
        with patch.object(CHECKER, 'load', side_effect=lambda name: tlc if name == 'run-tlc' else actual_load(name)), \
                patch.object(CHECKER, 'WITNESS_MIN_TRANSITIONS', 0 if shortcut else 1000):
            got = CHECKER.check(EVENTS, 'java', budget, diagnostics=False)
        return got, configs, deadlines

    def test_eager_witness_accepts_without_changing_bounds(self):
        got, configs, deadlines = self.search(['trace:accepted'])
        self.assertEqual(got, 'trace:accepted')
        self.assertEqual(len(configs), 1)
        self.assertIn('SPECIFICATION EagerTraceSpec\n', configs[0])
        self.assertIn('EnvBudget = 4\n', configs[0])
        self.assertEqual(deadlines, [60])

    def test_every_nonaccepting_shortcut_retries_the_original_problem(self):
        for first in ['trace:rejected:exhausted', 'error:timeout', 'error:tool']:
            for final in ['trace:accepted', 'trace:rejected:exhausted', 'error:timeout']:
                with self.subTest(first=first, final=final):
                    got, configs, deadlines = self.search([first, final])
                    self.assertEqual(got, final)
                    self.assertEqual(len(configs), 2)
                    self.assertEqual(configs[0].replace('SPECIFICATION EagerTraceSpec', 'SPECIFICATION TraceSpec'), configs[1])
                    self.assertEqual(deadlines, [60, None])

    def test_short_stream_skips_the_shortcut_and_retains_its_budget(self):
        for budget in [0, 1]:
            got, configs, deadlines = self.search(['trace:rejected:exhausted'], budget, shortcut=False)
            self.assertEqual(got, 'trace:rejected:exhausted')
            self.assertEqual(len(configs), 1)
            self.assertIn(f'EnvBudget = {budget}\n', configs[0])
            self.assertEqual(deadlines, [None])

    def test_deadline_kills_and_reaps_the_checker_and_returns_an_error(self):
        tlc = load('run-tlc')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            java = root / 'fake-java'
            pid = root / 'pid'
            java.write_text(f'#!/usr/bin/env python3\nimport os, time\n'
                            f'open({str(pid)!r}, "w").write(str(os.getpid()))\ntime.sleep(30)\n')
            java.chmod(0o755)
            got, output = tlc.run('jar', 'cfg', 'module', str(java), '1', None, models=root, timeout=1)
            self.assertEqual(got, 'error:timeout')
            self.assertIn('TLC exceeded 1 seconds', output)
            with self.assertRaises(ProcessLookupError):
                os.kill(int(pid.read_text()), 0)


if __name__ == '__main__':
    unittest.main()

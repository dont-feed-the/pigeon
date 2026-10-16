"""Parallel S9 workers retain every case, failure and isolated report."""

import contextlib
import importlib.machinery
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "check-s9-schedules"
LOADER = importlib.machinery.SourceFileLoader("s9_schedules", str(SCRIPT))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)


class ScheduleWorkers(unittest.TestCase):
    def test_parallel_failure_keeps_all_cases_and_reports(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            binary = root / "fake-test"
            binary.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys, time
report = pathlib.Path(os.environ['PIGEON_LAB_OUT'])
seed = os.environ['HEGEL_SEED']
(report.parent / ('started-' + seed)).touch()
# All workers must start before any finishes: this also detects accidental
# serialization of the executor and protects against timing-only assertions.
for _ in range(500):
    if len(list(report.parent.glob('started-*'))) == 3:
        break
    time.sleep(.01)
else:
    sys.exit(5)
keys = ['HEGEL_TEST_CASES', 'HEGEL_DATABASE', 'PIGEON_LAB_OUT', 'PIGEON_LAB_WORK']
settings = {key: os.environ[key] for key in keys}
settings['argv'] = sys.argv[1:]
(report / 'env.json').write_text(json.dumps(settings))
(report / 'timings.txt').write_text('case-' + seed + ' 1\\n')
if seed == '8':
    (report / 'failures.txt').write_text('bad case\\n')
    print('injected failure')
    sys.exit(1)
''')
            binary.chmod(0o755)
            for name in ["pigeond", "pigeon", "git-annex-p2p-iroh"]:
                (root / name).touch()
            out = root / "reports"
            argv = [str(SCRIPT), "--cases", "7", "--jobs", "3", "--seed", "7", "--out", str(out)]
            with patch.object(MODULE, "executable", return_value=binary), \
                    patch.dict(os.environ, PIGEON_LAB_BIN=str(root)), \
                    patch("sys.argv", argv), contextlib.redirect_stdout(io.StringIO()) as output:
                result = MODULE.main()
            self.assertEqual(result, 1)
            self.assertIn("injected failure", output.getvalue())
            envs = [json.loads((out / f"seed-{seed}/env.json").read_text()) for seed in [7, 8, 9]]
            self.assertEqual([int(e["HEGEL_TEST_CASES"]) for e in envs], [3, 2, 2])
            for key in ["HEGEL_DATABASE", "PIGEON_LAB_OUT", "PIGEON_LAB_WORK"]:
                self.assertEqual(len({e[key] for e in envs}), 3, key)
            self.assertEqual(envs[0]["argv"], ["--nocapture"])
            for e in envs[1:]:
                self.assertEqual(e["argv"], ["--nocapture", "--exact",
                    "random_crash_schedules_lose_and_duplicate_nothing"])
            self.assertEqual((out / "timings.txt").read_text().splitlines(),
                             [f"seed-{seed}/case-{seed} 1" for seed in [7, 8, 9]])
            self.assertEqual((out / "failures.txt").read_text(), "seed-8/bad case\n")


if __name__ == "__main__":
    unittest.main()

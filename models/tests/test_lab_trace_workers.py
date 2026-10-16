"""Real TLC verdicts and coverage stay intact with two isolated trace workers."""

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check-lab-traces"
GOOD = (ROOT / "models/tla/StorageTrace_good.ndjson").read_text()


def events(text):
    return [json.loads(line) for line in text.splitlines() if line.strip()]


def log(root, name, records):
    path = root / name / "trace.ndjson"
    path.parent.mkdir(exist_ok=True)
    path.write_text("".join(json.dumps(e) + "\n" for e in records))


def check(root, jobs, *extra):
    return subprocess.run([sys.executable, str(SCRIPT), str(root), "--jobs", str(jobs),
                           "--require", "StorageBoundary", *extra],
                          cwd=ROOT, capture_output=True, text=True, timeout=120)


class TraceWorkers(unittest.TestCase):
    def test_both_modes_accept_every_storage_and_recorded_filesystem_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in ("first", "second"):
                log(root, name, events(GOOD))
            fixture = ROOT / "models/tla/daemon-fixtures/keep-restart-visible.ndjson"
            recorded = events(fixture.read_text())
            # The fixture projects filesystem events from a longer run. The
            # merged-log envelope has consecutive clocks, preserving its order.
            clocks = {}
            for event in recorded:
                device = event["device"]
                clocks[device] = clocks.get(device, 0) + 1
                event["clock"] = clocks[device]
            log(root, "filesystem", recorded)
            outputs = []
            for jobs in (1, 2):
                out = check(root, jobs, "--require", "CaptureInstall")
                self.assertEqual(out.returncode, 0, out.stdout + out.stderr)
                accepted = {line for line in out.stdout.splitlines() if line.startswith("ok")}
                self.assertGreaterEqual(len(accepted), 3, out.stdout)
                outputs.append(accepted)
            self.assertEqual(*outputs)

    def test_valid_siblings_cannot_mask_any_bad_log_in_either_mode(self):
        for flaw in ("corrupt boundary", "unknown family", "missing field", "clock gap", "empty"):
            bad = copy.deepcopy(events(GOOD))
            if flaw == "corrupt boundary":
                bad[0]["vars"]["live"] = "overwritten"
            elif flaw == "unknown family":
                bad[0]["spec"] = "Unregistered"
            elif flaw == "missing field":
                del bad[0]["vars"]["pending"]
            elif flaw == "clock gap":
                bad[0]["clock"] += 1
            else:
                bad = []
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                log(root, "good", events(GOOD))
                log(root, "bad", bad)
                for jobs in (1, 2):
                    with self.subTest(flaw=flaw, jobs=jobs):
                        out = check(root, jobs)
                        self.assertNotEqual(out.returncode, 0, out.stdout + out.stderr)
                        self.assertIn("FAIL", out.stdout)
                        if flaw == "corrupt boundary":
                            self.assertIn("trace:rejected:", out.stdout)
                        if flaw == "missing field":
                            self.assertIn("required trace specification was not checked", out.stdout)

    def test_missing_required_family_and_empty_input_still_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            empty = check(root, 2)
            self.assertNotEqual(empty.returncode, 0)
            self.assertIn("FAIL no trace logs", empty.stdout)
            log(root, "good", events(GOOD))
            missing = check(root, 2, "--require", "CaptureInstall")
            self.assertNotEqual(missing.returncode, 0)
            self.assertIn("required trace specification was not checked: CaptureInstall", missing.stdout)

    def test_worker_failure_cannot_become_an_accepted_check(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            log(root, "good", events(GOOD))
            out = check(root, 2, "--java", str(root / "missing-java"))
            self.assertNotEqual(out.returncode, 0)
            self.assertNotIn("trace:accepted", out.stdout)

    def test_worker_count_is_bounded(self):
        for jobs in (0, 3):
            out = check(ROOT, jobs)
            self.assertEqual(out.returncode, 2)


if __name__ == "__main__":
    unittest.main()

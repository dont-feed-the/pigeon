"""Every current or future slice scenario belongs to exactly one runner.

Malformed manifests and empty selections fail instead of starting all tests
or silently omitting coverage. The workflow supplies the live lab catalogue.
"""
import importlib.machinery
import importlib.util
from pathlib import Path
import random
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "ci-slice-partition"
LOADER = importlib.machinery.SourceFileLoader("ci_slice_partition", str(SCRIPT))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)


class SlicePartitions(unittest.TestCase):
    def test_all_scenarios_and_future_ids_are_covered_once_in_1000_catalogues(self):
        randomizer = random.Random(60)
        for _ in range(1000):
            names = [f"S{n}" for n in range(1, randomizer.randrange(17, 100)) if n != 9]
            names.extend(["S9-capture-before-ref", "S9-free-after-index", "S9-keep-after-begin"])
            randomizer.shuffle(names)
            groups = MODULE.partition(names)
            self.assertCountEqual(names, [name for group in groups.values() for name in group])
            self.assertEqual(groups["S5"], ["S5"])
            self.assertIn("S16", groups["people"])
            self.assertEqual(set(groups["crashes"]), {name for name in names if name.startswith("S9-")})

    def test_malformed_catalogues_and_empty_partitions_fail_closed(self):
        for names in [[], ["S1", "S1"], ["garbage"], ["S0"], ["S1;exit"], ["S2/escape"]]:
            with self.assertRaises(ValueError):
                MODULE.partition(names)
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "list"
            manifest.write_text("S1 one scenario\nS5 disk isolation\nS14 people\n")
            empty = subprocess.run(["python3", str(SCRIPT), "recovery", str(manifest)], capture_output=True)
            self.assertNotEqual(empty.returncode, 0)
            self.assertEqual(empty.stdout, b"")
            good = subprocess.run(["python3", str(SCRIPT), "S5", str(manifest)], capture_output=True, check=True)
            self.assertEqual(good.stdout, b"S5\n")

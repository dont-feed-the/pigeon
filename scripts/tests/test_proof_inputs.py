"""A proof cache must miss when source paths, contents or provenance differ."""

import importlib.machinery
import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "proof-inputs"
LOADER = importlib.machinery.SourceFileLoader("proof_inputs", str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


class ProofInputs(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in (*MODULE.FIXED, "models/tla/Original.tla", "models/tla/Original.cfg"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("original\n")
        self.digest = MODULE.input_hash(self.root)

    def test_every_fixed_input_and_model_invalidates_a_result(self):
        for name in (*MODULE.FIXED, "models/tla/Original.tla", "models/tla/Original.cfg"):
            with self.subTest(name=name):
                path = self.root / name
                path.write_text("changed\n")
                self.assertNotEqual(MODULE.input_hash(self.root), self.digest)
                path.write_text("original\n")
                self.assertEqual(MODULE.input_hash(self.root), self.digest)

    def test_renaming_an_import_with_identical_bytes_invalidates_a_result(self):
        (self.root / "models/tla/Original.tla").rename(self.root / "models/tla/Renamed.tla")
        self.assertNotEqual(MODULE.input_hash(self.root), self.digest)

    def test_new_configuration_invalidates_a_result(self):
        (self.root / "models/tla/Additional.cfg").write_text("original\n")
        self.assertNotEqual(MODULE.input_hash(self.root), self.digest)

    def test_generated_fingerprints_and_unrelated_docs_do_not_change_inputs(self):
        cache = self.root / "models/tla/.tlacache"
        cache.mkdir()
        (cache / "Generated.tla").write_text("generated\n")
        (self.root / "README.md").write_text("documentation\n")
        self.assertEqual(MODULE.input_hash(self.root), self.digest)

    def test_only_matching_main_provenance_is_reusable(self):
        record = {"schema": 1, "input_hash": self.digest, "repository": "example/pigeon",
                  "ref": "refs/heads/main", "run_id": "123", "commit": "a" * 40}
        self.assertEqual(MODULE.verified_run(record, self.digest, "example/pigeon"),
                         "https://github.com/example/pigeon/actions/runs/123")
        for key, value in [("schema", 0), ("input_hash", "b" * 64),
                           ("repository", "other/pigeon"), ("ref", "refs/pull/46/merge"),
                           ("run_id", ""), ("commit", "bad")]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                MODULE.verified_run({**record, key: value}, self.digest, "example/pigeon")


if __name__ == "__main__":
    unittest.main()

"""Keep every configured safety claim covered and every audit independently runnable."""

import importlib.machinery
import importlib.util
import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch


def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


ROOT = Path(__file__).resolve().parents[2]
PROOFS = load("coverage_proofs", ROOT / "scripts/check-proofs")
TLC = load("coverage_tlc", ROOT / "scripts/run-tlc")


class ConfigurationCoverage(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.cfg = self.work / "T_n42o4.cfg"
        self.original = (ROOT / "models/tla/T_n42o4.cfg").read_text()
        self.cfg.write_text(self.original)
        # These checks inspect generated claims without installing a prover.
        self.checks = PROOFS.Checks.__new__(PROOFS.Checks)

    def theorem(self):
        module = self.checks.configuration_theorem(self.work, "CustodyTokensProof", "T_n42o4")
        return (self.work / f"{module}.tla").read_text()

    def test_plural_directive_preserves_all_four_token_guarantees(self):
        self.assertIn("Spec => [](TokensNeverLost /\\ TokenHolderHasContent /\\ "
                      "KCopies /\\ AtMostOneToken)", self.theorem())

    def test_indented_additional_claim_is_not_silently_ignored(self):
        self.cfg.write_text(self.original + "\n  INVARIANTS Extra Another \\* explanation\n")
        self.assertIn("AtMostOneToken /\\ Extra /\\ Another)", self.theorem())

    def test_continuation_names_are_not_silently_ignored(self):
        self.cfg.write_text(self.original.replace(" AtMostOneToken", "\n AtMostOneToken\n Extra"))
        self.assertIn("KCopies /\\ AtMostOneToken /\\ Extra)", self.theorem())

    def test_unsupported_claims_cannot_become_audits(self):
        for directive in ("INVARIANT next = 1", "PROPERTY EventuallyDone", "VIEW View",
                          "CONSTRAINT CutOff", "SYMMETRY Nodes", "Next <- OtherNext"):
            with self.subTest(directive=directive):
                self.cfg.write_text(self.original + "\n  " + directive + "\n")
                with self.assertRaises(RuntimeError):
                    self.theorem()
        self.cfg.write_text(self.original.replace("CHECK_DEADLOCK FALSE", "CHECK_DEADLOCK TRUE"))
        with self.assertRaises(RuntimeError):
            self.theorem()

    def test_empty_initial_state_is_refused_but_multiple_initial_states_are_valid(self):
        self.checks.java, self.checks.library, self.checks.jar = "java", self.work, self.work / "tlc.jar"
        for count in (0, 1, 16):
            with self.subTest(initial_states=count):
                self.cfg.write_text(self.original)
                output = ("Model checking completed. No error has been found.\n"
                          f"{count} distinct states found\n")
                with patch.object(self.checks, "run", return_value=(0, output)):
                    if count:
                        self.checks.coverage(self.work, "CustodyTokensProof", "T_n42o4")
                    else:
                        with self.assertRaises(RuntimeError):
                            self.checks.coverage(self.work, "CustodyTokensProof", "T_n42o4")


class ModelSelection(unittest.TestCase):
    def test_audits_are_separate_and_have_checked_proof_mappings(self):
        rows = list(TLC.expectations())
        audits = TLC.select_rows(rows, audit_only=True)
        self.assertEqual({row[0] for row in audits}, set(PROOFS.PROOFS.values()) -
                         {"CaptureTransaction_safe"})
        routine = TLC.select_rows(rows, full=True)
        self.assertFalse(set(audits) & set(routine))
        self.assertEqual(set(audits) | set(routine), set(TLC.select_rows(rows, audit=True)))

    def test_small_cases_negatives_and_liveness_remain_routine(self):
        fast = {row[0] for row in TLC.select_rows(TLC.expectations())}
        self.assertTrue({"T_nonce", "T_idem", "T_nononce", "T_n42o3", "T_n31o5", "T_n32o5"} <= fast)
        full = {row[0] for row in TLC.select_rows(TLC.expectations(), full=True)}
        self.assertTrue({"Evict_live2", "Evict_livekeep2", "Authority_decide4", "Evict_flip"} <= full)

    def test_explicit_names_still_run_audits_and_shards_partition_them(self):
        rows = list(TLC.expectations())
        self.assertEqual([row[0] for row in TLC.select_rows(rows, configs=["T_n42o4"])], ["T_n42o4"])
        audits = TLC.select_rows(rows, audit_only=True)
        for count in (1, 2, 4):
            shards = [audits[index::count] for index in range(count)]
            flattened = [row for shard in shards for row in shard]
            self.assertEqual(len(flattened), len(set(flattened)))
            self.assertEqual(set(flattened), set(audits))

    def test_balancing_preserves_all_rows_including_new_unmeasured_cases(self):
        rows = TLC.select_rows(TLC.expectations(), full=True)
        rows.append(("BrandNew", "NewModel", "full", "pass"))
        costs = TLC.model_costs()
        self.assertTrue(set(costs) <= {row[0] for row in TLC.expectations()})
        self.assertTrue(all(value > 0 for value in costs.values()))
        for count in (1, 3, 4):
            shards = [TLC.shard_rows(rows, index, count, costs) for index in range(count)]
            flattened = [row for shard in shards for row in shard]
            self.assertEqual(len(flattened), len(set(flattened)))
            self.assertEqual(set(flattened), set(rows))
            for shard in shards:
                self.assertEqual(shard, [row for row in rows if row in shard])

    def test_long_cases_do_not_pile_up_on_one_shard(self):
        rows = [(str(i), "Model", "full", "pass") for i in range(8)]
        costs = {str(i): value for i, value in enumerate((100, 1, 1, 1, 100, 1, 1, 1))}
        shards = [TLC.shard_rows(rows, index, 4, costs) for index in range(4)]
        self.assertTrue(all(sum(costs[row[0]] for row in shard) <= 100 for shard in shards))


class AuditSchedule(unittest.TestCase):
    def test_weekly_audit_survives_an_unchanged_main_and_releases_use_proofs(self):
        workflow = (ROOT / ".github/workflows/models-full.yml").read_text()
        fresh = workflow.split("      - id: fresh\n", 1)[1].split("\n  proofs:", 1)[0]
        script = textwrap.dedent(fresh.split("        run: |\n", 1)[1])
        mocks = ('date() { if [ "$*" = +%s ]; then echo 1000000; '
                 'else echo "$TEST_DAY"; fi; }\n'
                 'git() { echo "$TEST_COMMIT"; }\n')
        for event, day, recent, expected in (
                ("schedule", "1", False, {"audit=true"}),
                ("schedule", "2", False, set()),
                ("schedule", "1", True, {"run=true", "audit=true"}),
                ("schedule", "2", True, {"run=true"}),
                ("push", "1", False, {"run=true"}),
                ("workflow_dispatch", "2", False, {"run=true", "audit=true"})):
            with self.subTest(event=event, day=day, recent=recent), tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "output"
                output.touch()
                env = dict(os.environ, EVENT=event, TEST_DAY=day,
                           TEST_COMMIT=str(999940 if recent else 800000),
                           GITHUB_OUTPUT=str(output), GITHUB_STEP_SUMMARY=str(Path(temp) / "summary"))
                subprocess.run(["bash", "-eu", "-c", mocks + script], env=env, check=True)
                self.assertEqual(set(output.read_text().splitlines()), expected)


if __name__ == "__main__":
    unittest.main()

"""Fail-closed measurement contracts; reduced fixtures never certify a reference budget."""
import importlib.machinery
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'reference-budget'
LOADER = importlib.machinery.SourceFileLoader('reference_budget', str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


class ReferenceMeasurement(unittest.TestCase):
    def samples(self, cpu=5_000_000, rss=300_000_000, duration=600):
        return [dict(time=10, cpu_us=123, rss=rss),
                dict(time=10 + duration, cpu_us=123 + cpu, rss=rss)]

    def scans(self, **kwargs):
        return [dict(entries=100_000, pending=0, elapsed_us=1_000_000, **kwargs)]

    def test_all_reference_limits_are_required(self):
        self.assertEqual(MODULE.verdict(self.samples(), self.scans(), 100_000, 600, True)['budget_verdict'], 'passed')
        for samples, scans in [(self.samples(cpu=6_000_000), self.scans()),
                               (self.samples(rss=300_000_001), self.scans()),
                               (self.samples(duration=599), self.scans()),
                               (self.samples(), []),
                               (self.samples(), [dict(entries=99_999, pending=0, elapsed_us=1)])]:
            with self.subTest(samples=samples, scans=scans):
                self.assertEqual(MODULE.verdict(samples, scans, 100_000, 600, True)['budget_verdict'], 'failed')

    def test_smoke_never_certifies_budgets_even_with_good_numbers(self):
        result = MODULE.verdict(self.samples(), self.scans(), 32, 5, False)
        self.assertEqual(result['budget_verdict'], 'not-measured')

    def test_native_child_cpu_is_read_from_the_cgroup_total(self):
        self.assertEqual(MODULE.counters('usage_usec 9876543\nuser_usec 8000000\nsystem_usec 1876543\n'), 9876543)
        with self.assertRaises(KeyError):
            MODULE.counters('user_usec 8000000\n')
        with self.assertRaises(ValueError):
            MODULE.verdict(self.samples(cpu=-1), [], 100_000, 600, True)

    def test_process_name_parentheses_do_not_change_start_time_field(self):
        fields = ['S'] + [str(n) for n in range(4, 53)]
        self.assertEqual(MODULE.proc_identity('123 (worker (a) b) ' + ' '.join(fields)), 22)

    def test_every_generated_file_has_distinct_real_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'files'
            expected = MODULE.generate(root, 4, 4096, False)
            self.assertEqual(len(set(expected.values())), 4)
            self.assertEqual(sum(p.stat().st_size for p in root.rglob('*.bin')), 16_384)

    def test_pointer_size_and_digest_are_checked_independently(self):
        expected = {'000/000000.bin': 'a' * 64}
        for size, digest, good in [(500_000, 'a' * 64, True),
                                   (499_999, 'a' * 64, False),
                                   (500_000, 'b' * 64, False)]:
            value = f'/annex/objects/SHA256E-s{size}--{digest}.bin\n'.encode()
            blob = b'1' * 40 + b' blob ' + str(len(value)).encode() + b'\n' + value + b'\n'
            with patch.object(MODULE, 'tree', return_value={'000/000000.bin': '1' * 40}), \
                    patch.object(MODULE, 'git', return_value=blob):
                if good:
                    MODULE.verify_tree(Path('/synthetic'), expected, 500_000)
                else:
                    with self.assertRaises(RuntimeError):
                        MODULE.verify_tree(Path('/synthetic'), expected, 500_000)

    def test_actual_objects_are_hashed_and_symlinks_are_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            data = b'actual bytes'
            digest = MODULE.hashlib.sha256(data).hexdigest()
            expected = {'file.bin': digest}
            directory = repo / 'annex/objects/a/b'
            directory.mkdir(parents=True)
            obj = directory / f'SHA256E-s{len(data)}--{digest}.bin'
            obj.write_bytes(data)
            self.assertEqual(MODULE.objects(repo, expected, len(data), verify=True), 1)
            obj.write_bytes(b'other bytes!')
            with self.assertRaises(RuntimeError):
                MODULE.objects(repo, expected, len(data), verify=True)
            obj.unlink()
            obj.symlink_to('/missing')
            with self.assertRaises(RuntimeError):
                MODULE.objects(repo, expected, len(data), verify=True)


if __name__ == '__main__':
    unittest.main()

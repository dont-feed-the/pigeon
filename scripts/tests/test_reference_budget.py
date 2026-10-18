"""Fail-closed measurement contracts; reduced fixtures never certify a reference budget."""
import importlib.machinery
import importlib.util
from pathlib import Path
import sys
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

    def test_compression_guard_uses_physical_extent_usage_in_bytes(self):
        output = ('Processed 1 file, 4 regular extents (4 refs), 0 inline.\n'
                  'Type       Perc     Disk Usage   Uncompressed Referenced\n'
                  'TOTAL        3%      16384        507904       500000\n'
                  'zstd         3%      16384        507904       500000\n')
        self.assertEqual(MODULE.physical_extent_usage(output), dict(
            disk_bytes=16384, uncompressed_bytes=507904, referenced_bytes=500000))
        for invalid in ['', 'TOTAL 3% 16K 496K 488K', 'TOTAL 3% 0 -1 500000']:
            with self.assertRaises(RuntimeError):
                MODULE.physical_extent_usage(invalid)

    def test_a_timed_out_command_does_not_expose_its_invitation_in_any_traceback(self):
        import subprocess
        import traceback
        secret = 'synthetic-sensitive-invitation'
        command = ['pigeon', 'join', secret]
        with patch.object(MODULE.subprocess, 'run', side_effect=subprocess.TimeoutExpired(command, 2)):
            try:
                MODULE.run(command, timeout=2)
            except RuntimeError as error:
                self.assertEqual(str(error), 'fixture command timed out: pigeon')
                self.assertNotIn(secret, ''.join(traceback.format_exception(error)))
            else:
                self.fail('unbounded or unreported timeout')

    def test_partial_progress_is_explicitly_unmeasured_and_atomically_replaced(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'report.json'
            report = dict(budget_verdict='not-measured', stage='generating', generated_files=1000)
            MODULE.checkpoint(path, report)
            self.assertEqual(MODULE.json.loads(path.read_text()), report)
            self.assertFalse(path.with_suffix('.pending').exists())

    def test_full_profiles_refuse_lab_daemons_and_incomplete_build_diagnostics(self):
        terminal = [{'type': 'Said', 'message': {'id': 'doctor-ok'}}]
        ordinary = terminal + [{'type': 'Progress', 'message': {
            'id': 'doctor-version', 'args': {'program': name}}} for name in ['git', 'git-annex']]
        lab = ordinary + [{'type': 'Progress', 'message': {'id': 'doctor-lab-build'}}]
        self.assertEqual(MODULE.daemon_build(ordinary, True), 'ordinary')
        self.assertEqual(MODULE.daemon_build(lab, False), 'lab')
        for replies in [[], terminal, lab]:
            with self.subTest(replies=replies), self.assertRaises(RuntimeError):
                MODULE.daemon_build(replies, True)

    def test_doctor_reads_progress_from_stderr_before_classifying_the_build(self):
        import subprocess
        result = subprocess.CompletedProcess(['pigeon'], 0,
            stdout=b'{"type":"Said","message":{"id":"doctor-ok"}}\n',
            stderr=b'{"type":"Progress","message":{"id":"doctor-lab-build"}}\n')
        with patch.object(MODULE.subprocess, 'run', return_value=result):
            output = MODULE.run(['pigeon', '--json', 'doctor'], include_stderr=True)
        self.assertIn(b'doctor-lab-build', output)
        self.assertIn(b'doctor-ok', output)

    def test_dirty_full_profiles_stop_before_starting_native_fixtures(self):
        with tempfile.TemporaryDirectory() as temp:
            report = Path(temp) / 'report.json'
            argv = ['reference-budget', '--bin', temp, '--root', temp + '/new', '--report', str(report)]
            with patch.object(sys, 'argv', argv), \
                    patch.object(MODULE, 'run', side_effect=[b'a' * 40, b' M Cargo.toml']), \
                    patch.object(MODULE, 'measure') as measure:
                with self.assertRaisesRegex(RuntimeError, 'clean source checkout'):
                    MODULE.main()
                measure.assert_not_called()
            result = MODULE.json.loads(report.read_text())
            self.assertTrue(result['dirty_worktree'])
            self.assertEqual(result['budget_verdict'], 'not-measured')

    def test_preparation_deadlines_only_shorten_the_existing_bound(self):
        self.assertEqual(MODULE.preparation_seconds('600'), 600)
        self.assertEqual(MODULE.preparation_seconds('5400'), 5400)
        for value in ['0', '-1', '5401']:
            with self.subTest(value=value), self.assertRaises(MODULE.argparse.ArgumentTypeError):
                MODULE.preparation_seconds(value)

    def test_cleanup_attempts_every_peer_without_replacing_the_primary_failure(self):
        from unittest.mock import Mock
        peers = [Mock(), Mock()]
        peers[0].stop.side_effect = RuntimeError('private command output')
        report = {}
        MODULE.stop_peers(peers, report, True)
        for peer in peers:
            peer.stop.assert_called_once()
        self.assertEqual(report, {'cleanup_failures': ['RuntimeError']})
        with self.assertRaisesRegex(RuntimeError, 'reference peer cleanup failed'):
            MODULE.stop_peers(peers, {}, False)

    def test_failure_events_are_bounded_and_never_retain_raw_private_output(self):
        secret = 'synthetic-sensitive-invitation'
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            logs = home / 'state/logs'
            logs.mkdir(parents=True)
            rows = [MODULE.json.dumps(dict(message='capture-failed', error='index.lock ' + secret,
                    added=secret, avg10='Some(12.5)', time='2026-10-18T06:00:00Z')) for _ in range(60)]
            rows += [MODULE.json.dumps(dict(message=secret)), '[]', '{malformed']
            (logs / 'pigeond.ndjson').write_text('\n'.join(rows))
            events = MODULE.diagnostic_events(home)
            self.assertEqual(len(events), 50)
            self.assertEqual(events[-1]['error_kind'], 'index-lock')
            self.assertEqual(events[-1]['io_pressure_avg10'], 12.5)
            self.assertNotIn(secret, MODULE.json.dumps(events))

    def test_failure_inventory_counts_without_claiming_hash_verification_or_live_processes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            peer = MODULE.Peer(root, root / 'bin', 'laptop', False)
            repo = root / 'repo'
            objects = repo / 'annex/objects'
            objects.mkdir(parents=True)
            (objects / 'SHA256E-fixture').write_bytes(b'generated fixture')
            (objects / 'SHA256E-link').symlink_to(objects / 'SHA256E-fixture')
            with patch.object(peer, 'sample', side_effect=RuntimeError('private state')), \
                    patch.object(MODULE, 'run', return_value=b'a\0b\0'):
                report = peer.diagnostics(repo)
            self.assertEqual(report['index_paths'], 2)
            self.assertEqual(report['unverified_object_files'], 1)
            self.assertEqual(report['resources_unavailable'], 'RuntimeError')
            self.assertNotIn('private state', MODULE.json.dumps(report))
            self.assertEqual(report['native_processes'], [])

    def test_process_name_parentheses_do_not_change_cpu_counters(self):
        fields = ['S'] + [str(n) for n in range(4, 53)]
        self.assertEqual(MODULE.proc_cpu_ticks('123 (worker (a) b) ' + ' '.join(fields)), 29)

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

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

    def test_larger_smokes_cross_metadata_bounds_without_changing_qualification(self):
        for count in [1, 32, 257, 1024, 4096]:
            self.assertEqual(MODULE.profile_dimensions(True, count), (count, 4096, 5))
            self.assertEqual(MODULE.verdict(self.samples(), self.scans(), count, 5, False)['budget_verdict'], 'not-measured')
        self.assertEqual(MODULE.profile_dimensions(False), (100_000, 500_000, 600))

    def test_smoke_dimensions_cannot_change_full_profiles_or_accept_invalid_counts(self):
        for count in [0, -1, 4097, True, 1.5]:
            with self.subTest(count=count), self.assertRaises(ValueError):
                MODULE.profile_dimensions(True, count)
        with self.assertRaises(ValueError):
            MODULE.profile_dimensions(False, 1024)

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

    def test_latest_phase_progress_survives_many_scans_within_the_same_private_envelope(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            logs = home / 'state/logs'
            logs.mkdir(parents=True)
            rows = [dict(message='capture-ingestion-started', total=100000, path='private-name'),
                    dict(message='capture-ingestion-progress', processed=1024, total=100000),
                    dict(message='capture-ingestion-progress', processed=2048, total=100000,
                         error='private-native-output', observed='private-value')]
            rows += [dict(message='candidate-scan-complete', entries=n) for n in range(100)]
            (logs / 'pigeond.ndjson').write_text('\n'.join(MODULE.json.dumps(row) for row in rows))
            events = MODULE.diagnostic_events(home)
            self.assertEqual(len(events), 50)
            phases = [event for event in events if event['message'].startswith('capture-ingestion')]
            self.assertEqual(len(phases), 2)
            self.assertEqual(phases[-1]['processed'], 2048)
            self.assertNotIn('private-', MODULE.json.dumps(events))
            self.assertEqual(events[-1]['entries'], 99)

    def test_preparation_timeline_reuses_isolated_samples_at_a_bounded_cadence(self):
        report = {}
        progress = dict(accepted=1, backup_accepted=0, objects=0)
        resources = dict(laptop=dict(cpu_us=10, rss=20), backup=dict(cpu_us=30, rss=40))
        MODULE.preparation_sample(report, 0, progress, resources)
        progress['accepted'] = 2
        resources['laptop']['rss'] = 99
        MODULE.preparation_sample(report, 29, progress, resources)
        MODULE.preparation_sample(report, -1, progress, resources)
        self.assertEqual(len(report['preparation_samples']), 1)
        self.assertEqual(report['preparation_samples'][0]['progress']['accepted'], 1)
        self.assertEqual(report['preparation_samples'][0]['resources']['laptop']['rss'], 20)
        MODULE.preparation_sample(report, 30, progress, resources)
        self.assertEqual(len(report['preparation_samples']), 2)
        for step in range(2, 300):
            MODULE.preparation_sample(report, step * 30, progress, resources)
        self.assertEqual(len(report['preparation_samples']), 256)
        self.assertEqual(report['preparation_samples'][-1]['elapsed_seconds'], 8970)
        self.assertGreater(report['preparation_samples'][0]['elapsed_seconds'], 0)

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


class DiagnosticPreparation(unittest.TestCase):
    def test_fast_preparation_cannot_certify_even_passing_cpu_and_backup_numbers(self):
        for verdict in ['passed', 'failed', 'not-measured']:
            report = dict(budget_verdict=verdict, backup_resources=dict(budget_verdict='passed'))
            MODULE.diagnostic_verdict(report)
            self.assertEqual(report['budget_verdict'], 'not-measured')
            self.assertEqual(report['diagnostic_budget_verdict'], verdict)
            self.assertEqual(report['backup_resources']['budget_verdict'], 'not-measured')
            self.assertEqual(report['backup_resources']['diagnostic_budget_verdict'], 'passed')

    def test_readback_must_confirm_one_gentle_setting_before_sampling(self):
        class Peer:
            def __init__(self, rows):
                self.rows, self.calls = rows, []
            def cli(self, *args):
                self.calls.append(args)
                return [dict(type='Settings', settings=self.rows)]
        correct = dict(key='resources.preset', value='gentle')
        for rows in [[], [dict(key='resources.preset', value='fast')],
                     [dict(key='other.preset', value='gentle')], [correct, correct]]:
            with self.subTest(rows=rows), self.assertRaises(RuntimeError):
                MODULE.restore_measurement_preset([Peer(rows)])
        peer = Peer([correct])
        with patch.object(MODULE.time, 'sleep'):
            MODULE.restore_measurement_preset([peer])
        self.assertEqual(peer.calls, [('settings', 'resources.preset', 'gentle'),
                                      ('settings', 'resources.preset')])

    def test_native_process_state_ignores_parentheses_in_comm_and_unknown_states(self):
        self.assertEqual(MODULE.proc_state('123 (git (worker)) T 1 2 3'), 'T')
        self.assertEqual(MODULE.proc_state('123 (worker) strange 1 2 3'), 'other')


class NativeOperationPrivacy(unittest.TestCase):
    def test_operation_labels_do_not_export_paths_keys_or_bearer_arguments(self):
        commands = [
            (b'/usr/bin/git\0-c\0annex.thin=false\0annex\0add\0--batch\0', 'annex/add'),
            (b'/usr/bin/git-annex\0whereis\0--batch\0', 'annex/whereis'),
            (b'git\0--git-dir=/private/repository\0hash-object\0secret-path\0', 'git/hash-object'),
            (b'git-annex-p2p-iroh\0synthetic-private-bearer\0', 'transport-helper'),
        ]
        for command, wanted in commands:
            with self.subTest(wanted=wanted):
                label = MODULE.native_operation(command)
                self.assertEqual(label, wanted)
                for private in ['private', 'secret', 'bearer', 'repository']:
                    self.assertNotIn(private, label)

    def test_config_values_and_directory_names_are_not_mistaken_for_commands(self):
        for command in [b'git\0-C\0hash-object\0unknown\0',
                        b'git\0-c\0update-index\0private-command\0',
                        b'/private/git-like\0hash-object\0',
                        b'git-annex\0private-subcommand\0']:
            with self.subTest(command=command):
                self.assertEqual(MODULE.native_operation(command), 'other')
        self.assertEqual(MODULE.native_operation(b'git\0-C\0private\0ls-files\0'), 'git/ls-files')

    def test_complete_front_tokens_survive_a_truncated_large_path_argument(self):
        prefix = b'git\0ls-files\0--stage\0--\0' + b'private-name' * 1000
        self.assertEqual(MODULE.native_operation(prefix[:4096]), 'git/ls-files')
        for incomplete in [b'', b'git', b'git\0', b'git\0hash-object', b'git\0-c\0',
                           b'git\0annex\0', b'git\0\xff\0']:
            self.assertEqual(MODULE.native_operation(incomplete), 'other')

    def test_option_scanning_stops_before_an_unbounded_command_prefix(self):
        prefix = b'git\0' + b'-C\0private\0' * 1000 + b'ls-files\0'
        self.assertEqual(MODULE.native_operation(prefix[:4096]), 'other')

    def test_unreadable_command_lines_keep_an_unknown_operation_and_reads_are_bounded(self):
        from unittest.mock import mock_open
        with patch.object(MODULE.Path, 'open', side_effect=PermissionError):
            self.assertEqual(MODULE.process_operation(123), 'other')
        reader = mock_open(read_data=b'git\0hash-object\0private\0')
        with patch.object(MODULE.Path, 'open', reader):
            self.assertEqual(MODULE.process_operation(123), 'git/hash-object')
        reader().read.assert_called_once_with(4096)

    @unittest.skipUnless(Path('/proc/self/cmdline').is_file(), 'Linux native process diagnostics')
    def test_live_git_batch_process_is_classified_without_exporting_its_arguments(self):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(['git', 'init', '-q', directory], check=True, capture_output=True)
            process = subprocess.Popen(['git', '-C', directory, 'cat-file', '--batch'],
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE)
            try:
                self.assertIsNone(process.poll())
                self.assertEqual(MODULE.process_operation(process.pid), 'git/cat-file')
            finally:
                process.communicate(timeout=5)
            self.assertEqual(process.returncode, 0)

"""Reject ambiguous identities, leaked invitations and falsely successful cleanup."""
import copy
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'check-upgrade-peers'
LOADER = importlib.machinery.SourceFileLoader('upgrade_peers', str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


class UpgradePeerContracts(unittest.TestCase):
    def roster(self):
        return dict(format=2, invitation='synthetic-private-bearer', devices={
            'device-a': dict(name='upgrade-laptop', role='laptop', person='alice',
                             annex_uuid='uuid-a'),
            'device-b': dict(name='upgrade-backup', role='custodian', person='alice',
                             annex_uuid='uuid-b')})

    def test_public_identity_snapshot_excludes_invitation_and_unrelated_fields(self):
        roster = self.roster()
        roster['devices']['device-a']['unrelated'] = 'another-private-value'
        snapshot = MODULE.identity_snapshot(roster, dict(laptop='uuid-a', backup='uuid-b'))
        self.assertEqual(snapshot['laptop'], dict(device='device-a', annex_uuid='uuid-a',
                                                 person='alice', role='laptop'))
        self.assertNotIn('private', json.dumps(snapshot))

    def test_missing_ambiguous_wrong_role_or_mismatched_identity_is_rejected(self):
        original = self.roster()
        variants = []
        for field, value in [('name', 'other'), ('role', 'laptop'), ('annex_uuid', 'wrong')]:
            roster = copy.deepcopy(original)
            roster['devices']['device-b'][field] = value
            variants.append(roster)
        duplicate = copy.deepcopy(original)
        duplicate['devices']['device-c'] = duplicate['devices']['device-b'].copy()
        variants.append(duplicate)
        retired = copy.deepcopy(original)
        retired['format'] = 1
        variants.append(retired)
        for roster in variants:
            with self.subTest(roster=roster), self.assertRaises(RuntimeError):
                MODULE.identity_snapshot(roster, dict(laptop='uuid-a', backup='uuid-b'))

    def test_two_peers_cannot_claim_the_same_annex_identity(self):
        roster = self.roster()
        roster['devices']['device-b']['annex_uuid'] = 'uuid-a'
        with self.assertRaisesRegex(RuntimeError, 'distinct'):
            MODULE.identity_snapshot(roster, dict(laptop='uuid-a', backup='uuid-a'))

    def test_checks_still_run_under_python_optimization(self):
        command = ('import runpy,sys; m=runpy.run_path(sys.argv[1]); '
                   'm["require"](False,"expected safety refusal")')
        result = subprocess.run([sys.executable, '-O', '-c', command, str(SCRIPT)],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('expected safety refusal', result.stderr)

    def test_existing_report_is_never_overwritten_or_used_to_start_peers(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / 'report.json'
            report.write_text('original evidence')
            argv = [str(SCRIPT), '--old-bin', directory, '--new-bin', directory,
                    '--root', directory + '/new', '--report', str(report), '--first', 'backup']
            with patch.object(sys, 'argv', argv), patch.object(MODULE, 'exercise') as exercise, \
                    patch('sys.stderr'):
                with self.assertRaises(SystemExit):
                    MODULE.main()
                exercise.assert_not_called()
            self.assertEqual(report.read_text(), 'original evidence')

    def test_a_cleanup_failure_cannot_leave_a_passing_verdict(self):
        def failing_cleanup(_args, report):
            report.update(stage='completed', verdict='two-peer-upgrade-passed')
            raise RuntimeError('synthetic cleanup failure')
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / 'report.json'
            argv = [str(SCRIPT), '--old-bin', directory, '--new-bin', directory,
                    '--root', directory + '/new', '--report', str(report), '--first', 'laptop']
            with patch.object(sys, 'argv', argv), \
                    patch.object(MODULE, 'exercise', side_effect=failing_cleanup):
                with self.assertRaises(SystemExit):
                    MODULE.main()
            result = json.loads(report.read_text())
            self.assertEqual((result['stage'], result['verdict']), ('failed', 'failed'))
            self.assertEqual(result['error'], 'RuntimeError')

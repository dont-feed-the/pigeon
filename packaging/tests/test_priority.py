"""Preset changes must change supervisor policy exactly once and keep data.

Manager stand-ins record commands; systemd-analyze parses the real units.
Runtime priority inheritance remains a separate hosted-platform check.
"""
import os
import plistlib
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SERVICE = Path(__file__).resolve().parents[1] / 'pigeon-service'


class Priority(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='pigeon-priority-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.home = self.root / 'home'
        self.home.mkdir()
        self.bin = self.root / 'commands'
        self.bin.mkdir()
        self.calls = self.root / 'calls'
        self.env = dict(os.environ, HOME=str(self.home), PATH=f"{self.bin}:{os.environ['PATH']}")
        for key in ('XDG_CONFIG_HOME', 'XDG_STATE_HOME'):
            self.env.pop(key, None)
        for name, body in {
            'uname': 'echo Linux',
            'systemctl': f'printf "%s\\n" "$*" >> "{self.calls}"',
            'launchctl': f'printf "%s\\n" "$*" >> "{self.calls}"; [ "$1" != print ]',
        }.items():
            self.command(name, body)
        self.prefix = self.home / 'installation with spaces'
        self.units = self.home / '.config/systemd/user'
        self.state = self.home / '.local/state/pigeon'
        self.state.mkdir(parents=True)
        self.hint = self.state / 'resource-priority'

    def command(self, name, body):
        path = self.bin / name
        path.write_text('#!/bin/sh\n' + body + '\n')
        path.chmod(0o755)

    def run_service(self, *args, ok=True):
        result = subprocess.run(['sh', str(SERVICE), '--prefix', str(self.prefix), *args],
                                env=self.env, text=True, capture_output=True, timeout=15)
        self.assertEqual(result.returncode == 0, ok, result.stdout + result.stderr)
        return result

    def test_linux_priority_change_uses_a_dropin_and_restarts_once(self):
        self.run_service()
        custom = self.units / 'pigeond.service.d/99-mine.conf'
        custom.write_text('[Service]\nEnvironment=MY_SETTING=kept\n')
        data = self.state / 'sentinel'
        data.write_bytes(b'identity and groups stay')
        self.calls.write_text('')
        self.hint.write_text('1 normal\n')
        self.run_service('--priority-only')
        policy = self.units / 'pigeond.service.d/40-pigeon-resources.conf'
        for setting in ('Nice=0', 'CPUWeight=100', 'IOWeight=100', 'IOSchedulingClass=best-effort'):
            self.assertIn(setting, policy.read_text())
        calls = self.calls.read_text()
        self.assertEqual(calls.count('try-restart pigeond.service'), 1)
        self.run_service('--priority-only')
        self.assertEqual(self.calls.read_text(), calls)
        self.hint.write_text('1 background\n')
        self.run_service('--priority-only')
        self.assertIn('Nice=10', policy.read_text())
        self.assertIn('IOSchedulingClass=idle', policy.read_text())
        self.assertTrue(custom.exists())
        self.assertEqual(data.read_bytes(), b'identity and groups stay')
        self.assertTrue((self.units / 'pigeon-priority.path').exists())

    def test_torn_unknown_or_oversized_hints_leave_current_policy_and_manager_alone(self):
        self.run_service()
        policy = self.units / 'pigeond.service.d/40-pigeon-resources.conf'
        before = policy.read_bytes()
        for bad in (b'1 nor', b'2 normal\n', b'1 normal\nanything', b'x' * 100000, b'1 normal\n\n'):
            self.hint.write_bytes(bad)
            self.calls.write_text('')
            self.run_service('--priority-only', ok=False)
            self.assertEqual(policy.read_bytes(), before)
            self.assertEqual(self.calls.read_text(), '')

    def test_priority_does_not_recreate_an_uninstalled_daemon(self):
        self.run_service()
        self.run_service('--stop')
        self.calls.write_text('')
        self.hint.write_text('1 normal\n')
        self.run_service('--priority-only')
        self.assertFalse((self.units / 'pigeond.service').exists())
        self.assertFalse((self.units / 'pigeon-priority.path').exists())
        self.assertEqual(self.calls.read_text(), '')

    def test_systemd_parses_real_priority_units_with_escaped_state_and_install_paths(self):
        if not shutil.which('systemd-analyze'):
            self.skipTest('systemd-analyze is not installed on this platform')
        self.env['XDG_STATE_HOME'] = str(self.home / 'state $value% with spaces')
        helper = self.prefix / 'bin/pigeon-service'
        helper.parent.mkdir(parents=True)
        shutil.copy2(SERVICE, helper)
        self.run_service()
        path = self.units / 'pigeon-priority.path'
        self.assertIn('state $value%% with spaces', path.read_text())
        result = subprocess.run(['systemd-analyze', 'verify', '--user', '--man=no',
                                 str(self.units / 'pigeon-priority.service'), str(path)],
                                capture_output=True, text=True, env=self.env, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_invalid_hint_never_blocks_uninstall_or_updates(self):
        self.run_service()
        self.hint.write_bytes(b'broken request')
        self.run_service('--update-only')
        self.run_service('--stop')
        self.assertFalse((self.units / 'pigeond.service').exists())
        self.assertFalse((self.units / 'pigeon-priority.path').exists())

    def test_macos_changes_process_type_and_does_not_restart_equal_choice(self):
        self.command('uname', 'echo Darwin')
        self.state = self.home / 'Library/Logs/pigeon'
        self.state.mkdir(parents=True)
        self.hint = self.state / 'resource-priority'
        self.run_service()
        plist = self.home / 'Library/LaunchAgents/org.pigeon.pigeond.plist'
        self.calls.write_text('')
        self.hint.write_text('1 normal\n')
        self.run_service('--priority-only')
        self.assertEqual(plistlib.loads(plist.read_bytes())['ProcessType'], 'Standard')
        calls = self.calls.read_text()
        self.assertEqual(calls.count('bootout gui/'), 1)
        self.run_service('--priority-only')
        self.assertEqual(self.calls.read_text(), calls)
        watcher = self.home / 'Library/LaunchAgents/org.pigeon.priority.plist'
        agent = plistlib.loads(watcher.read_bytes())
        self.assertEqual(agent['WatchPaths'], [str(self.hint)])
        self.assertEqual(agent['ProgramArguments'], [str(self.prefix / 'bin/pigeon-service'),
                                                     '--prefix', str(self.prefix), '--priority-only'])
        self.hint.write_text('1 background\n')
        self.run_service('--priority-only')
        self.assertEqual(plistlib.loads(plist.read_bytes())['ProcessType'], 'Background')


if __name__ == '__main__':
    unittest.main()

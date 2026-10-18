"""Owned-image cleanup contracts; command doubles do not provide native budget evidence."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'check-reference-budget'


class MountOwnership(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.commands = self.root / 'bin'
        self.commands.mkdir()
        self.state = self.root / 'state.json'
        for name, body in {
            'sudo': 'shift; exec "$@"',
            'df': 'printf "Avail\\n8000000000\\n"',
            'stat': 'echo cgroup2fs',
            'truncate': 'touch "$3"',
            'mkfs.btrfs': 'exit 0',
            'blkid': 'echo owned-uuid',
            'systemd-run': 'exit 0',
            'python3': '[ "$REFERENCE_CASE" != profile-fails ]',
        }.items():
            self.executable(name, '#!/bin/sh\n' + body + '\n')
        program = '''#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
name=Path(sys.argv[0]).name
p=Path(os.environ['REFERENCE_STATE'])
s=json.loads(p.read_text()) if p.exists() else {'mounted':False,'unmounted':False}
case=os.environ['REFERENCE_CASE']
if name=='mount':s['mounted']=True
elif name=='umount':
    if case=='busy':sys.exit(1)
    s['mounted']=False;s['unmounted']=True
elif name=='mountpoint':sys.exit(0 if s['mounted'] else 1)
elif name=='findmnt':print('changed-uuid' if case=='changed' and s.get('queried') else 'owned-uuid');s['queried']=True
p.write_text(json.dumps(s))
'''
        # The helper uses the real Python, not the deliberately failing
        # profile-command shim that is first on PATH.
        import sys
        program = program.replace('#!/usr/bin/env python3', '#!' + sys.executable)
        for name in ('mount', 'umount', 'mountpoint', 'findmnt'):
            self.executable(name, program)

    def executable(self, name, body):
        p = self.commands / name
        p.write_text(body)
        p.chmod(0o755)

    def case(self, name):
        env = dict(os.environ, RUNNER_TEMP=str(self.root), REFERENCE_CASE=name,
                   REFERENCE_STATE=str(self.state), PATH=str(self.commands) + ':' + os.environ['PATH'])
        result = subprocess.run(['bash', str(SCRIPT), str(self.root / 'report.json')],
                                capture_output=True, text=True, env=env, timeout=20)
        return result, json.loads(self.state.read_text())

    def test_owned_mount_is_unmounted_before_its_image_is_removed(self):
        result, state = self.case('success')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(state['unmounted'])
        self.assertFalse(list(self.root.glob('pigeon-reference.*')))

    def test_profile_failure_still_cleans_the_owned_mount(self):
        result, state = self.case('profile-fails')
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(state['unmounted'])
        self.assertFalse(list(self.root.glob('pigeon-reference.*')))

    def test_busy_or_changed_mount_keeps_the_image(self):
        for name in ('busy', 'changed'):
            with self.subTest(name=name):
                self.state.unlink(missing_ok=True)
                result, state = self.case(name)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(state['unmounted'])
                self.assertTrue(list(self.root.glob('pigeon-reference.*/tree.img')))


if __name__ == '__main__':
    unittest.main()

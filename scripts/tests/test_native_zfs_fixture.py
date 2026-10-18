"""Disposable-pool cleanup never destroys an unowned or unidentified pool.

Shims exercise shell control flow only. These are not native ZFS evidence.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'check-zfs'


@unittest.skipUnless(sys.platform.startswith('linux'), 'Linux fixture control flow')
class PoolOwnership(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.commands = self.root / 'bin'
        self.commands.mkdir()
        self.state = self.root / 'state.json'
        self.executable('sudo', '#!/bin/sh\nexec "$@"\n')
        self.executable('modprobe', '#!/bin/sh\nexit 0\n')
        self.executable('zfs', '#!/bin/sh\nexit 0\n')
        self.executable('modinfo', '#!/bin/sh\necho synthetic-module\n')
        self.executable('stat', '#!/bin/sh\necho zfs\n')
        self.executable('cargo', '#!/bin/sh\n[ "$PIGEON_FIXTURE_CASE" != cargo-fails ]\n')
        self.executable('zpool', '''#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
path=Path(os.environ['PIGEON_FIXTURE_STATE'])
state=json.loads(path.read_text()) if path.exists() else {'created':False,'destroyed':False,'queries':0}
case=os.environ['PIGEON_FIXTURE_CASE'];args=sys.argv[1:];command=args[0]
if command=='list':sys.exit(0 if case=='existing' else 1)
if command=='create':
    state['created']=True
    volume=Path(next(a.split('=',1)[1] for a in args if a.startswith('mountpoint=')))
    (volume/'.git').mkdir();(volume/'.git/config').write_text('synthetic fixture metadata')
elif command=='get':
    state['queries']+=1
    path.write_text(json.dumps(state))
    if not state['created'] and case!='existing':sys.exit(1)
    if case=='unknown-guid':sys.exit(1)
    print('999' if case=='changed-guid' and state['queries']>1 else '777')
elif command=='destroy':state['destroyed']=True
else:sys.exit(2)
path.write_text(json.dumps(state))
''')

    def executable(self, name, body):
        path = self.commands / name
        path.write_text(body)
        path.chmod(0o755)

    def run_case(self, case):
        env = dict(os.environ, RUNNER_TEMP=str(self.root),
                   PATH=str(self.commands) + os.pathsep + os.environ['PATH'],
                   PIGEON_FIXTURE_CASE=case, PIGEON_FIXTURE_STATE=str(self.state))
        result = subprocess.run(['bash', str(SCRIPT)], env=env, text=True,
                                capture_output=True, timeout=20)
        state = json.loads(self.state.read_text()) if self.state.exists() else {}
        return result, state

    def test_a_matching_owned_pool_is_destroyed_and_its_private_image_removed(self):
        result, state = self.run_case('success')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(state['created'])
        self.assertTrue(state['destroyed'])
        self.assertFalse(list(self.root.glob('pigeon-zfs.*')))

    def test_a_preexisting_pool_is_never_destroyed(self):
        result, state = self.run_case('existing')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(state['created'])
        self.assertFalse(state['destroyed'])

    def test_changed_or_unrecorded_identity_retains_the_image_and_pool(self):
        for case in ['changed-guid', 'unknown-guid']:
            with self.subTest(case=case):
                self.state.unlink(missing_ok=True)
                previous = set(self.root.glob('pigeon-zfs.*/vdev.img'))
                result, state = self.run_case(case)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(state['created'])
                self.assertFalse(state['destroyed'])
                self.assertEqual(len(set(self.root.glob('pigeon-zfs.*/vdev.img')) - previous), 1)

    def test_a_failed_native_check_retains_hidden_metadata_before_destroy(self):
        result, state = self.run_case('cargo-fails')
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(state['destroyed'])
        retained = list(self.root.glob('pigeon-zfs-failure-*/volume/.git/config'))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0].read_text(), 'synthetic fixture metadata')


if __name__ == '__main__':
    unittest.main()

"""Frozen-program integrity and real Git/object proofs for upgrade evidence."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest

UPGRADE = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'check-upgrade'))


class FrozenPrograms(unittest.TestCase):
    def test_atomic_fixture_saves_have_explicit_permissions_under_private_umask(self):
        with tempfile.TemporaryDirectory() as directory:
            previous = os.umask(0o077)
            try:
                UPGRADE['atomic_save'](Path(directory), b'fixture')
            finally:
                os.umask(previous)
            self.assertEqual((Path(directory) / 'notes.txt').stat().st_mode & 0o777, 0o644)

    def test_changed_program_cannot_keep_its_source_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = dict(source_revision='a' * 40, features=['pigeond/lab'], sha256={})
            for name in UPGRADE['PROGRAMS']:
                (root / name).write_bytes(name.encode())
                manifest['sha256'][name] = hashlib.sha256(name.encode()).hexdigest()
            (root / 'soak-build.json').write_text(json.dumps(manifest))
            self.assertEqual(UPGRADE['frozen'](root), manifest)
            (root / 'pigeond').write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'differs from frozen manifest'):
                UPGRADE['frozen'](root)


class HistoricalObjects(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name) / 'repo'
        subprocess.run(['git', 'init', '-q', '--bare', '-b', 'main', str(self.repo)], check=True)
        self.current, self.history, self.files = {}, set(), {}
        entries = []
        for name, data in [('notes.txt', b'new notes'), ('photo.bin', bytes(range(256)))]:
            key = self.add_object(data)
            self.current[name] = UPGRADE['fingerprint'](data)
            pointer = self.git('hash-object', '-w', '--stdin', input=('/annex/objects/' + key + '\n').encode()).strip()
            entries.append(b'100644 blob ' + pointer + b'\t' + name.encode() + b'\n')
        self.add_object(b'old notes')
        tree = self.git('mktree', input=b''.join(entries)).strip().decode()
        commit = self.git('-c', 'user.name=fixture', '-c', 'user.email=fixture@example.test',
                          'commit-tree', tree, input=b'fixture\n').strip().decode()
        self.git('update-ref', 'refs/heads/main', commit)

    def git(self, *args, **kwargs):
        return subprocess.check_output(['git', '--git-dir', str(self.repo), *args], **kwargs)

    def add_object(self, data):
        size, digest = UPGRADE['fingerprint'](data)
        self.history.add((size, digest))
        key = f'SHA256E-s{size}--{digest}'
        path = self.repo / 'annex/objects/bucket' / key / key
        path.parent.mkdir(parents=True)
        path.write_bytes(data)
        self.files[digest] = path
        return key

    def test_current_and_retained_objects_of_different_sizes_are_checked(self):
        UPGRADE['prove'](self.repo, self.current, self.history)

    def test_pointer_bytes_cannot_hide_a_different_executable_mode(self):
        with self.assertRaisesRegex(RuntimeError, 'executable modes differ'):
            UPGRADE['prove'](self.repo, self.current, self.history,
                             {'notes.txt': 0o644, 'photo.bin': 0o755})

    def test_same_size_corruption_in_an_old_version_fails(self):
        path = self.files[hashlib.sha256(b'old notes').hexdigest()]
        path.write_bytes(b'x' * len(b'old notes'))
        with self.assertRaisesRegex(RuntimeError, 'hash mismatch'):
            UPGRADE['prove'](self.repo, self.current, self.history)

    def test_missing_retained_object_cannot_pass_with_correct_current_files(self):
        self.files[hashlib.sha256(b'old notes').hexdigest()].unlink()
        with self.assertRaisesRegex(RuntimeError, 'historical object missing'):
            UPGRADE['prove'](self.repo, self.current, self.history)

"""Published snapshots reject metadata drift, unsafe payloads and implicit lab conversion."""
import copy
import base64
import hashlib
import io
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
ARCHIVE = runpy.run_path(str(SCRIPTS / 'prepare-upgrade-archive'))
UPGRADE = runpy.run_path(str(SCRIPTS / 'check-upgrade'))


class PublishedArchiveContracts(unittest.TestCase):
    def test_native_pin_is_bounded_unique_and_bound_to_its_source_blob(self):
        body = b'PIGEON_VERSION="10.20261006-pigeon2"\n'
        def metadata(body):
            return dict(encoding='base64', size=len(body),
                        content=base64.b64encode(body).decode(),
                        sha=hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest())
        valid = metadata(body)
        def read(item):
            with patch.dict(ARCHIVE['annex_pin'].__globals__, api=lambda _path: item):
                return ARCHIVE['annex_pin']('a' * 40)
        self.assertEqual(read(valid), '10.20261006-pigeon2')
        for invalid in [dict(valid, sha='0' * 40), dict(valid, size=len(body) + 1),
                        dict(valid, size=16385), dict(valid, encoding='raw'),
                        dict(valid, content='a' * 32769), dict(valid, content='!'),
                        metadata(body + body), metadata(b'OTHER_VERSION=10.0\n'),
                        metadata(b'PIGEON_VERSION=10.0;exit\n')]:
            with self.subTest(metadata=invalid), self.assertRaises(ValueError):
                read(invalid)
        with patch.dict(ARCHIVE['annex_pin'].__globals__, api=lambda _: self.fail('network called')):
            with self.assertRaises(ValueError):
                ARCHIVE['annex_pin']('main')

    def test_each_release_requires_its_own_pinned_native_version(self):
        for expected, actual, accepted in [
            ('10.20261006-pigeon1', '10.20261006-pigeon1', True),
            ('10.20261006-pigeon2', '10.20261006-pigeon2', True),
            ('10.20261006-pigeon2', '10.20261006-pigeon1', False),
            ('10.20261006-pigeon1', '9.0-pigeon1', False),
        ]:
            with self.subTest(expected=expected, actual=actual), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                tag, source = 'v0.6.51', 'a' * 40
                package = f'pigeon-{tag}-linux-x86_64.tar.gz'
                native = 'git-annex-pigeon-linux-x86_64.tar.gz'
                for name, programs in [(package, ARCHIVE['PROGRAMS']), (native, ['git-annex'])]:
                    with tarfile.open(root / name, 'w:gz') as stream:
                        for program in programs:
                            value = actual if program == 'git-annex' else 'pigeon ' + tag[1:]
                            body = ('#!/usr/bin/env python3\nprint(' + repr(value) + ')\n').encode()
                            entry = tarfile.TarInfo('bin/' + program)
                            entry.mode, entry.size = 0o500, len(body)
                            stream.addfile(entry, io.BytesIO(body))
                (root / 'SHA256SUMS').write_text(''.join(
                    ARCHIVE['sha'](root / name) + '  ' + name + '\n' for name in [package, native]))
                assets = [dict(name=name, size=(root / name).stat().st_size,
                               digest='sha256:' + ARCHIVE['sha'](root / name))
                          for name in [package, native, 'SHA256SUMS']]
                pin = ('PIGEON_VERSION=' + expected + '\n').encode()
                metadata = dict(encoding='base64', size=len(pin),
                                content=base64.b64encode(pin).decode(),
                                sha=hashlib.sha1(b'blob ' + str(len(pin)).encode() + b'\0' + pin).hexdigest())
                def api(path):
                    replies = {f'releases/tags/{tag}': dict(tag_name=tag, draft=False,
                               published_at='2026-10-18T00:00:00Z', assets=assets),
                               f'git/ref/tags/{tag}': dict(object=dict(type='commit', sha=source)),
                               f'contents/third_party/git-annex/VERSION?ref={source}': metadata}
                    self.assertIn(path, replies)
                    return replies[path]
                real_run = subprocess.run
                def run(args, **kwargs):
                    if args[:3] == ['gh', 'release', 'download']:
                        destination = Path(args[args.index('--dir') + 1])
                        for asset in assets:
                            shutil.copyfile(root / asset['name'], destination / asset['name'])
                        return subprocess.CompletedProcess(args, 0, '', '')
                    return real_run(args, **kwargs)
                with patch.dict(ARCHIVE['prepare'].__globals__, api=api), patch('subprocess.run', side_effect=run):
                    if accepted:
                        result = ARCHIVE['prepare'](tag, root / 'snapshot')
                        manifest = json.loads((Path(result['binary']) / 'soak-build.json').read_text())
                        self.assertEqual(manifest['annex']['version'], expected)
                        self.assertEqual(result['source'], source)
                    else:
                        with self.assertRaisesRegex(ValueError, 'annex version differs'):
                            ARCHIVE['prepare'](tag, root / 'snapshot')

    def test_extra_packaging_assets_are_allowed_but_missing_or_duplicate_payloads_are_not(self):
        release = dict(draft=False, published_at='2026-10-16T21:09:26Z',
                       assets=[dict(name=n) for n in ['linux.tar.gz', 'SHA256SUMS', 'pigeon.rb']])
        self.assertEqual(set(ARCHIVE['checked_assets'](release, ['linux.tar.gz', 'SHA256SUMS'])),
                         {'linux.tar.gz', 'SHA256SUMS'})
        for change in [dict(draft=True), dict(published_at=None),
                       dict(assets=[dict(name='SHA256SUMS')]),
                       dict(assets=release['assets'] + [dict(name='linux.tar.gz')])]:
            invalid = dict(release, **change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                ARCHIVE['checked_assets'](invalid, ['linux.tar.gz', 'SHA256SUMS'])

    def test_coherent_payload_and_checksum_tampering_still_fails_asset_digest_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'linux.tar.gz').write_bytes(b'original')
            sums = root / 'SHA256SUMS'
            sums.write_text(hashlib.sha256(b'original').hexdigest() + '  linux.tar.gz\n')
            assets = {p.name: dict(size=p.stat().st_size,
                                   digest='sha256:' + ARCHIVE['sha'](p)) for p in root.iterdir()}
            ARCHIVE['verify_download'](root, assets)
            (root / 'linux.tar.gz').write_bytes(b'changed!')
            sums.write_text(hashlib.sha256(b'changed!').hexdigest() + '  linux.tar.gz\n')
            with self.assertRaises(ValueError):
                ARCHIVE['verify_download'](root, assets)

    def test_checksum_duplicates_and_paths_outside_the_download_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'sums'
            digest = 'a' * 64
            for content in [digest + '  ../outside\n',
                            digest + '  same\n' + digest + '  same\n',
                            'not-a-digest  file\n']:
                path.write_text(content)
                with self.subTest(content=content), self.assertRaises(ValueError):
                    ARCHIVE['checksums'](path)

    def test_program_extraction_never_follows_an_archived_hard_link(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'payload.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                for name in ARCHIVE['PROGRAMS']:
                    entry = tarfile.TarInfo('bin/' + name)
                    entry.mode = 0o755
                    entry.size = 1
                    if name == 'pigeond':
                        entry.type = tarfile.LNKTYPE
                        entry.linkname = '../../outside'
                        entry.size = 0
                    stream.addfile(entry, io.BytesIO(b'x') if entry.size else None)
            with self.assertRaises(ValueError):
                ARCHIVE['extract_programs'](archive, root / 'bin')
            self.assertFalse((root / 'outside').exists())
            self.assertFalse((root / 'bin/pigeond').exists())

    def test_annex_links_cannot_escape_the_verified_prefix(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'annex.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                entry = tarfile.TarInfo('bin/git-annex')
                entry.type = tarfile.SYMTYPE
                entry.linkname = '../../outside'
                stream.addfile(entry)
            with self.assertRaises(tarfile.FilterError):
                ARCHIVE['extract_annex'](archive, root / 'annex')
            self.assertFalse((root / 'outside').exists())

    def test_existing_snapshot_stops_before_metadata_or_network_access(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(ARCHIVE['prepare'].__globals__, api=lambda _path: self.fail('network called')):
                with self.assertRaises(ValueError):
                    ARCHIVE['prepare']('v0.5.13', Path(directory))

    def test_published_origin_requires_explicit_mode_and_annex_cannot_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            binary = prefix / 'bin'
            binary.mkdir()
            annex = prefix / 'git-annex/bin/git-annex'
            annex.parent.mkdir(parents=True)
            annex.write_bytes(b'annex')
            manifest = dict(source_revision='a' * 40, features=[], sha256={},
                            origin=dict(kind='published-release-archives', repository='dont-feed-the/pigeon',
                                        tag='v0.5.13', published_at='2026-10-16T21:09:26Z',
                                        package_sha256='b' * 64, annex_archive_sha256='c' * 64,
                                        checksums_sha256='d' * 64),
                            annex=dict(sha256=ARCHIVE['sha'](annex)))
            for name in UPGRADE['PROGRAMS']:
                (binary / name).write_bytes(name.encode())
                manifest['sha256'][name] = ARCHIVE['sha'](binary / name)
            (binary / 'soak-build.json').write_text(json.dumps(manifest))
            with self.assertRaises(ValueError):
                UPGRADE['frozen'](binary)
            self.assertEqual(UPGRADE['frozen'](binary, True), manifest)
            annex.write_bytes(b'drift')
            with self.assertRaisesRegex(ValueError, 'annex differs'):
                UPGRADE['frozen'](binary, True)

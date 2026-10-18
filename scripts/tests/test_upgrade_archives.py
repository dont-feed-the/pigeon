"""Published snapshots reject metadata drift, unsafe payloads and implicit lab conversion."""
import copy
import hashlib
import io
import json
from pathlib import Path
import runpy
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
ARCHIVE = runpy.run_path(str(SCRIPTS / 'prepare-upgrade-archive'))
UPGRADE = runpy.run_path(str(SCRIPTS / 'check-upgrade'))


class PublishedArchiveContracts(unittest.TestCase):
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

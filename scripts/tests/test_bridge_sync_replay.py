"""Replay every raw DAV trace and reuse only identical normalized TLC inputs.

The solver boundary is recorded while the real parser, normalizer and input
writer run. Malformed raw streams and distinct constants/models still fail.
"""
import contextlib
import copy
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'check-bridge-sync-traces'
LOADER = importlib.machinery.SourceFileLoader('bridge_sync_replay', str(SCRIPT))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)


def missing(path='missing.txt', label='synthetic'):
    return [dict(spec='BridgeSync', clock=i + 1, action=action, vars=values)
            for i, (action, values) in enumerate([
                ('Begin', dict(client=label, device=f'device-{label}', method='GET', kind='files', path=path)),
                ('View', dict(required=None, status=0, revision=f'revision-{label}', files=[])),
                ('Response', dict(status=404, etag=None)),
            ])]


class ReplayInputs(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        models = self.root / 'models/tla'
        models.mkdir(parents=True)
        for name in ('BridgeSync', 'BridgeSyncTrace'):
            (models / f'{name}.tla').write_bytes((MODULE.ROOT / 'models/tla' / f'{name}.tla').read_bytes())
        self.traces = self.root / 'traces'
        self.traces.mkdir()
        self.root_patch = patch.object(MODULE, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.jar_patch = patch.object(MODULE.tlc, 'tlc_jar', return_value=self.root / 'unused.jar')
        self.jar_patch.start()
        self.addCleanup(self.jar_patch.stop)

    def write(self, name, rows):
        (self.traces / f'sync-{name}.ndjson').write_text(''.join(json.dumps(r) + '\n' for r in rows))

    def replay(self):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            MODULE.replay(self.traces, 'java')

    def test_a_thousand_equivalent_streams_need_one_identical_solver_input(self):
        for i in range(1000):
            self.write(f'{i:04}', missing(f'file-{i}.txt', f'client-{i}'))
        with patch.object(MODULE.tlc, 'run', return_value=('trace:accepted', '')) as solver:
            self.replay()
        self.assertEqual(solver.call_count, 1)

    def test_method_and_resource_constants_cannot_reuse_another_verdict(self):
        for field, value in [('method', 'HEAD'), ('kind', 'final')]:
            with self.subTest(field=field):
                self.write('a', missing())
                changed = missing()
                changed[0]['vars'][field] = value
                self.write('z', changed)
                with patch.object(MODULE.tlc, 'run', side_effect=[('trace:accepted', ''), ('trace:rejected:x', '')]) as solver:
                    with self.assertRaises(MODULE.Refused):
                        self.replay()
                self.assertEqual(solver.call_count, 2)

    def test_later_malformed_streams_are_refused_even_after_an_equivalent_success(self):
        original = missing()
        mutations = []
        changed = copy.deepcopy(original)
        changed[-1]['clock'] += 1
        mutations.append(changed)
        changed = copy.deepcopy(original)
        changed[0]['vars']['future'] = True
        mutations.append(changed)
        mutations.append(original[:-1])
        for rows in mutations:
            with self.subTest(rows=rows):
                self.write('a', original)
                self.write('z', rows)
                with patch.object(MODULE.tlc, 'run', return_value=('trace:accepted', '')):
                    with self.assertRaises(MODULE.Refused):
                        self.replay()

    def test_a_changed_response_must_receive_its_own_solver_verdict(self):
        self.write('a', missing())
        changed = missing()
        changed[-1]['vars']['status'] = 200
        self.write('z', changed)
        with patch.object(MODULE.tlc, 'run', side_effect=[('trace:accepted', ''), ('trace:rejected:x', '')]) as solver:
            with self.assertRaises(MODULE.Refused):
                self.replay()
        self.assertEqual(solver.call_count, 2)

    def test_a_model_source_change_invalidates_an_otherwise_identical_input(self):
        self.write('a', missing())
        self.write('z', missing())
        model = self.root / 'models/tla/BridgeSyncTrace.tla'
        def solve(*args, **kwargs):
            if solve.calls == 0:
                model.write_bytes(model.read_bytes() + b'\n\\* changed source\n')
            solve.calls += 1
            return 'trace:accepted', ''
        solve.calls = 0
        with patch.object(MODULE.tlc, 'run', side_effect=solve):
            self.replay()
        self.assertEqual(solve.calls, 2)


if __name__ == '__main__':
    unittest.main()

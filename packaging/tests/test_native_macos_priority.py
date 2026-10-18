"""Exercise actual launchd only on a disposable hosted Mac CI worker.

Ordinary local packaging tests use manager stand-ins and never start or stop
the current user's service. The native fixture refuses pre-existing units.
"""
import os
from pathlib import Path
import subprocess
import unittest


@unittest.skipUnless(os.environ.get('GITHUB_ACTIONS') == 'true' and os.uname().sysname == 'Darwin',
                     'requires a disposable hosted Mac with launchd')
class NativeMacPriority(unittest.TestCase):
    def test_actual_launchd_changes_priority_and_children_inherit_it(self):
        subprocess.run([str(Path(__file__).resolve().parents[1] / 'check-macos-priority')],
                       check=True, timeout=120)


if __name__ == '__main__':
    unittest.main()

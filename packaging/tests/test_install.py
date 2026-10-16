"""Exercise release installation, updates, rollback and data-preserving removal.

Archives use tiny stand-in executables; real binaries and patched git-annex
are installed separately by the release workflow before publication.
"""

import hashlib
import os
import plistlib
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

PACKAGING = Path(__file__).resolve().parents[1]


class Installation(unittest.TestCase):
    """A complete installation lifecycle in an isolated person's directories."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="pigeon-install-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.prefix = self.home / "pigeon with spaces"
        self.releases = self.root / "releases"
        self.releases.mkdir()
        self.env = dict(os.environ, HOME=str(self.home), PIGEON_RELEASES=str(self.releases))
        for key in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME"):
            self.env.pop(key, None)
        self.platform = "macos-arm64" if os.uname().sysname == "Darwin" else "linux-x86_64"
        # Exercise both supervisors without starting a service on the test host.
        self.commands = self.root / "commands"
        self.commands.mkdir()
        self.executable(self.commands / "systemctl", "exit 0")
        self.executable(self.commands / "launchctl", '[ "$1" != print ] || exit 113')
        self.env["PATH"] = f"{self.commands}:{os.environ['PATH']}"
        self.release("v0.3.2")
        self.release("v0.3.3")

    @staticmethod
    def executable(path, body):
        path.write_text(f"#!/bin/sh\n{body}\n")
        path.chmod(0o755)

    def release(self, tag):
        dest = self.releases / "download" / tag
        dest.mkdir(parents=True)
        stage = self.root / tag
        stage.mkdir()
        binaries = stage / "bin"
        binaries.mkdir()
        self.executable(binaries / "pigeon", f"echo 'pigeon {tag[1:]}'")
        for name in ("pigeond", "git-annex-p2p-iroh"):
            self.executable(binaries / name, "exit 0")
        for name in ("pigeon-install", "install-git-annex", "pigeon-service"):
            shutil.copy2(PACKAGING / name, binaries / name)
        archive = dest / f"pigeon-{tag}-{self.platform}.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(binaries, arcname="bin")
        annex_stage = self.root / f"annex-{tag}"
        (annex_stage / "bin").mkdir(parents=True)
        self.executable(annex_stage / "bin/git-annex", "echo 10.20261006-pigeon1")
        (annex_stage / "bin/git-remote-p2p-annex").symlink_to("git-annex")
        annex = dest / f"git-annex-pigeon-{self.platform}.tar.gz"
        with tarfile.open(annex, "w:gz") as tar:
            tar.add(annex_stage / "bin", arcname="bin")
        (dest / "SHA256SUMS").write_text("".join(
            f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n"
            for p in (archive, annex)
        ))
        (self.releases / "latest").write_text(tag)

    def run_installer(self, *args, ok=True):
        result = subprocess.run(
            ["sh", str(PACKAGING / "pigeon-install"), "--prefix", str(self.prefix), *args],
            env=self.env, capture_output=True, text=True, timeout=30,
        )
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def version(self):
        return subprocess.check_output([self.prefix / "bin/pigeon", "--version"], text=True).strip()

    def test_upgrade_rollback_and_uninstall_keep_data(self):
        (self.prefix / "data").mkdir(parents=True)
        sentinel = self.prefix / "data/device-key"
        sentinel.write_text("keep this identity and group data")
        self.run_installer("--version", "v0.3.2")
        self.assertEqual(self.version(), "pigeon 0.3.2")
        self.assertIn("already installed", self.run_installer("--version", "v0.3.2").stdout)
        self.run_installer()
        self.assertEqual(self.version(), "pigeon 0.3.3")
        self.run_installer("--rollback")
        self.assertEqual(self.version(), "pigeon 0.3.2")
        self.run_installer("--uninstall")
        self.assertFalse((self.prefix / "bin").exists())
        self.assertFalse((self.home / ".local/bin/pigeon").is_symlink())
        self.assertEqual(sentinel.read_text(), "keep this identity and group data")

    def test_bad_checksum_leaves_current_installation_untouched(self):
        self.run_installer("--version", "v0.3.2", "--no-service")
        archive = next((self.releases / "download/v0.3.3").glob("pigeon-*.tar.gz"))
        archive.write_bytes(b"broken download")
        self.run_installer("--version", "v0.3.3", "--no-service", ok=False)
        self.assertEqual(self.version(), "pigeon 0.3.2")
        self.assertFalse(list(self.prefix.glob(".pigeon-install.*")))

    def test_missing_rollback_and_unknown_uninstall_do_not_remove_data(self):
        self.prefix.mkdir()
        sentinel = self.prefix / "keep-me"
        sentinel.write_text("still here")
        self.run_installer("--rollback", ok=False)
        self.run_installer("--uninstall", ok=False)
        self.assertEqual(sentinel.read_text(), "still here")

    def test_main_build_carries_the_service_helper_into_the_installation(self):
        source = self.prefix / "src"
        (source / ".git").mkdir(parents=True)
        (source / "packaging").mkdir()
        (source / "Cargo.toml").write_text('[workspace.package]\nversion = "0.3.4"\n')
        for name in ("pigeon-install", "pigeon-service"):
            shutil.copy2(PACKAGING / name, source / "packaging" / name)
        self.executable(source / "packaging/install-git-annex", "exit 0")
        target = source / "target/release"
        target.mkdir(parents=True)
        self.executable(target / "pigeon", "echo pigeon 0.3.4+gabcdef123456")
        for name in ("pigeond", "git-annex-p2p-iroh"):
            self.executable(target / name, "exit 0")
        self.executable(self.commands / "git", 'case "$*" in *rev-parse*) echo abcdef123456 ;; esac')
        self.executable(self.commands / "cargo", "exit 0")
        self.env.pop("CARGO_TARGET_DIR", None)
        self.run_installer("--main")
        self.assertEqual(self.version(), "pigeon 0.3.4+gabcdef123456")
        self.assertTrue((self.prefix / "bin/pigeon-service").is_file())

    def test_conflicting_modes_leave_the_installation_untouched(self):
        self.run_installer("--version", "v0.3.2", "--no-service")
        self.run_installer("--main", "--uninstall", ok=False)
        self.run_installer("--rollback", "--version", "v0.3.3", ok=False)
        self.assertEqual(self.version(), "pigeon 0.3.2")

    def test_supervisor_configuration_escapes_paths(self):
        for os_name in ("Linux", "Darwin"):
            with self.subTest(os=os_name):
                self.executable(self.commands / "uname", f"echo {os_name}")
                prefix = '/a path/with & <quotes> " and $dollar%'
                result = subprocess.check_output(
                    ["sh", str(PACKAGING / "pigeon-service"), "--prefix", prefix, "--print"],
                    env=self.env, text=True,
                )
                if os_name == "Darwin":
                    agent = plistlib.loads(result.encode())
                    self.assertEqual(agent["ProgramArguments"], [prefix + "/bin/pigeond"])
                else:
                    self.assertIn("MemoryHigh=512M", result)
                    self.assertIn("$$dollar%%", result)
                    self.assertIn('\\"', result)

    def test_system_unit_uses_the_service_account_and_system_directories(self):
        self.executable(self.commands / "uname", "echo Linux")
        result = subprocess.check_output(
            ["sh", str(PACKAGING / "pigeon-service"), "--system", "--prefix", "/opt/pigeon", "--print"],
            env=self.env, text=True,
        )
        for line in ("User=pigeon", "Group=pigeon", 'ExecStart="/opt/pigeon/bin/pigeond" --service',
                     "StateDirectory=pigeon", "RuntimeDirectory=pigeon", "WantedBy=multi-user.target"):
            self.assertIn(line, result)
        self.assertNotIn("Nice=", result)

    def launch_agent(self, launchctl):
        """Install the macOS agent against a launchctl stand-in; time passes instantly."""
        self.executable(self.commands / "uname", "echo Darwin")
        self.executable(self.commands / "sleep", "exit 0")
        self.executable(self.commands / "launchctl", launchctl)
        return subprocess.run(
            ["sh", str(PACKAGING / "pigeon-service"), "--prefix", "/opt/pigeon"],
            env=self.env, capture_output=True, text=True, timeout=30,
        )

    def test_agent_waits_until_launchd_has_unloaded_the_old_job(self):
        state = self.root / "launchd"
        state.mkdir()
        # launchd keeps the job for three more polls after bootout returns, and
        # refuses to bootstrap it until then, as on a real Mac.
        result = self.launch_agent(f"""
            cd "{state}"
            case $1 in
              bootout) echo 3 > teardown ;;
              print) n=$(cat teardown); [ "$n" -gt 0 ] || exit 113; echo $((n - 1)) > teardown ;;
              bootstrap) [ "$(cat teardown)" = 0 ] || {{ echo "Bootstrap failed: 5: Input/output error" >&2; exit 5; }}
                         touch bootstrapped ;;
            esac""")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((state / "bootstrapped").exists())
        agent = plistlib.loads((self.home / "Library/LaunchAgents/org.pigeon.pigeond.plist").read_bytes())
        self.assertEqual(agent["ProgramArguments"], ["/opt/pigeon/bin/pigeond"])

    def test_agent_gives_up_when_launchd_never_unloads_the_old_job(self):
        bootstrapped = self.root / "bootstrapped"
        result = self.launch_agent(f'[ "$1" != bootstrap ] || touch "{bootstrapped}"')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("still runs pigeond ten seconds after stopping it", result.stderr)
        self.assertFalse(bootstrapped.exists())


if __name__ == "__main__":
    unittest.main()

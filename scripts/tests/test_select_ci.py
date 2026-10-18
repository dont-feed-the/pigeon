"""CI dependency selection over real complete diffs and adversarial metadata.

Release events remain full; skipped or missing selected checks cannot pass.
Version-only bumps must never hide a dependency or workspace-graph change.
"""

import copy
import importlib.machinery
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "select-ci"
LOADER = importlib.machinery.SourceFileLoader("select_ci", str(SCRIPT))
MODULE = importlib.util.module_from_spec(importlib.util.spec_from_loader(LOADER.name, LOADER))
LOADER.exec_module(MODULE)


class DependencyGroups(unittest.TestCase):
    def groups(self, *paths):
        return MODULE.classify(paths)[0]

    def test_documents_do_not_start_rust_models_or_nextcloud(self):
        self.assertEqual(self.groups("README.md", "CHANGELOG.md", "docs/spec/06-status.md"), set())

    def test_standalone_evidence_tools_keep_repository_contracts_without_runtime_jobs(self):
        for path in MODULE.EVIDENCE_TOOLS:
            self.assertEqual(self.groups(path), set(), path)
            self.assertEqual(self.groups(path, "crates/pigeond/src/capture.rs"), MODULE.RUNTIME)

    def test_evidence_tool_names_do_not_exempt_unknown_neighbours_or_ci_policy(self):
        for path in ["scripts/check-upgrade-extra", "scripts/prepare-upgrade-archive-extra",
                     "scripts/check-archive-upgrades-extra", "scripts/tests/test_upgrade_archives_extra.py",
                     "scripts/tests/test_reference_unknown.py",
                     "scripts/select-ci", ".github/workflows/lab.yml"]:
            self.assertEqual(self.groups("scripts/check-upgrade", path), MODULE.GROUPS, path)

    def test_packaging_and_bridge_have_distinct_dependents(self):
        self.assertEqual(self.groups("packaging/pigeon-service", "packaging/tests/test_install.py"), {"packaging"})
        self.assertEqual(self.groups("crates/pigeon-bridge/src/dav.rs"), {"rust", "bridge", "bridge_traces"})

    def test_core_and_daemon_changes_exercise_runtime_consumers(self):
        for path in ["crates/pigeon-core/src/purge.rs", "crates/pigeond/src/restore.rs",
                     "crates/pigeon-proto/src/api.rs", "crates/pigeon-git/src/lib.rs"]:
            self.assertEqual(self.groups(path), MODULE.RUNTIME)

    def test_formal_changes_check_proofs_models_and_implementation_traces(self):
        for path in ["models/tla/Authority.tla", "models/tla/Authority_mixed.cfg", "scripts/check-proofs"]:
            self.assertEqual(self.groups(path), MODULE.FORMAL)
            self.assertNotIn("bridge", self.groups(path))

    def test_annex_changes_cover_stock_contract_and_runtime_users(self):
        self.assertEqual(self.groups("third_party/git-annex/patches/fix.patch"), MODULE.RUNTIME | {"lab", "packaging"})

    def test_bridge_contract_changes_also_replay_real_multi_peer_transactions(self):
        self.assertEqual(self.groups("models/tla/BridgeSyncTrace.tla"), MODULE.FORMAL | {"bridge"})
        for path in ["scripts/check-bridge-lifecycle-traces", "scripts/check-bridge-sync-traces"]:
            self.assertEqual(self.groups(path), {"bridge", "bridge_traces"})

    def test_mixed_changes_union_all_required_consumers(self):
        self.assertEqual(self.groups("docs/note.md", "packaging/pigeon-service", "models/tla/Purge.tla"),
                         MODULE.FORMAL | {"packaging"})

    def test_unknown_paths_and_build_or_policy_changes_are_full(self):
        for path in ["unexpected.txt", "crates/new-runtime/src/lib.rs", "docs/data.bin",
                     "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml",
                     "crates/pigeon/Cargo.toml", "crates/pigeond/build.rs", "AGENTS.md",
                     ".github/workflows/ci.yml", "scripts/select-ci", "scripts/tests/test_select_ci.py"]:
            self.assertEqual(self.groups(path), MODULE.GROUPS, path)

    def test_web_client_toolchain_is_full_but_its_sources_are_runtime(self):
        for name in ["package.json", "package-lock.json", ".nvmrc", "build.mjs"]:
            path = f"crates/pigeon-web/client/{name}"
            self.assertEqual(self.groups(path), MODULE.GROUPS, path)
        for path in ["crates/pigeon-web/client/src/main.ts", "crates/pigeon-web/client/tsconfig.json"]:
            self.assertEqual(self.groups(path), MODULE.RUNTIME, path)

    def test_every_non_pr_event_is_full_without_reading_git(self):
        with patch.object(MODULE, "git", side_effect=AssertionError("must not inspect diff")):
            for event in ["push", "schedule", "workflow_dispatch", "workflow_call", "pull_request_target", ""]:
                self.assertEqual(set(MODULE.plan(event)["groups"]), MODULE.GROUPS, event)


class CompleteDiff(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/*"]\n[workspace.package]\nversion = "0.5.9"\n')
        for name in ["pigeon", "pigeon-core"]:
            self.write(f"crates/{name}/Cargo.toml", f'[package]\nname = "{name}"\nversion.workspace = true\n')
        self.write("Cargo.lock", 'version = 4\n\n' + "\n".join(
            f'[[package]]\nname = "{name}"\nversion = "0.5.9"\n' for name in ["pigeon", "pigeon-core"])
            + '\n[[package]]\nname = "external"\nversion = "1.0.0"\nsource = "registry+https://example.test"\nchecksum = "original"\n')
        self.write("docs/note.md", "original\n")
        self.base = self.commit()
        def run(*args):
            return subprocess.check_output(["git", *args], cwd=self.root, stderr=subprocess.PIPE).decode()
        self.override = patch.object(MODULE, "git", side_effect=run)
        self.override.start()
        self.addCleanup(self.override.stop)

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, stderr=subprocess.PIPE).decode().strip()

    def write(self, path, value):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(value)

    def change(self, path, before, after):
        file = self.root / path
        self.assertIn(before, file.read_text())
        file.write_text(file.read_text().replace(before, after))

    def commit(self):
        self.git("add", ".")
        self.git("-c", "user.name=CI fixture", "-c", "user.email=ci@example.test", "commit", "-qm", "fixture")
        return self.git("rev-parse", "HEAD")

    def bump(self):
        self.change("Cargo.toml", 'version = "0.5.9"', 'version = "0.5.10"')
        self.change("Cargo.lock", 'version = "0.5.9"', 'version = "0.5.10"')

    def result(self):
        return MODULE.plan("pull_request", self.base, self.commit())

    def test_required_version_bump_does_not_make_a_document_or_packaging_pr_full(self):
        self.bump()
        self.write("docs/note.md", "new words\n")
        self.assertEqual(self.result()["groups"], [])
        self.write("packaging/pigeon-service", "changed service\n")
        self.assertEqual(set(self.result()["groups"]), {"packaging"})

    def test_version_bump_preserves_formal_and_runtime_dependency_selection(self):
        self.bump()
        self.write("models/tla/Authority.tla", "model changed\n")
        self.assertEqual(set(self.result()["groups"]), MODULE.FORMAL)
        self.write("crates/pigeon-core/src/lib.rs", "runtime changed\n")
        self.assertEqual(set(self.result()["groups"]), MODULE.FORMAL | MODULE.RUNTIME)

    def test_dependency_change_hidden_alongside_version_bump_is_full(self):
        self.bump()
        self.change("Cargo.lock", 'checksum = "original"', 'checksum = "changed"')
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)

    def test_external_package_version_and_root_dependency_changes_are_full(self):
        self.bump()
        self.change("Cargo.lock", 'version = "1.0.0"', 'version = "1.0.1"')
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)
        self.git("reset", "--hard", self.base)
        self.bump()
        self.write("Cargo.toml", (self.root / "Cargo.toml").read_text() + '\n[workspace.dependencies]\nnew = "1"\n')
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)

    def test_incomplete_version_bump_is_full(self):
        self.change("Cargo.toml", 'version = "0.5.9"', 'version = "0.5.10"')
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)

    def test_invalid_prerelease_or_decreased_versions_keep_cargo_checks(self):
        for version in ["invalid", "0.05.10", "0.5.10-rc.1", "0.5.8"]:
            self.write("Cargo.toml", '[workspace]\nmembers = ["crates/*"]\n[workspace.package]\nversion = "' + version + '"\n')
            self.write("Cargo.lock", self.git("show", f"{self.base}:Cargo.lock").replace('version = "0.5.9"', f'version = "{version}"'))
            self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS, version)

    def test_malformed_lock_cannot_be_ignored(self):
        self.bump()
        self.write("Cargo.lock", "invalid = [\n")
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)

    def test_workspace_member_addition_and_duplicate_lock_record_are_full(self):
        self.bump()
        self.write("crates/new/Cargo.toml", '[package]\nname="new"\nversion.workspace=true\n')
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)
        self.git("reset", "--hard", self.base)
        self.bump()
        self.change("Cargo.lock", 'version = 4', 'version = 4\n[[package]]\nname="pigeon"\nversion="0.5.10"')
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)

    def test_registry_package_with_a_workspace_name_is_never_exempted(self):
        self.write("Cargo.lock", (self.root / "Cargo.lock").read_text()
                   + '\n[[package]]\nname="pigeon-core"\nversion = "0.5.9"\nsource="registry+https://example.test"\n')
        self.base = self.commit()
        self.bump()
        self.assertEqual(set(self.result()["groups"]), MODULE.GROUPS)

    def test_deleted_source_and_rename_to_docs_keep_the_old_path_dependencies(self):
        self.write("crates/pigeond/src/old.rs", "source\n")
        self.base = self.commit()
        self.git("mv", "crates/pigeond/src/old.rs", "docs/note-from-source.md")
        result = self.result()
        self.assertEqual(set(result["groups"]), MODULE.RUNTIME)
        self.assertIn("crates/pigeond/src/old.rs", result["paths"])
        self.assertIn("docs/note-from-source.md", result["paths"])

    def test_diff_exceeds_api_file_limits_and_keeps_newline_paths_intact(self):
        self.bump()
        for number in range(350):
            self.write(f"docs/change-{number}.md", "documentation\n")
        self.write("docs/space and\nnewline.md", "documentation\n")
        self.write("crates/pigeond/src/last.rs", "runtime change after many documents\n")
        result = self.result()
        self.assertGreater(len(result["paths"]), 350)
        self.assertIn("docs/space and\nnewline.md", result["paths"])
        self.assertEqual(set(result["groups"]), MODULE.RUNTIME)

    def test_missing_history_empty_diff_and_stale_base_cannot_reduce_checks(self):
        for base, head in [("0" * 40, self.base), (self.base, self.base), (None, None)]:
            self.assertEqual(set(MODULE.plan("pull_request", base, head)["groups"]), MODULE.GROUPS)
        self.git("checkout", "-qb", "topic")
        self.write("docs/note.md", "topic\n")
        head = self.commit()
        self.git("checkout", "-qb", "new-base", self.base)
        self.write("docs/other.md", "new base\n")
        base = self.commit()
        self.assertEqual(set(MODULE.plan("pull_request", base, head)["groups"]), MODULE.GROUPS)


class RequiredChecks(unittest.TestCase):
    def needs(self, groups):
        needs = {job: {"result": "success" if group in groups else "skipped"}
                 for job, group in MODULE.JOBS.items()}
        needs["repository"] = {"result": "success"}
        needs["changes"] = {"result": "success", "outputs":
                            {group: str(group in groups).lower() for group in MODULE.GROUPS}}
        return needs

    def test_only_intentionally_unselected_checks_may_be_skipped(self):
        for selected in [set(), {"packaging"}, MODULE.FORMAL, MODULE.RUNTIME, MODULE.GROUPS]:
            MODULE.verify_results(self.needs(selected))
        needs = self.needs(MODULE.RUNTIME)
        needs["daemons"]["result"] = "skipped"
        with self.assertRaises(ValueError):
            MODULE.verify_results(needs)

    def test_failed_cancelled_missing_or_undetermined_selection_never_pass(self):
        original = self.needs(set())
        for job, result in [("changes", "failure"), ("repository", "cancelled"),
                            ("rust", "failure"), ("models", "cancelled")]:
            needs = copy.deepcopy(original)
            needs[job]["result"] = result
            with self.assertRaises(ValueError):
                MODULE.verify_results(needs)
        for removed in ["changes", "rust"]:
            needs = copy.deepcopy(original)
            del needs[removed]
            with self.assertRaises(ValueError):
                MODULE.verify_results(needs)
        del original["changes"]["outputs"]["rust"]
        with self.assertRaises(ValueError):
            MODULE.verify_results(original)

    def test_lab_and_bridge_cannot_succeed_after_missing_or_skipped_selected_jobs(self):
        for scope in ["lab", "bridge"]:
            jobs = MODULE.SCOPES[scope]
            needs = {job: {"result": "success"} for job in jobs}
            needs["changes"] = {"result": "success", "outputs": {g: "true" for g in jobs.values()}}
            MODULE.verify_results(needs, scope)
            for job in jobs:
                skipped = copy.deepcopy(needs)
                skipped[job]["result"] = "skipped"
                with self.assertRaises(ValueError):
                    MODULE.verify_results(skipped, scope)
            for group in jobs.values():
                missing = copy.deepcopy(needs)
                del missing["changes"]["outputs"][group]
                with self.assertRaises(ValueError):
                    MODULE.verify_results(missing, scope)
            for job in jobs:
                needs[job]["result"] = "skipped"
            needs["changes"]["outputs"] = {g: "false" for g in jobs.values()}
            MODULE.verify_results(needs, scope)


if __name__ == "__main__":
    unittest.main()

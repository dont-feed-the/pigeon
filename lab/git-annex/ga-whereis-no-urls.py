#!/usr/bin/env python3
"""Recorded locations and trust outcomes survive omitting unused URL lookups.

All keys and URL claims are synthetic; no file contents or remote network
requests are needed. Both query modes must return every requested key.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def run():
    with tempfile.TemporaryDirectory(prefix="ga-whereis-") as directory:
        root = Path(directory)
        env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1",
                   GIT_TERMINAL_PROMPT="0")

        def git(*args, data=None):
            result = subprocess.run(["git", *args], cwd=root, env=env, input=data,
                                    capture_output=True, timeout=60)
            assert result.returncode == 0, f"native setup/query failed: {args[:2]}"
            return result.stdout

        git("init", "-q", "-b", "main")
        git("annex", "init", "-q", "location metadata contract")
        keys = ["SHA256E-s1--" + hashlib.sha256(f"metadata-{n}".encode()).hexdigest()
                for n in range(3)]
        ordinary = "11111111-2222-4333-8444-555555555555"
        web = "00000000-0000-0000-0000-000000000001"
        url = "https://example.invalid/synthetic-location-contract"
        git("annex", "registerurl", keys[0], url)
        git("annex", "setpresentkey", keys[1], ordinary, "1")

        def query(no_urls):
            args = ["git", "annex", "whereis", "--batch-keys", "--json"]
            if no_urls:
                args.append("--no-urls")
            result = subprocess.run(args, cwd=root, env=env,
                                    input=("\n".join(keys) + "\n").encode(),
                                    capture_output=True, timeout=60)
            assert result.returncode in (0, 1), "whereis did not complete"
            rows = [json.loads(line) for line in result.stdout.splitlines()]
            assert [row.get("key") for row in rows] == keys, "missing or reordered key replies"
            return rows

        def uuids(row, field):
            return {entry["uuid"] for entry in row.get(field, [])}

        for trust in ["trust", "untrust"]:
            git("annex", trust, "web", "--force")
            full, metadata = query(False), query(True)
            assert web in uuids(full[0], "whereis" if trust == "trust" else "untrusted")
            assert ordinary in uuids(full[1], "whereis") | uuids(full[1], "untrusted")
            assert not uuids(full[2], "whereis") | uuids(full[2], "untrusted")
            assert url in {u for field in ["whereis", "untrusted"]
                           for entry in full[0].get(field, []) for u in entry.get("urls", [])}
            for before, after in zip(full, metadata):
                assert before.get("success") == after.get("success")
                for field in ["whereis", "untrusted"]:
                    assert uuids(before, field) == uuids(after, field), "location/trust changed"
                    assert not any(entry.get("urls") for entry in after.get(field, []))
        assert not any(p.is_file() for p in (root / ".git/annex/objects").rglob("*"))
        print("PASS: no-urls retains complete key/UUID/trust replies and omits URLs; zero content objects")


if __name__ == "__main__":
    run()

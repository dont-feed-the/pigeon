#!/usr/bin/env python3
"""pigeon-pull PEER — proof of concept of pigeon's authorization rule as a policy layer over git-annex.

Fetches PEER's main branch, checks each new commit against pigeon's rule, merges the authorized
prefix, and keeps the first unauthorized commit (and what follows) as a proposal ref. Content and
location logs are then synced by git-annex itself (--only-annex).
Rule (spec Idea 2): owner(p) = u if the topmost folder is `u_`; owner may do anything there;
elsewhere (frozen) only additions are allowed.  User = commit author email local part
(a real version would use `git verify-commit` with SSH allowed-signers to bind users to keys)."""
import subprocess, sys

def git(*a, check=True):
    return subprocess.run(["git", *a], capture_output=True, text=True, check=check).stdout.strip()

def owner(path):
    top = path.split("/", 1)[0]
    return top[:-1] if "/" in path and top.endswith("_") and len(top) > 1 else None

def violations(commit):
    user = git("log", "-1", "--format=%ae", commit).split("@")[0]
    out = []
    for line in git("diff-tree", "--no-commit-id", "-r", "--name-status", "--no-renames", commit).splitlines():
        status, path = line.split("\t", 1)
        o = owner(path)
        ok = (o == user) if o else (status == "A")
        if not ok:
            out.append(f"{user} {'modifies' if status == 'M' else 'deletes' if status == 'D' else status} {path}"
                       + (f" (owned by {o})" if o else " (frozen)"))
    return user, out

peer = sys.argv[1]
subprocess.run(["git", "annex", "sync", "-q", "--only-annex", peer], capture_output=True)  # learn the peer's annex uuid + location logs
git("fetch", "-q", peer, f"main:refs/remotes/{peer}/main")
new = git("rev-list", "--reverse", f"HEAD..{peer}/main").split()
accepted = None
for c in new:
    user, bad = violations(c)
    if bad:
        git("update-ref", f"refs/proposals/{peer}/{c[:8]}", c)
        print(f"  ✗ {c[:8]} by {user}: " + "; ".join(bad) + f"  → kept as refs/proposals/{peer}/{c[:8]}")
        break
    print(f"  ✓ {c[:8]} by {user}: {git('log', '-1', '--format=%s', c)}")
    accepted = c
if accepted:
    git("merge", "-q", "--no-edit", accepted)
subprocess.run(["git", "annex", "sync", "-q", "--only-annex", "--content", peer], capture_output=True)

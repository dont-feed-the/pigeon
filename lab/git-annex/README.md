# git-annex experiments

Shell scripts run against git-annex 10.20261006 (standalone build in
`~/git-annex.linux`), each in its own lab directory under `~`. Results are
summarised in [`docs/design/GIT-ANNEX.md`](../../docs/design/GIT-ANNEX.md) and
in the proposal, §5.

| Script | Experiments | Asserts? |
|---|---|---|
| `ga-setup.sh` | lab with repositories A, B, C for E1–E6 | no |
| `ga-e1.sh` | E1 concurrent drops of the last copies | no, prints |
| `ga-e2.sh` | E2 deletion keeps content, E3 concurrent edits, E6 absent content | no, prints |
| `ga-e3.sh` | E6' fresh clone, E4 mass deletion | no, prints |
| `ga-e7.sh`, `pigeon-pull.py` | E7 proposals filtered at pull | no, prints |
| `ga-e8-thin.sh` | E8 thin laptops | no, prints |
| `ga-e9-custody.sh` | E9 roles: untrusted laptops, custodians, K = 2 | no, prints |
| `ga-e10-partial-custodians.sh`, `ga-e10c-partial-history.sh` | E10 partial custodians | no, prints |
| `ga-e15-sparse-folders.sh` | E15 freeing space per folder with sparse checkout (ADR 0031) | **yes**, exits non-zero on failure |
| `ga-e15b-pointer-free-transitions.sh` | E15b no pointer file during freeing and keeping, observed continuously, with `kill -9` trials (ADR 0039, issue #2) | **yes** |
| `ga-e18-unprotected-versions.sh` | E18 offline in-place saves against `annex.thin`, failed uploads, reconnection, re-thinning (ADR 0041, issue #3) | **yes** |
| `ga-e19-install-races.py` | E19 install racing application saves and `kill -9`, thin and not thin, with a fuzzing writer (ADR 0040, issue #1); Linux, about 3 minutes | **yes** |

The lab joins CI once its scripts assert, as E15, E15b, E18 and E19 do (ADR 0026).
Pending: E11 native Windows, E12 `remotedaemon` only fetches, E13
`ServeAppendOnly` over P2P, E14 purge by metadata.

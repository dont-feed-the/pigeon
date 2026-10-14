# pigeon's git-annex

pigeon runs git-annex 10.20261006 with the patches in `patches/` (ADR 0069).
Two of them fix bugs in git-annex's P2P protocol server that pigeon's
experiments found:

| Patch | Bug | Shown by |
|---|---|---|
| `0001-p2p-checkpresent-honours-removal-lock.patch` | `CHECKPRESENT` reports a copy that its owner holds locked for removal, so two concurrent drops over P2P each count the other's copy and both drop it (`P2P/Annex.hs`) | E23 |
| `0002-p2p-relay-connectdone-after-output.patch` | `CONNECTDONE` can overtake the end of git's output, so a fetch over P2P hangs for good, about twice in 100; it was also sent twice (`P2P/IO.hs` `runRelayService`) | E25, E12's retries |
| `0100-pigeon-version.patch` | none: it makes `git annex version` say `10.20261006-pigeon1` | |

pigeon keeps its own guards against both bugs (the filter's `CHECKPRESENT`
probe of 09 §9.4, deadlines on P2P fetches and the restart of a stuck
`remotedaemon`), so a stock git-annex stays safe to run.

## Building

`build.sh OUTDIR` fetches the release named in `VERSION` from Hackage,
checks its sha256, applies `patches/series` in order and builds it with the
pinned Stack, without the assistant and the benchmarks. A cold build takes
about an hour, so it runs on GitHub's runners only: `git-annex.yml` builds
it when this directory changes, caches the binaries by patch set and proves
the patches with E23 and E25 against the stock build. `slice.yml` and
`models-full.yml` restore the cached build through
`.github/actions/git-annex-pigeon`.

To move to a new upstream release, change `VERSION`, refresh the patches with
`patch -p1` on the new tree (drop those upstream has taken), and let
`git-annex.yml` prove the result.

## Licence and source

git-annex is licensed under the AGPL, version 3 or later (its `COPYRIGHT`
file), and these patches are under the same licence. Whoever gets pigeon's
build of git-annex, or talks to it over the network, can get its complete
source here: the Hackage tarball named in `VERSION` and these patches.

## Upstream

0001 and 0002 are written to be sent upstream as they are, with the
reproducers E23 (`lab/git-annex/ga-e23-p2p-drop.py`) and E25
(`lab/git-annex/ga-e25-relay-stress.py`). They have not been reported yet.
Once git-annex releases either fix, the patch is dropped at the next pin.

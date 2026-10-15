# Model safety proofs and symbolic checking

TLAPS is useful here: two checked proofs now remove domain-size and execution
length bounds from specific model safety claims. Apalache also checks the
capture invariant by induction, but was slower than TLC for this workload
and provides less coverage than the parameterized proof. It remains an
optional tool. Neither tool proves that the Rust implementation refines the
models; daemon traces, storage contracts and fault injection test that link.

## What is proved

The proof modules extend the original models and use their `Init`, `Next`
and safety predicates directly. Auxiliary state facts strengthen induction;
there is no separately maintained transition relation.

| Proof | Claim | Assumptions and scope |
|---|---|---|
| `CaptureTransactionProof.Safety` | `Spec => []CapturedBeforeDisplacement`: every displaced value was committed | Any `Values` containing the initial value `1`, including infinite sets; `Off = {}`. Any execution length, saves, staging, commit failures, rollback, receives and installs described by `CaptureTransaction`. It does not model annex object durability or daemon crashes at finer I/O boundaries. |
| `AnnexDropProof.Safety` | `Spec => [](TypeOK /\ LockedStays /\ INV_K)`: a locked copy stays present and at least K copies remain | Any finite `Cust`, natural `K <= Cardinality(Cust)`, and natural `MaxGets`; safe checks, named remotes, no trusted repositories, no log-only proof, **no lock expiry**. Any execution length, partitions, stale logs, aborts, dead markings, and gets admitted by `AnnexDrop`. It does not cover force drops, disk faults, corruption, ranked P2P checks or expiry. |

The capture proof keeps the staged value and prior committed index across a
failure. Only an idle install may displace the index, and that index is
committed. The drop proof establishes that every counted copy has a shared
lock, which prevents a removal lock on its holder. At removal, the counted
copies remain among the holders afterwards. Finite-set cardinality then
gives the K-copy bound. Lock expiry breaks that auxiliary lock fact, so the
proof deliberately makes no claim about expiry-enabled configurations.

`scripts/check-proofs` first proves every obligation with `--nofp` in a fresh
temporary model directory. It then uses TLC to evaluate the proof's own
`ProofParameters` expression against the original configuration constants.
This check also requires a nonempty initial state. A generated TLAPS theorem
separately proves that `Safety` implies the invariants actually named in the
configuration. TLAPS trusts imported theorems, so the runner first checks the
root proof module itself, then the generated implication. A change cannot
silently outgrow the proof's assumptions or add an unproved invariant.
Deadlock/liveness checks and operator overrides are refused for proof
coverage. Every `audit` row must have a checked proof/configuration mapping.

Four protocol mutations must leave actual unproved TLAPS obligations:
removing the idle install guard, failing to retain the committed value,
removing the K-copy threshold, and ignoring a shared lock when starting a
drop. Four configuration mutations must fail the coverage check: enabling
expiry, P2P presence checks, trusted repositories, or log-only proofs.
Syntax errors, tool failures and timeouts do not count as detected mutations.
A fifth mutation adds an invariant that is true initially but false later;
the generated theorem must fail. Omitted proofs and missing `Safety`
theorems fail the runner.

## Effect on CI

The proof job runs before the fast and full model jobs. Only
`AnnexDrop_design4` moves from the routine full set to `audit`: the proof
covers that instance and all larger finite custodian sets, every feasible K
and every natural get bound. The same finite instance still runs with TLC
weekly, by hand, and when explicitly named. This retains an independent
finite cross-check while removing a redundant graph traversal from each
model push.

All other model frequencies remain. In particular, `AnnexDrop_design` and
`AnnexDrop_gets2` have expiry enabled; the ranked and deliberately unsafe
configurations, liveness, all implementation traces, laws, contract tests,
and crash/fault tests keep their checks. `CaptureTransaction_safe` is cheap
and remains in the fast TLC set as another cross-check.

## Reproduce the checks and measurements

On Linux x86_64 with Python 3.12+ and Java 21:

```sh
scripts/check-proofs
scripts/check-proofs --benchmark
# Include the expensive finite graph in the comparison:
scripts/check-proofs --benchmark --include-slow
# Ordinary finite checks, and the independent proof-covered audit:
scripts/run-tlc --full
scripts/run-tlc AnnexDrop_design4
scripts/run-tlc --audit
```

The first command downloads and checks SHA-256 pins for TLAPS 1.5.0 and
TLC 1.7.4. The TLAPS installer compiles and self-tests its bundled Isabelle;
the installed toolchain is cached under `~/.cache/pigeon`. No proof
fingerprints are reused. The benchmark additionally downloads the pinned
Apalache 0.62.4 distribution. `--tlapm PATH` and `--apalache PATH` accept
existing installations after checking the exact version. The binary TLAPS
distribution is Linux x86_64; proofs can also be checked with a matching
local installation on another platform.

Logs and `timings.json`, including platform and commands, go to
`target/formal-proofs` (`--out DIR` changes it). Temporary paths in commands
belong to isolated copies of the repository's models. Timings include
process startup, exclude tool downloads/installation, and are single samples
on this workspace (Linux x86_64, Java 21, four available CPUs; TLC uses one
worker and a 1 GiB heap, TLAPS two workers). They are not CI benchmarks:

| Check | Wall time | Guarantee |
|---|---:|---|
| TLC `AnnexDrop_design4` | 358.8 s | Exhaustive safety for its four-custodian finite graph |
| TLAPS `AnnexDropProof` + configured-property proof + cfg check | 12.8 s | Parameterized safety under the assumptions above |
| TLC `CaptureTransaction_safe` | 0.84 s | Exhaustive safety over three values |
| TLAPS `CaptureTransactionProof` + configured-property proof + cfg check | 2.87 s | Parameterized safety for arbitrary values |
| Apalache capture bounded check, length 10 | 8.45 s | Safety only through ten transitions over three values |
| Apalache capture induction, base + step + implication | 21.1 s | Arbitrary execution length over the chosen three values |
| Apalache unguarded-install counterexample, length 6 | 7.16 s | Detects the existing negative design |

The complete proof job also checks mutations; it took about one minute with
tools already installed. A fresh download and TLAPS installation took about
55 seconds and was tested separately. Cache
restoration/download and first installation add overhead; the approximately
30-fold improvement above compares the drop proof and coverage check with
its finite traversal, not total workflow time or daemon testing time.

## Why not replace more with Apalache

[Apalache's default check](https://apalache-mc.org/docs/apalache/running.html)
is bounded model checking. A successful length-10 search does not exclude
a length-11 bug. The benchmark therefore also checks induction's three
obligations: initial states imply the strengthened invariant; an arbitrary
invariant state stays in it after `Next`; and the invariant implies the
safety claim. This proves arbitrary execution length **for the selected
finite value domain**, while TLAPS proves it without that domain bound.

`CaptureTransactionSymbolic` supplies constants with `ConstInit`. This avoids
Apalache's inability to type an empty `Off` set in the TLC configuration;
annotations in the model are comments with no TLA+ semantics. The auxiliary
invariant is shared with TLAPS. The Apalache counterexample test requires an
actual invariant violation, rather than treating a nonzero exit as evidence.

This is a useful independent symbolic check and an option for future models
with arithmetic or larger finite domains. It is not a general speedup here.
Other models use constructs such as recursive operators and `CHOOSE` that
need adaptation to Apalache's
[supported fragment](https://apalache-mc.org/docs/apalache/features.html).
Do not rewrite protocol semantics merely to obtain a faster solver verdict.

## Trust and limits

[TLAPS](https://proofs.tlapl.us/) checks the structured induction proofs using
its bundled SMT solver and temporal reasoning backend, with standard
finite-set theorems from its mathematical library. The pinned proof manager,
SMT encoding/solver and that library are trusted components. These are not
independently kernel-replayed certificates for the SMT steps. Scheduled TLC
audits provide another implementation of finite model checking, not a proof
that either tool is bug free.

These theorems say what the abstract actions preserve under explicit
assumptions. They do not establish universal implementation refinement,
filesystem atomicity, annex durability, hardware behavior, custody/repair
guarantees absent from the model, or liveness. Reducing those tests would
require evidence about those particular links and properties.

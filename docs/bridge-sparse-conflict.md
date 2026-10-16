# Sparse accepted conflicts and pending client files

> Mission: record the small model and concrete regression behind sparse
> bridge conflict recovery.

Concurrent client saves can leave a formerly accepted version under a native
conflict-copy name. A managed bridge peer may already have compacted that
version: its pointer remains in the sparse index, but the working-tree path
is absent. That absence is expected, and does not establish a pending local
file at the original name.

The old incoming installer checked whether the path lost its name before
handling sparse absence. It could create a `Concurrent` divergence with a
content key but no physical file or captured stat. Compaction then rejected
the missing pending file. Subsequent daemon rounds stopped before fetching,
preventing unrelated native edits from reaching the client.

`BridgeSparseConflict.tla` explores inspection, legacy repair, incoming
installation, compaction and the next fetch. A live write may appear while
inspection awaits Git reads. It checks that a hidden accepted path advances
its index without inventing an overlay; live files and real/uncertain pending
entries remain protected.

Legacy repair is permitted only for a missing, hidden `Concurrent` entry
without a captured stat, whose key matches the index and whose complete
indexed Git entry survives at an accepted conflict-copy name. The model
compares OID, mode and object kind. The corresponding pool object must be
verified, and absence must still hold after inspection. Suggestions, deletion
overlays, symlinks, unreadable paths and incomplete evidence retain their
entries. A surviving file or uncertain pending entry may require normal
capture/recovery; the model promises progress only for clean sparse input
and fully proven old phantom entries.

The good configuration explores 41 states and fair progress, including live
files, symlinks and unreadable paths alongside true absence. The legacy
configuration explores 14,345 states, including 2,048 combinations of
recovery evidence. Both pass in under two seconds locally. Negative controls
reproduce the old synthetic overlay and its resulting fetch stall, show that
checking only the sparse flag loses a live conflict's overlay, and show that
dropping complete-entry equality, pool verification or the final absence
check permits an unjustified repair.

These are abstract checks of branch ordering and evidence requirements,
not runtime trace replay. Accepted catalogue membership, Git reads, file
observations and verified pool content are model inputs. The model does not
prove filesystem atomicity, hashing, crash recovery, sparse Git behavior or
distributed convergence. The signed incoming-install regression and concrete
overlay recovery tests check implementation behavior separately.

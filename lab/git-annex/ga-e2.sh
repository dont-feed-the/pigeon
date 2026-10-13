#!/usr/bin/env bash
# E2 deletion keeps content, E3 concurrent edits, E6 a remote that never
# fetched content. Runs on the lab of ga-setup.sh; prints results, asserts nothing.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab; cd $L
obj() { find $1/.git/annex/objects -type f 2>/dev/null | wc -l; }
echo "== E2 deletion: A deletes docs/f.txt, B syncs with content (like the assistant)"
(cd B && git annex get -q . )
(cd A && git rm -q docs/f.txt && git commit -qm "delete f" && git annex sync -q --content >/dev/null 2>&1)
before=$(obj B); (cd B && git annex sync -q --content >/dev/null 2>&1)
echo "B tree has f.txt? $(test -e B/docs/f.txt && echo yes || echo no); B objects before/after sync: $before/$(obj B)"
(cd B && git annex unused 2>&1 | grep -E 'unused|SHA' | head -3)
echo "== E3 conflict: A and B edit docs/s.txt concurrently"
(cd A && echo "A's edit" > docs/s.txt && git annex add -q docs/s.txt && git commit -qm "A edits s")
(cd B && echo "B's edit" > docs/s.txt && git annex add -q docs/s.txt && git commit -qm "B edits s")
(cd A && git annex sync -q >/dev/null 2>&1); (cd B && git annex sync -q >/dev/null 2>&1); (cd A && git annex sync -q >/dev/null 2>&1)
echo "B docs: $(ls B/docs | tr '\n' ' ')"; echo "A docs: $(ls A/docs | tr '\n' ' ')"
for f in B/docs/s.txt*; do echo "  $f: $(cat $f)"; done
echo "== E5 numcopies=2, only A and B hold big.bin: drop in A"
(cd A && git annex numcopies 2 >/dev/null && git annex get -q docs/big.bin; git annex drop docs/big.bin 2>&1 | tail -4)
(cd A && git annex numcopies 1 >/dev/null)
echo "== E6 partial: C never fetched content"
(cd C && git annex sync -q >/dev/null 2>&1; echo "C docs/big.bin size: $(stat -c %s docs/big.bin) bytes; head: $(head -c 70 docs/big.bin)")
(cd C && git annex adjust --hide-missing >/dev/null 2>&1; echo "C after adjust --hide-missing: $(ls docs | tr '\n' ' ') ; branch: $(git branch --show-current)")

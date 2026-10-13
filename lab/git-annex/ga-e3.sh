#!/usr/bin/env bash
# E6' a fresh clone never gets content, E4 mass deletion (rm -rf) then sync.
# Runs on the lab of ga-setup.sh; prints results, asserts nothing.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab; cd $L
obj() { find $1/.git/annex/objects -type f -size +900k 2>/dev/null | wc -l; }
echo "== E5' numcopies=2, only A and B hold big.bin"
(cd C && git annex drop -q docs/big.bin >/dev/null 2>&1)
(cd A && git annex get -q docs/big.bin; git annex numcopies 2 >/dev/null; git annex whereis docs/big.bin 2>&1 | head -1; git annex drop docs/big.bin 2>&1 | grep -v '^$' | tail -4; git annex numcopies 1 >/dev/null)
echo "== E6' partial: fresh clone D never gets content"
git clone -q A D; (cd D && git annex init -q D && git annex sync -q >/dev/null 2>&1; echo "D docs/big.bin: $(stat -c %s docs/big.bin) bytes, content: $(head -c 80 docs/big.bin | tr -d '\0')"; git annex adjust -q --hide-missing >/dev/null 2>&1; echo "D after --hide-missing: [$(ls docs | tr '\n' ' ')]")
echo "== E2' deletion content after everyone synced"
for r in A B C; do (cd $r && git annex sync -q >/dev/null 2>&1); done
(cd B && git annex unused 2>&1 | grep -E '^ +[0-9]' | head -3; echo "B objects still on disk: $(find .git/annex/objects -type f | wc -l)")
echo "== E4 mass delete: B loses docs/ (rm -rf), runs assist"
(cd B && git annex get -q . ; rm -rf docs; git annex assist -q >/dev/null 2>&1; git log --oneline -1)
(cd A && git annex assist -q >/dev/null 2>&1; echo "A docs now: [$(ls docs 2>/dev/null | tr '\n' ' ')]; A big objects on disk: $(find .git/annex/objects -type f -size +900k | wc -l)")
(cd A && git revert --no-edit HEAD >/dev/null 2>&1 && git annex assist -q >/dev/null 2>&1; echo "A after git revert: [$(ls docs | tr '\n' ' ')], big.bin bytes: $(stat -c %s docs/big.bin)")

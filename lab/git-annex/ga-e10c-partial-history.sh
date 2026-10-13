#!/usr/bin/env bash
# E10c: do balanced partial custodians also keep OLD versions (history)?
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab6; chmod -R u+w $L 2>/dev/null; rm -rf $L; mkdir -p $L; cd $L
q() { "$@" >/dev/null 2>&1; }
git init -q L1; cd L1; git annex init -q laptop; git annex config --set annex.addunlocked true >/dev/null
q git annex numcopies 2; q git annex mincopies 2; q git annex untrust here
mkdir docs; for i in 1 2 3 4; do head -c 300000 /dev/urandom > docs/f$i.bin; done; git annex add -q .; git -c user.email=a@p commit -qm init; cd ..
for c in C1 C2 C3; do git clone -q L1 $c; (cd $c && git annex init -q $c && git remote rename origin L1); done
for r in L1 C1 C2 C3; do (cd $r; git config annex.synccontent false; git config annex.autocommit false; for o in L1 C1 C2 C3; do [ $r = $o ] || git remote add $o ../$o 2>/dev/null; done); done
(cd L1 && q git annex sync && q git annex groupwanted custodian "sizebalanced=custodian:2"; for c in C1 C2 C3; do q git annex group $c custodian; q git annex wanted $c groupwanted; q git annex maxsize $c "10 megabytes"; done; q git annex sync)
pass() { for r in 1 2; do for c in C1 C2 C3; do (cd $c && q git annex sync --content --all); done; done; (cd L1 && q git annex sync); }
pass
old=$(cd L1 && git annex lookupkey docs/f1.bin)
(cd L1 && head -c 300000 /dev/urandom > docs/f1.bin && git annex add -q docs/f1.bin && git -c user.email=a@p commit -qm edit && q git annex sync)
pass
(cd L1 && echo "current f1: $(git annex whereis docs/f1.bin 2>/dev/null | head -1 | grep -o '[0-9]* cop[a-z]*')   old f1 (history): $(git annex whereis --key $old 2>/dev/null | head -1 | grep -o '[0-9]* cop[a-z]*')")

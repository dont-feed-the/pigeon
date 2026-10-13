#!/usr/bin/env bash
# E10: partial custodians. 10 files x 1 MB, K = 2 -> 20 MB of custody needed. Custodians use sizebalanced=custodian:2 with maxsize.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab5; chmod -R u+w $L 2>/dev/null; rm -rf $L; mkdir -p $L; cd $L
q() { "$@" >/dev/null 2>&1; }
git init -q L1; cd L1; git annex init -q laptop; git annex config --set annex.addunlocked true >/dev/null; git config annex.thin true
q git annex numcopies 2; q git annex mincopies 2; q git annex untrust here
mkdir docs; for i in $(seq -w 1 10); do head -c 1000000 /dev/urandom > docs/f$i.bin; done; git annex add -q .; git -c user.email=a@p commit -qm init; cd ..
for c in C1 C2 C3; do git clone -q L1 $c; (cd $c && git annex init -q $c && git remote rename origin L1); done
for r in L1 C1 C2 C3; do (cd $r; git config annex.synccontent false; for o in L1 C1 C2 C3; do [ $r = $o ] || git remote add $o ../$o 2>/dev/null; done); done
(cd L1 && q git annex sync && q git annex groupwanted custodian "sizebalanced=custodian:2"
 for c in C1 C2 C3; do q git annex group $c custodian; q git annex wanted $c groupwanted; done
 q git annex maxsize C1 "4.5 megabytes"; q git annex maxsize C2 "4.5 megabytes"; q git annex maxsize C3 "4.5 megabytes"; q git annex sync)
pass() { for r in 1 2; do for c in C1 C2 C3; do (cd $c && q git annex sync --content --all); done; done; (cd L1 && q git annex sync); }
report() { (cd L1; for c in C1 C2 C3; do printf "%s holds %s files; " $c $(git annex find --in=$c docs | wc -l); done
  echo; echo "   protected (>= 2 custodian copies): $(git annex find --copies=custodian:2 docs | wc -l)/10 ; unprotected: [$(git annex find --lackingcopies=1 docs | sed 's|docs/||' | tr '\n' ' ')]"); }
echo "== phase 1: capacity 3 x 4.5 MB = 13.5 MB < 20 MB needed"; pass; report
echo "== laptop tries to evict everything"; (cd L1 && git annex drop docs 2>&1 | grep -c '^drop.*ok' | sed 's/^/   evicted: /'; echo "   still on laptop: $(git annex find --in=here docs | wc -l) files")
echo "== phase 2: C3 grows to 12 MB"; (cd L1 && q git annex maxsize C3 "12 megabytes"; q git annex sync); pass; report
echo "== C2 is lost: declared dead, then the others run their passes"; (cd L1 && q git annex dead C2; q git annex sync); report; pass; report

#!/usr/bin/env bash
# Lab setup: three git-annex repositories A, B, C under ~/ga-lab, wired as
# remotes of each other, for the experiments ga-e*.sh. Prints, asserts nothing.
set -e
export PATH=~/git-annex.linux:$PATH
git config --global user.name pigeon >/dev/null; git config --global user.email p@example.org; git config --global init.defaultBranch main
L=~/ga-lab; rm -rf $L; mkdir -p $L; cd $L
git init -q A; cd A; git annex init -q A; git annex config --set annex.addunlocked true >/dev/null
mkdir docs alice_; echo hello > docs/f.txt; echo shared > docs/s.txt; head -c 1000000 /dev/urandom > docs/big.bin; echo "alice's" > alice_/r.txt
git annex add -q . ; git commit -qm init
cd $L; for r in B C; do git clone -q A $r; (cd $r && git annex init -q $r && git remote rename origin A); done
for r in A B C; do (cd $r; for o in A B C; do [ $r = $o ] || git remote add $o ../$o 2>/dev/null || true; done); done
for r in A B C; do (cd $r && git annex sync -q 2>/dev/null); done
(cd B && git annex get -q docs/big.bin)
echo setup-done

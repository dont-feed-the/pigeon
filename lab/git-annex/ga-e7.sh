#!/usr/bin/env bash
# E7 proposals: bob commits on B, alice's node pulls them with pigeon-pull.py
# and accepts one by cherry-pick. Prints results, asserts nothing.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab2; chmod -R u+w $L 2>/dev/null; rm -rf $L; mkdir -p $L; cd $L
c() { git -c user.name=$1 -c user.email=$1@pigeon commit -qm "$2"; }
git init -q A; cd A; git annex init -q alice-laptop; git annex config --set annex.addunlocked true >/dev/null
mkdir docs alice_; echo v1 > docs/report.txt; echo "alice's notes" > alice_/notes.txt; git annex add -q .; c alice init
cd ..; git clone -q A B; cd B; git annex init -q bob-laptop; git remote rename origin A; cd ../A; git remote add B ../B; cd ..
echo "== bob works on B (3 commits)"
(cd B && echo "photos from bob" > docs/new.txt && git annex add -q docs/new.txt && c bob "bob adds docs/new.txt")
(cd B && echo "bob was here" > alice_/notes.txt && git annex add -q alice_/notes.txt && c bob "bob edits alice_/notes.txt")
(cd B && echo "v2 by bob" > docs/report.txt && git annex add -q docs/report.txt && c bob "bob edits frozen docs/report.txt")
echo "== alice's node runs pigeon-pull B"
(cd A && python3 ~/pigeon-pull.py B)
(cd A && echo "A tree: $(git ls-files | tr '\n' ' ')"; echo "docs/new.txt content on A: $(cat docs/new.txt)"; echo "alice_/notes.txt on A: $(cat alice_/notes.txt)"; echo "proposals: $(git for-each-ref --format='%(refname)' refs/proposals)")
echo "== alice reviews and accepts bob's proposal for her own folder (cherry-pick, committed as alice)"
(cd A && p=$(git for-each-ref --format="%(objectname)" refs/proposals | head -1) && git cherry-pick -n $p >/dev/null 2>&1; git annex get -q alice_/notes.txt; c alice "accept bob's proposal"; echo "alice_/notes.txt on A: $(cat alice_/notes.txt)")

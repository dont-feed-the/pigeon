#!/usr/bin/env bash
# E8 thin laptops: B drops both files (numcopies 1) relying on A's copies.
# Prints results, asserts nothing.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab3; chmod -R u+w $L 2>/dev/null; rm -rf $L; mkdir -p $L; cd $L
c() { git -c user.name=$1 -c user.email=$1@pigeon commit -qm "$2"; }
git init -q A; cd A; git annex init -q A; git annex config --set annex.addunlocked true >/dev/null; git config annex.thin true
mkdir docs; head -c 1000000 /dev/urandom > docs/inplace.bin; head -c 1000000 /dev/urandom > docs/rename.bin; git annex add -q .; c alice init; git annex fix -q . 2>/dev/null
cd ..; git clone -q A B; cd B; git annex init -q B; git remote rename origin A; git annex get -q .; cd ../A; git remote add B ../B; git annex sync -q >/dev/null 2>&1; cd ..
k1=$(cd A && git annex lookupkey docs/inplace.bin); k2=$(cd A && git annex lookupkey docs/rename.bin)
echo "links: inplace.bin nlink=$(stat -c %h A/docs/inplace.bin), rename.bin nlink=$(stat -c %h A/docs/rename.bin)   (2 = hard-linked to the annex object)"
echo "du A (work tree + annex): $(du -sh A --exclude=.git/objects | cut -f1) for 2 MB of files"
# (a) an application modifies inplace.bin IN PLACE (same inode); (b) an editor writes a new file and renames it over rename.bin
printf 'XXXX' | dd of=A/docs/inplace.bin bs=1 seek=0 conv=notrunc status=none
head -c 1000000 /dev/urandom > A/docs/.tmp && mv A/docs/.tmp A/docs/rename.bin
check() { # is an INTACT copy of key $1 present in repo $2 ?
  f=$(find $2/.git/annex/objects -name "$1" -type f 2>/dev/null | head -1)
  if [ -z "$f" ]; then echo absent; elif [ "$(sha256sum < "$f" | cut -c1-64)" = "$(echo $1 | sed 's/.*--//; s/\..*//')" ]; then echo intact; else echo CORRUPT; fi; }
echo "after edits, before any commit:  A has old inplace: $(check $k1 A) | A has old rename: $(check $k2 A)"
echo "== B now unsyncs both files (numcopies 1), relying on A's copies"
(cd B && git annex drop docs/inplace.bin 2>&1 | tail -2 | tr '\n' ' '; echo; git annex drop docs/rename.bin 2>&1 | tail -2 | tr '\n' ' '; echo)
echo "intact copies left of OLD inplace.bin: A=$(check $k1 A) B=$(check $k1 B)"
echo "intact copies left of OLD rename.bin:  A=$(check $k2 A) B=$(check $k2 B)"

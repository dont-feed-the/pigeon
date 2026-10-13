#!/usr/bin/env bash
# E9: thin laptops are UNTRUSTED editable caches; custodians hold locked, read-only objects and count toward K = 2.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab4; chmod -R u+w $L 2>/dev/null; rm -rf $L; mkdir -p $L; cd $L
q() { "$@" >/dev/null 2>&1; }
git init -q L1; cd L1; git annex init -q laptop1; git annex config --set annex.addunlocked true >/dev/null; git config annex.thin true
q git annex numcopies 2; q git annex mincopies 2; q git annex untrust here
mkdir docs; head -c 500000 /dev/urandom > docs/old.bin; git annex add -q .; git -c user.email=alice@p commit -qm init; cd ..
for c in C1 C2 C3; do git clone -q L1 $c; (cd $c && git annex init -q $c && git config annex.addunlocked false && git remote rename origin L1); done
for r in L1 C1 C2 C3; do (cd $r; git config annex.synccontent false; for o in L1 C1 C2 C3; do [ $r = $o ] || git remote add $o ../$o 2>/dev/null; done); done
(cd L1 && q git annex sync && q git annex groupwanted custodian "include=*" && for c in C1 C2; do q git annex group $c custodian; q git annex wanted $c groupwanted; done; q git annex wanted C3 "exclude=*"
 q git annex copy --to C1 docs/old.bin; q git annex copy --to C2 docs/old.bin; q git annex sync)
counted() { (cd ~/ga-lab4/L1 && git annex whereis "$1" 2>/dev/null | head -1 | grep -o '[0-9]* cop[a-z]*'); }
lacking() { (cd ~/ga-lab4/L1 && git annex find --lackingcopies=1 docs/ | tr '\n' ' '); }
drop() { (cd ~/ga-lab4/L1 && git annex drop "$@" 2>&1 | grep -o 'failed\|ok' | head -1); }
echo "1. new.bin created on the thin, untrusted laptop: counted=$(counted docs/new.bin) unprotected=[$(cd L1 && head -c 500000 /dev/urandom > docs/new.bin && git annex add -q docs/new.bin && git -c user.email=alice@p commit -qm new && q git annex sync; lacking)] laptop-unsync=$(drop docs/new.bin)"
(cd L1 && q git annex copy --to C1 docs/new.bin)
echo "2. copied to C1:                       counted=$(counted docs/new.bin) unprotected=[$(lacking)] laptop-unsync=$(drop docs/new.bin)"
(cd L1 && q git annex copy --to C2 docs/new.bin)
echo "3. copied to C2:                       counted=$(counted docs/new.bin) unprotected=[$(lacking)] laptop-unsync=$(drop docs/new.bin)"
(cd L1 && q git annex get docs/old.bin)
echo "4. laptop holds old.bin; drop --from C1 (would leave C2 + untrusted laptop): $(drop --from C1 docs/old.bin)"
printf 'XXXX' | dd of=L1/docs/old.bin bs=1 seek=0 conv=notrunc status=none
k=$(cd C1 && git annex lookupkey docs/old.bin); ok() { f=$(find $1/.git/annex/objects -name "$k" -type f | head -1); [ -n "$f" ] && [ "$(sha256sum < $f | cut -c1-64)" = "$(echo $k | sed 's/.*--//; s/\..*//')" ] && echo intact || echo "not intact"; }
echo "5. in-place edit of old.bin on the laptop: C1 $(ok C1), C2 $(ok C2), laptop $(ok L1); C1 object writable? $(test -w $(find C1/.git/annex/objects -name "$k" -type f) && echo yes || echo no)"
(cd L1 && q git annex dead C2; q git annex group C3 custodian; q git annex wanted C3 groupwanted; q git annex sync)
echo "6. C2 declared dead, C3 made custodian: unprotected=[$(lacking)]"
(cd C3 && q git annex sync --content); (cd L1 && q git annex sync)
echo "   after C3's repair pass (sync --content): unprotected=[$(lacking)]  pre-edit old.bin on C3: $(ok C3)"
echo "7. pre-edit version (history) after the repair pass: $(cd L1 && git annex whereis --key $k 2>/dev/null | head -1 | grep -o '[0-9]* cop[a-z]*')"
(cd L1 && q git annex groupwanted custodian anything; q git annex sync); (cd C3 && q git annex sync --content --all); (cd L1 && q git annex sync)
echo "   with custodian wanted = anything and --all:            $(cd L1 && git annex whereis --key $k 2>/dev/null | head -1 | grep -o '[0-9]* cop[a-z]*')"

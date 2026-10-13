#!/usr/bin/env bash
# E1: A and B each hold the only two copies (numcopies=1) and drop concurrently.
export PATH=~/git-annex.linux:$PATH
L=~/ga-lab; cd $L
loss=0; one=0; none=0
for i in $(seq 1 ${1:-40}); do
  (cd A && git annex get -q docs/big.bin) ; (cd B && git annex get -q docs/big.bin)
  (cd A && git annex drop -q docs/big.bin >/dev/null 2>&1) & (cd B && git annex drop -q docs/big.bin >/dev/null 2>&1) & wait
  a=$(cd A && git annex find --in=here docs/big.bin | wc -l); b=$(cd B && git annex find --in=here docs/big.bin | wc -l)
  # ground truth: is the object file physically present?
  pa=$(find A/.git/annex/objects -type f -name 'SHA256E*' -size +900k | wc -l); pb=$(find B/.git/annex/objects -type f -name 'SHA256E*' -size +900k | wc -l)
  t=$((pa+pb)); if [ $t -eq 0 ]; then loss=$((loss+1)); (cd A && git annex fsck -q --from B docs/big.bin >/dev/null 2>&1); echo "LOSS at trial $i"; break; elif [ $t -eq 1 ]; then one=$((one+1)); else none=$((none+1)); fi
done
echo "trials done: loss=$loss  one-dropped=$one  none-dropped=$none"

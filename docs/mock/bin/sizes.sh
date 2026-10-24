#!/usr/bin/env bash
# Physical-pixel screenshots of the surface on Myoko (2880x1800, scale 2) and a 1920x1080 screen (scale 1).
set -euo pipefail
cd /tmp/pyxamp-mock
one() { # name screen dbl fold
  local s="sz-$$-$RANDOM" lw lh d
  if [ "$2" = myoko ]; then lw=1440; lh=900; d=2; else lw=1920; lh=1080; d=1; fi
  agent-browser --session "$s" open "file:///tmp/pyxamp-mock/sizes.html?screen=$2&dbl=$3&fold=$4" >/dev/null
  agent-browser --session "$s" set viewport $lw $lh $d >/dev/null; agent-browser --session "$s" reload >/dev/null; agent-browser --session "$s" wait 900 >/dev/null
  local p; p=$(agent-browser --session "$s" eval "JSON.stringify(window.PLAN)" | tr -d '\\')
  agent-browser --session "$s" screenshot "shots/sizes/$1.png" >/dev/null; agent-browser --session "$s" close >/dev/null
  echo "shots/sizes/$1.png $p"
}
one myoko-2x-default        myoko 0 none
one myoko-4x-ctrl-d         myoko 1 none
one myoko-2x-eq-folded      myoko 0 eq
one myoko-2x-both-folded    myoko 0 both
one myoko-2x-shade-folded   myoko 0 shade
one fhd-1x-default          fhd 0 none
one fhd-2x-ctrl-d           fhd 1 none
one fhd-1x-pl-folded        fhd 0 pl
one fhd-2x-ctrl-d-shade     fhd 1 shade

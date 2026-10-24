#!/usr/bin/env bash
# shot.sh <out.png> <page-and-query> [width height dpr]
set -euo pipefail
out=$1; page=$2; w=${3:-1280}; h=${4:-800}; d=${5:-1}
s="pyx-$$-$RANDOM"
agent-browser --session "$s" open "file:///tmp/pyxamp-mock/$page" >/dev/null
agent-browser --session "$s" set viewport "$w" "$h" "$d" >/dev/null
agent-browser --session "$s" reload >/dev/null
agent-browser --session "$s" wait 1200 >/dev/null
agent-browser --session "$s" screenshot "$out" >/dev/null
echo "errors: $(agent-browser --session "$s" errors 2>/dev/null | head -5)"
agent-browser --session "$s" close >/dev/null
echo "$out"

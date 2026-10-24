#!/usr/bin/env bash
# sheet.sh <out.png> <query>: renders sheet.html 1840 logical px wide at DPR 1, full height.
set -euo pipefail
out=$1; qs=$2; s="sh-$$-$RANDOM"
ab() { agent-browser --session "$s" "$@"; }
ab open "file:///tmp/pyxamp-mock/sheet.html?$qs" >/dev/null
ab set viewport 1840 1000 1 >/dev/null
ab reload >/dev/null
for i in $(seq 1 60); do d=$(ab eval "document.body.dataset.done || ''" | tr -d '"'); [ "$d" = 1 ] && break; sleep 0.5; done
h=$(ab eval "document.body.scrollHeight" | tr -d '"')
ab set viewport 1840 "$h" 1 >/dev/null
ab wait 500 >/dev/null
ab screenshot "$out" >/dev/null
e=$(ab eval "(window.errs || []).join(' | ') || 'none'" | tr -d '"')
ab close >/dev/null
echo "$out ($h px tall, page errors: $e)"

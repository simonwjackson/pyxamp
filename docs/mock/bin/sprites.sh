#!/usr/bin/env bash
# Writes every sprite sheet at 1x into sprites/, and a 4x view into shots/sprites-4x.png.
set -euo pipefail
cd /tmp/pyxamp-mock
s="sp-$$"; ab() { agent-browser --session "$s" "$@"; }
ab open "file:///tmp/pyxamp-mock/sprites.html" >/dev/null; ab set viewport 1300 900 1 >/dev/null; ab reload >/dev/null
for i in $(seq 1 40); do [ "$(ab eval "document.body.dataset.done || ''" | tr -d '"')" = 1 ] && break; sleep 0.5; done
ab eval "JSON.stringify(window.exportAll())" > /tmp/pyx-sprites.json
python3 - <<'PY'
import json, base64
d = json.loads(json.loads(open('/tmp/pyx-sprites.json').read()))
for k, v in d.items():
    open(f'/tmp/pyxamp-mock/sprites/{k}.png', 'wb').write(base64.b64decode(v.split(',', 1)[1])); print(f'sprites/{k}.png')
PY
h=$(ab eval "document.body.scrollHeight" | tr -d '"'); ab set viewport 1300 "$h" 1 >/dev/null; ab wait 300 >/dev/null
ab screenshot shots/sprites-4x.png >/dev/null; ab close >/dev/null; echo shots/sprites-4x.png

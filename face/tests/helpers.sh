# helpers.sh: shared helpers for the face checks inside a running session
# (session.sh). Sourced by face/tools/verify and face/tools/verify-step2.
# Shots are kept in $OUT.

commands() { ctl '{"commands":true}' | jq -c '[.commands[].command | {tag: .action._tag, payload: .action.payload, bus: .target.busName, owner: .target.owner}]'; }
clear_commands() { ctl '{"clear":true}' >/dev/null; }
last_command() { commands | jq -c '.[-1] // empty'; }
load() { ctl "{\"load\":\"$1\"}" >/dev/null; sleep 0.6; }
geometry() { layer_geometry; }
# pixel X Y in a shot -> "#rrggbb"
pixel() { magick "$T/shots/$1.png" -format "%[hex:p{$2,$3}]" info: | cut -c1-6 | tr 'A-F' 'a-f' | sed 's/^/#/'; }
crop() { # crop SHOT W H X Y OUTNAME: keep a crop in $OUT
  magick "$T/shots/$1.png" -crop "$2x$3+$4+$5" +repage "$OUT/$6.png"
}
surface_crop() { # surface_crop SHOT OUTNAME [EXTRA_W EXTRA_H]: the surface (plus room for popups) in physical px
  local g k s mx lx ly lw lh
  g=$(describe); k=$(echo "$g" | jq -r .k); s=$(monitor_scale "$(echo "$g" | jq -r .screen)"); mx=$(monitor_x "$(echo "$g" | jq -r .screen)")
  read -r lx ly lw lh <<< "$(layer_geometry)"
  crop "$1" "$(echo "$g" | jq -r ".physW + ${3:-0}")" "$(echo "$g" | jq -r ".physH + ${4:-0}")" "$(jq -n "($lx - $mx) * $s | floor")" "$(jq -n "$ly * $s | floor")" "$2"
}
# Step-2 requests the replay server received: [{tag, payload}].
requests() { ctl '{"requests":true}' | jq -c '[.requests[].request.action | {tag: ._tag, payload: .payload}]'; }
# Those of one tag (or tags joined by |).
requests_of() { requests | jq -c --arg t "$1" '[.[] | select(.tag | test("^(" + $t + ")$"))]'; }
last_request() { requests | jq -c '.[-1] // empty'; }
# load_at NAME T: serve a fixture with its wall-clock fields moved so that fixture time T is now.
load_at() { ctl "{\"load\":\"$1\",\"rebaseWall\":$2}" >/dev/null; sleep 0.6; }

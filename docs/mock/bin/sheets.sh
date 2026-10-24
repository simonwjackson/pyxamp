#!/usr/bin/env bash
# Renders every state board into shots/states/.
set -uo pipefail
cd /tmp/pyxamp-mock
names=(x 1-pyxis-host 2-pyxis-console 3-spoken-word 4-jellyfin 5-followed-player 6-playlist-section 7-eq-section 8-edge-states)
for g in 1 2 3 4 5 6 7 8; do bin/sheet.sh "shots/states/${names[$g]}-2x.png" "g=$g&k=2"; done
bin/sheet.sh shots/states/1-pyxis-host-1x.png 'g=1&k=1'
bin/sheet.sh shots/states/6-playlist-section-1x.png 'g=6&k=1'
bin/sheet.sh shots/states/5-followed-player-1x.png 'g=5&k=1&only=f-follow,f-back-1,f-back-2,f-back-3,f-shade'

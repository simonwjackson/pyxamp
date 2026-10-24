#!/usr/bin/env bash
# Live-page stills (index.html with the mock-controls panel) and the squint pair.
set -uo pipefail
cd /tmp/pyxamp-mock
i() { bin/shot.sh "shots/$1.png" "$2" "${3:-1600}" "${4:-1000}" "${5:-1}" >/dev/null && echo "shots/$1.png"; }
i 01-live-default-2x          'index.html?mon=2&t=4'
i 02-live-default-1x          'index.html?mon=1&t=4'
i 03-live-following-firefox   'index.html?mon=2&state=f-follow&hold=1'
i 04-live-console-play-on     'index.html?mon=2&state=c-menu&hold=1'
i 05-live-spoken-word-menu    'index.html?mon=2&state=s-rowmenu&hold=1'
i 06-live-jump-box            'index.html?mon=2&state=p-jump&hold=1'
i 07-live-desktop-mode        'index.html?mon=2&state=x-desktop&hold=1'
i 08-live-dpr2-myoko          'index.html?t=4&ui=0' 1440 900 2
i 09-live-reduced-motion-vis-off 'index.html?mon=2&rm=1&t=4'
# squint: the 2x surface, blurred, so the silhouette can be judged on its own
bin/magick shots/01-live-default-2x.png -crop 600x900+20+16 +repage shots/squint-sharp.png
bin/magick shots/squint-sharp.png -blur 0x6 shots/squint-blurred.png && echo shots/squint-blurred.png

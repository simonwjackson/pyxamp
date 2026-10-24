#!/usr/bin/env bash
# Records the motion take on the live page at 2x (Myoko scale): 3.5 s of marquee and spectrum,
# EQ fold and unfold (click the EQ button; instant), SUPER+Z hide and show (instant).
# RM=1 records the same take with prefers-reduced-motion: reduce.
# A variant of ~/.agents/skills/motion-mockup/scripts/take.sh (mouse steps need coordinates).
set -euo pipefail
cd /tmp/pyxamp-mock
export PATH=/nix/store/9qh7i54hk78czcdvxwd78q4i1idc9mc3-ffmpeg-9.0.2-bin/bin:$PATH
F=bin/ffmpeg; W=620; H=960; base="shots/motion/take-${W}x${H}-2x${RM:+-reduced-motion}"
s="take-$$"; ab() { agent-browser --session "$s" "$@"; }
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&ui=0" >/dev/null
ab set viewport $W $H 1 >/dev/null
[ -n "${RM:-}" ] && ab set media dark reduced-motion >/dev/null
ab reload >/dev/null; ab wait 1200 >/dev/null
pos() { ab eval "(() => { const h = pyx.hits().find(h => h.id === '$1'); const k = pyx.k / devicePixelRatio; return Math.round((h.x + h.w / 2) * k) + ' ' + Math.round((h.y + h.h / 2) * k); })()" | tr -d '"'; }
EQ=$(pos eqbtn)
ab mouse move 600 940 >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null
ab eval "window.play && play(); pyx.S.marqueeT0 = now(); pyx.S.queue[2] = { ...LONG_TRACK }; true" >/dev/null
ab record start "$base.mp4" --fps ${FPS:-30} >/dev/null
ab wait 3500 >/dev/null
ab mouse move $EQ >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null; ab wait 1000 >/dev/null
ab mouse down >/dev/null; ab mouse up >/dev/null; ab wait 1000 >/dev/null
ab mouse move 600 940 >/dev/null
ab press Meta+z >/dev/null; ab wait 800 >/dev/null
ab press Meta+z >/dev/null; ab wait 1400 >/dev/null
ab record stop >/dev/null
ab close >/dev/null
$F -loglevel error -y -i "$base.mp4" -c:v libvpx-vp9 -b:v 0 -crf 30 -row-mt 1 -deadline good -cpu-used 4 "$base.webm"
$F -loglevel error -y -i "$base.mp4" -vf "fps=15,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=none" "$base.gif"
rm -f "$base.mp4"
secs=$(bin/ffprobe -v error -show_entries format=duration -of csv=p=0 "$base.webm")
rows=$(awk -v s="$secs" 'BEGIN { r = int((s * 10 + 5) / 6); print (r < 1) ? 1 : r }')
$F -loglevel error -y -i "$base.webm" -vf "fps=10,crop=300:470:0:0,tile=6x${rows}:padding=2:margin=2:color=black" -frames:v 1 "$base.filmstrip.png"
echo "$base.webm $secs s"

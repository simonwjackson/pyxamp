#!/usr/bin/env bash
# Interaction checks on the live page (index.html). Prints PASS/FAIL per check.
cd /tmp/pyxamp-mock
s="chk-$$"; pass=0; fail=0
ab() { agent-browser --session "$s" "$@"; }
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&${Q:-}" >/dev/null; ab set viewport 1600 1000 1 >/dev/null; ab reload >/dev/null; ab wait 1200 >/dev/null
sum() { ab eval "JSON.stringify(pyx.summary())" | sed 's/^"//; s/"$//; s/\\"/"/g'; }
check() { local got; got=$(sum); if echo "$got" | grep -q -- "$2"; then echo "PASS $1"; pass=$((pass+1)); else echo "FAIL $1 -> $got (want $2)"; fail=$((fail+1)); fi; }
pos() { ab eval "(() => { const h = pyx.hits().filter(h => h.id === '$1')[${2:-0}]; if (!h) return '0 0'; const k = pyx.k / devicePixelRatio; return Math.round((h.x + h.w / 2) * k) + ' ' + Math.round((h.y + h.h / 2) * k); })()" | tr -d '"'; }
click() { local p; p=$(pos "$1" "${2:-0}"); ab mouse move $p >/dev/null; ab mouse down ${3:-left} >/dev/null; ab mouse up ${3:-left} >/dev/null; ab wait 150 >/dev/null; }
key() { ab press "$1" >/dev/null; ab wait ${2:-120} >/dev/null; }
ab mouse move 900 900 >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null   # focus the page
check 'starts playing item 3 of the Pyxis queue' '"play":"play","cur":2'
key z; check 'Z: previous' '"cur":1'
key b; key b; check 'B twice: next, next' '"cur":3'
key c; check 'C: pause' '"play":"pause"'
key c; check 'C again: unpause' '"play":"play"'
key v; check 'V: stop' '"play":"stop"'
key x; check 'X: play' '"play":"play"'
key r; check 'R: repeat all -> one' '"repeat":"one"'
key r; check 'R: one -> off' '"repeat":"off"'
key r; check 'R: off -> all' '"repeat":"all"'
key s; check 'S: shuffle on' '"shuffle":true'
key s; check 'S: shuffle off' '"shuffle":false'
key Control+d; check 'Ctrl+D: doubles (k 2 -> 4)' '"dbl":true,"k":4'
key Control+d; check 'Ctrl+D again: back to k 2' '"dbl":false,"k":2'
key Control+t; check 'Ctrl+T: time remaining' '"timeMode":"remaining"'
key j; check 'J: opens the jump box' '"plView":"jump"'
key b; key r; key o; check 'typing goes into the jump box, not transport' '"jump":"bro"'
key Escape; check 'Esc: closes the jump box' '"plView":"queue"'
click eqbtn; check 'EQ button: folds the EQ section at once, no animation (H 435 -> 319 within 150 ms)' '"eqOpen":false.*"H":319'
click eqbtn; check 'EQ button again: unfolds at once (H 435)' '"eqOpen":true.*"H":435'
click plbtn; check 'PL button: folds the playlist section at once (H 232)' '"plOpen":false.*"H":232'
key Alt+e; check 'Alt+E: unfolds the playlist' '"plOpen":true.*"H":435'
key Alt+g; check 'Alt+G: folds the EQ at once' '"eqOpen":false.*"H":319'
key Alt+g
click cl-O; check 'O (clutterbar): opens the options menu' '"menu":"O"'
p=$(ab eval "(() => { const h = pyx.S._menuHits.find(m => m.it.sub === 'playon'); const k = pyx.k / devicePixelRatio; return Math.round((h.x + 10) * k) + ' ' + Math.round((h.y + 8) * k); })()" | tr -d '"'); ab mouse move $p >/dev/null; ab wait 150 >/dev/null
check 'hover Play on: the submenu opens' '"menu":"O","sub":0'
key Escape; check 'Esc: closes the submenu only' '"menu":"O","sub":null'
key Escape; check 'Esc again: closes the menu' '"menu":null'
click cl-V; check 'V (clutterbar): opens the visualizer menu' '"menu":"V"'
key Escape; check 'Esc: closes it' '"menu":null'
click eqpre; check 'PRESETS: opens the presets menu' '"menu":"presets"'
key Escape; check 'Esc: closes it' '"menu":null'
click pl-misc; check 'MISC: opens its menu' '"menu":"pl-misc"'
ab mouse move 1100 950 >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null; ab wait 120 >/dev/null; check 'a click on the desktop outside closes a menu' '"menu":null'
click row 4 right; check 'right-click a playlist row: its menu' '"menu":"row"'
key Escape; check 'Esc: closes it' '"menu":null'
key l; check 'L: library view' '"plView":"lib"'
key Enter; key Enter; ab wait 100 >/dev/null; t=$(ab eval "pyx.S.lib.map(p => p.name).join('>')" | tr -d '"'); [ "$t" = "Pyxis>Albums" ] && { echo "PASS Enter, Enter: Library > Pyxis > Albums"; pass=$((pass+1)); } || { echo "FAIL drill: $t"; fail=$((fail+1)); }
key Backspace; key Backspace; key Escape; check 'Backspace x2, Esc: back to the queue' '"plView":"queue"'
key Alt+3; check 'Alt+3: item info in the playlist section' '"info":true'
key Escape; check 'Esc: closes item info' '"info":false'
key Alt+w; check 'Alt+W: windowshade' '"shade":true'
key Alt+w; check 'Alt+W again: full size' '"shade":false'
key Meta+z; a=$(ab eval "getComputedStyle(document.getElementById('scene')).opacity + ' ' + pyx.hits().length" | tr -d '"'); check 'SUPER+Z: hides' '"shown":false'; [ "${a#* }" = 0 ] && { echo "PASS SUPER+Z: hidden at once (no hit regions on the next frame)"; pass=$((pass+1)); } || { echo "FAIL SUPER+Z left $a"; fail=$((fail+1)); }
key Meta+z; check 'SUPER+Z again: shows' '"shown":true'
click cl-A; check 'A: on the desktop' '"flash":"ON THE DESKTOP"'
errs=$(ab eval "(pyx.errors || []).length" | tr -d '"'); [ "$errs" = 0 ] && { echo "PASS no page errors"; pass=$((pass+1)); } || { echo "FAIL page errors: $errs"; fail=$((fail+1)); }
ab close >/dev/null
# spoken word (Ossicle)
s="chk2-$$"
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&state=s-podcast" >/dev/null; ab set viewport 1600 1000 1 >/dev/null; ab reload >/dev/null; ab wait 1200 >/dev/null
ab mouse move 900 900 >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null
p0=$(ab eval "Math.round(posNow(pyx.S, now()))" | tr -d '"')
key z; check 'spoken word, Z: marquee says -15 S' '"flash":"-15 S"'
p1=$(ab eval "Math.round(posNow(pyx.S, now()))" | tr -d '"'); d=$((p0 - p1)); [ $d -ge 13 ] && [ $d -le 16 ] && { echo "PASS Z moved back about 15 s ($p0 -> $p1)"; pass=$((pass+1)); } || { echo "FAIL Z moved $d s"; fail=$((fail+1)); }
key b; check 'spoken word, B: marquee says +30 S' '"flash":"+30 S"'
key r; check 'spoken word, R does nothing (repeat stays all, drawn dimmed)' '"repeat":"all"'
key s; check 'spoken word, S does nothing' '"shuffle":false'
p=$(pos bal); set -- $p; ab mouse move $1 $2 >/dev/null; ab mouse down >/dev/null; ab mouse move $(( $1 - 3 )) $2 >/dev/null; ab wait 100 >/dev/null
check 'dragging the speed slider: SPEED in the marquee' '"flash":"SPEED: 1.[0-9]X"'
ab mouse up >/dev/null
click row 2 right; ab wait 100 >/dev/null
p=$(ab eval "(() => { const h = pyx.S._menuHits.find(m => m.it.act === 'played'); const k = pyx.k / devicePixelRatio; return Math.round((h.x + 20) * k) + ' ' + Math.round((h.y + 8) * k); })()" | tr -d '"'); ab mouse move $p >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null; ab wait 100 >/dev/null
pl=$(ab eval "pyx.S.queue[2].played === true" | tr -d '"'); [ "$pl" = true ] && { echo "PASS right-click > Mark as played marks item 3 (it was not played)"; pass=$((pass+1)); } || { echo "FAIL mark as played: $pl"; fail=$((fail+1)); }
errs=$(ab eval "(pyx.errors || []).length" | tr -d '"'); [ "$errs" = 0 ] && { echo "PASS no page errors (spoken word)"; pass=$((pass+1)); } || { echo "FAIL page errors: $errs"; fail=$((fail+1)); }
ab close >/dev/null
# reduced motion: the vis area says VIS OFF until a mode is picked
s="chk3-$$"
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&rm=1" >/dev/null; ab set viewport 1600 1000 1 >/dev/null; ab reload >/dev/null; ab wait 1000 >/dev/null
v=$(ab eval "String(pyx.S.reduced && pyx.S._visOffLabel)" | tr -d '"'); [ "$v" = true ] && { echo "PASS reduced motion: vis area draws VIS OFF"; pass=$((pass+1)); } || { echo "FAIL reduced motion label: $v"; fail=$((fail+1)); }
click cl-V
p=$(ab eval "(() => { const h = pyx.S._menuHits.find(m => m.it.act === 'vis:spec'); const k = pyx.k / devicePixelRatio; return Math.round((h.x + 20) * k) + ' ' + Math.round((h.y + 8) * k); })()" | tr -d '"'); ab mouse move $p >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null; ab wait 150 >/dev/null
v=$(ab eval "String(pyx.S._visOffLabel)" | tr -d '"'); [ "$v" = false ] && { echo "PASS reduced motion: V menu > Spectrum turns the vis on, label gone"; pass=$((pass+1)); } || { echo "FAIL label still drawn: $v"; fail=$((fail+1)); }
errs=$(ab eval "(pyx.errors || []).length" | tr -d '"'); [ "$errs" = 0 ] && { echo "PASS no page errors (reduced motion)"; pass=$((pass+1)); } || { echo "FAIL page errors: $errs"; fail=$((fail+1)); }
ab close >/dev/null
# pass 2: album mode, prompt line, add to playlist
s="chk4-$$"
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&state=am-tracks" >/dev/null; ab set viewport 1600 1000 1 >/dev/null; ab reload >/dev/null; ab wait 1000 >/dev/null
ab mouse move 900 900 >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null
mclick() { local p; p=$(ab eval "(() => { const h = pyx.S._menuHits.find(m => $1); if (!h) return '0 0'; const k = pyx.k / devicePixelRatio; return Math.round((h.x + 20) * k) + ' ' + Math.round((h.y + 8) * k); })()" | tr -d '"'); ab mouse move $p >/dev/null; ab wait 120 >/dev/null; [ -n "${2:-}" ] && { ab mouse down >/dev/null; ab mouse up >/dev/null; ab wait 120 >/dev/null; }; }
key g; check 'G: album mode on' '"albumMode":true'
click pl-album; check 'title-bar record button: album mode off' '"albumMode":false'
click pl-album; check 'record button again: on' '"albumMode":true'
key ArrowDown; check 'album mode, Down: next album row' '"albumSel":1'
key Enter; check 'Enter: opens that album' '"albumOpen":1'
key Escape; check 'Esc: back to the album rows' '"albumOpen":null'
click pl-misc; mclick "m.it.act === 'prompt:savepl'" go; check 'MISC > Save queue as playlist: the prompt line opens' '"prompt":"savepl:"'
key M; key i; key x; key z; check 'typing goes into the prompt line, not to transport (Z, X ignored)' '"cur":3.*"prompt":"savepl:Mixz"'
key Backspace; check 'Backspace edits the prompt' '"prompt":"savepl:Mix"'
key Escape; check 'Esc: cancels the prompt' '"prompt":null'
click pl-misc; mclick "m.it.act === 'prompt:savepl'" go; key S; key u; key n; key Enter; check 'Enter: saves and the marquee says so' '"flash":"SAVED AS PLAYLIST: SUN"'
click eqpre; mclick "m.it.act === 'prompt:eq'" go; check 'Presets > Save as new preset: the same prompt line' '"prompt":"eq:"'
key Escape; check 'Esc closes it' '"prompt":null'
key g; click row 5 right; check 'right-click a track: its menu' '"menu":"row"'
mclick "m.it.sub === 'addpl'"; check 'hover Add to playlist: the playlists submenu opens' '"menu":"row","sub":3'
mclick "m.level === 1 && m.it.act === 'addpl:1'" go; check 'pick a playlist: added' '"flash":"ADDED TO PLAYLIST: LONG DRIVE NORTH"'
errs=$(ab eval "(pyx.errors || []).length" | tr -d '"'); [ "$errs" = 0 ] && { echo "PASS no page errors (pass 2)"; pass=$((pass+1)); } || { echo "FAIL page errors: $errs"; fail=$((fail+1)); }
ab close >/dev/null
# pass 2: position conflict and failed track
s="chk5-$$"
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&state=cf-row" >/dev/null; ab set viewport 1600 1000 1 >/dev/null; ab reload >/dev/null; ab wait 1000 >/dev/null
ab mouse move 900 900 >/dev/null; ab mouse down >/dev/null; ab mouse up >/dev/null
check 'conflict: the display says POSITION CONFLICT' '"conflict":true.*"mq":"POSITION CONFLICT"'
key ArrowRight; v=$(ab eval "pyx.S.conflict.pick" | tr -d '"'); [ "$v" = 1 ] && { echo "PASS Right: picks the second position (Pixel)"; pass=$((pass+1)); } || { echo "FAIL pick: $v"; fail=$((fail+1)); }
key Enter; check 'Enter on the conflict row: keeps Pixel 40:02, resolved' '"play":"play".*"flash":"KEPT PIXEL 40:02.*"conflict":false'
ab open "file:///tmp/pyxamp-mock/index.html?mon=2&state=ft-retry1" >/dev/null; ab wait 700 >/dev/null
check 'failed track: the display shows the first retry' '"mq":"CANNOT PLAY: RETRY 1 IN [12] S"'
ab wait 2600 >/dev/null; check 'after 2 s: the second retry, in 10 s' '"mq":"CANNOT PLAY: RETRY 2 IN'
ab wait 10000 >/dev/null; v=$(ab eval "pyx.S.cur + ' ' + pyx.S.queue[3].failed" | tr -d '"'); [ "$v" = "4 true" ] && { echo "PASS after 12 s: skipped to the next track, failed row marked red"; pass=$((pass+1)); } || { echo "FAIL skip: $v"; fail=$((fail+1)); }
errs=$(ab eval "(pyx.errors || []).length" | tr -d '"'); [ "$errs" = 0 ] && { echo "PASS no page errors (conflict, failed track)"; pass=$((pass+1)); } || { echo "FAIL page errors: $errs"; fail=$((fail+1)); }
ab close >/dev/null
echo "$pass passed, $fail failed"

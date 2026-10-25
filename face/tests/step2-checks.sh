# step2-checks.sh: the face's step-2 views and states against the seam's
# replay fixtures (docs/seam-step2.md): own playback, console mode, yield and
# hand-back, the queue and album mode, the library view, the jump box, the
# prompt line, context menus, spoken word, position conflicts, retries, the EQ
# and balance. Real keys and pointer through the headless session; real
# screenshots at 1x and 2x. Sourced by face/tools/verify and
# face/tools/verify-step2 inside a running session (session.sh, helpers.sh)
# with check/eq/pass/fail defined.

T0=1793000000000                      # the fixtures' wall-clock reference (make-fixtures)
PL_TOP=232                            # the playlist section's top with EQ open (sp)

s2shot() { # s2shot NAME [EXTRA_W]: a surface crop at the current monitor, and its describe
  local mon; mon=$(field screen)
  shot "s2-$1" "$mon"; surface_crop "s2-$1" "$1" "${2:-0}" 0
  describe > "$OUT/$1.json"
}
row_y() { echo $((PL_TOP + 20 + ${2:-0} * 13 + $1 * 13 + 6)); } # row_y INDEX [HEAD 0|1]: a row's middle (sp)
lastreq() { requests_of "$1" | jq -c '.[-1].payload // empty'; }
nreq() { requests_of "$1" | jq 'length'; }
own_requests() { requests_of 'own\..*|item\..*|queue\..*|station\..*|playlist\..*|conflict\..*|output\..*'; }
row() { describe | jq -r ".plRows[$1]"; }
cpixel() { magick "$OUT/$1.png" -format "%[hex:p{$2,$3}]" info: | cut -c1-6 | tr 'A-F' 'a-f' | sed 's/^/#/'; } # a pixel of a kept crop
focus_face() { click_sp 150 110; }

step2_checks() {
  echo "== step 2: own playback"
  focus_face
  load own-pyxis-playing; clear_commands
  check "own Pyxis: green, not following" eq "$(describe | jq -c '[.mode, .lcd, .status, .source]')" '["own","green","play","pyxis"]'
  check "own Pyxis: the marquee numbers the queue item" eq "$(field marquee)" "2. Stereolab - Miss Modular (4:51)"
  check "own Pyxis: the title names the queue" eq "$(field plTitle)" "PYXIS QUEUE"
  check "own Pyxis: the EQ works here (no note)" eq "$(field eqNote)" "null"
  s2shot own-pyxis-playing-1x
  kb key x; kb key c; kb key z; kb key b; kb key v
  check "X C Z B V send own.transport play, pause, previous, next, stop" eq "$(requests_of own.transport | jq -c '[.[].payload.action]')" '["play","pause","previous","next","stop"]'
  check "and no step-1 command" eq "$(commands)" "[]"
  clear_commands; kb key r; kb key s; kb key up; kb key right
  check "R: own.repeat off -> all" eq "$(lastreq own.repeat)" '{"mode":"all"}'
  check "S: own.shuffle on" eq "$(lastreq own.shuffle)" '{"enabled":true}'
  check "Up: own.volume +2%" eq "$(lastreq own.volume)" '{"value":0.82}'
  check "Right: own.seek +5 s fenced by the occurrence" eq "$(lastreq own.seek | jq -c '[.occurrence, (.positionMs > 65000 and .positionMs < 80000)]')" '["occ-2",true]'
  clear_commands; click_sp 100 77
  check "a click on the position bar seeks own playback" eq "$(lastreq own.seek | jq -c '[.occurrence, (.positionMs > 90000 and .positionMs < 97000)]')" '["occ-2",true]'
  clear_commands; click_sp 50 97; click_sp 119 97
  check "click Play and Next: own.transport" eq "$(requests_of own.transport | jq -c '[.[].payload.action]')" '["play","next"]'

  echo "== step 2: balance and EQ go to the engine"
  clear_commands; click_sp 205 62; sleep 0.4
  bal=$(lastreq balance.set | jq '.value')
  check "balance: a click right of centre sends balance.set ($bal)" test -n "$(echo "$bal" | grep -E '^0\.(7[5-9]|8[0-9])$')"
  check "balance: kept in the preferences" eq "$(field balance)" "$bal"
  check "balance: the display says how much" eq "$(field marquee)" "BALANCE: $(jq -n "$bal * 100 | round")% RIGHT"
  click_sp 196 62; sleep 0.3
  check "balance snaps to the centre" eq "$(lastreq balance.set)" '{"value":0}'
  clear_commands; click_sp 85 161; sleep 0.4
  check "EQ: a band slider sends eq.set with ten bands" eq "$(lastreq eq.set | jq -c '[.on, .bandsDb[0], (.bandsDb | length), .preampDb]')" '[true,11,10,0]'
  s2shot own-eq-band-1x
  clear_commands; click_sp 238 140; sleep 0.4; kb key down; kb key enter; sleep 0.4      # PRESETS > Flat
  check "PRESETS > Flat sends a flat eq.set" eq "$(lastreq eq.set | jq -c '.bandsDb')" '[0,0,0,0,0,0,0,0,0,0]'

  echo "== step 2: Play on (Pyxis outputs)"
  # The pointer hovers Play on and leaves the menus (a late pointer event then hovers
  # nothing); the keys walk the submenu: Right opens it on This computer.
  clear_commands; click_sp 10 8; at_sp 40 25; sleep 0.4; at_sp 262 300; sleep 0.3; kb key right; sleep 0.2
  check "O menu > Play on lists the outputs" eq "$(field menu)" "main"
  s2shot play-on-menu-1x 160
  kb key down; kb key enter; sleep 0.3   # Down: Living Room
  check "Play on > Living Room sends output.select" eq "$(lastreq output.select)" '{"source":"pyxis","output":"sonos-living-room"}'
  check "the display says where it plays" eq "$(field marquee)" "PLAYS ON LIVING ROOM"

  echo "== step 2: console mode"
  sleep 1.6   # the Play on confirmation goes
  load own-pyxis-console
  check "console: green, the title starts with the device" eq "$(describe | jq -c '[.lcd, .console, .marquee]')" '["green",true,"Living Room: 1. Stereolab - Brakhage (4:30)"]'
  check "console: a flat line in the visualizer" eq "$(field visual)" "flat"
  check "console: NO EQ ON LIVING ROOM" eq "$(field eqNote)" "NO EQ ON LIVING ROOM"
  sleep 2.2; s2shot own-console-1x
  check "console: the flat line is drawn" eq "$(cpixel own-console-1x 50 51)" "#b0c8b0"

  echo "== step 2: yield and hand-back"
  load_at own-yielded $((T0 - 86000))
  check "yielded: amber, following Firefox" eq "$(describe | jq -c '[.lcd, .mode]')" '["amber","following"]'
  check "yielded: the tail states the resume rule" test -n "$(field marquee | grep -E 'PYXAMP PAUSED, RESUMES IF THIS STOPS BY 1:2[56]$')"
  check "yielded: own playback's row says paused" eq "$(row 1)" "2. Stereolab - Miss Modular | paused"
  check "yielded: NO EQ ON FIREFOX" eq "$(field eqNote)" "NO EQ ON FIREFOX"
  clear_commands; kb key x
  check "yielded: X goes to the followed player (step-1 command)" eq "$(commands | jq -c '[.[].tag, .[].bus]')" '["transport.play","org.mpris.MediaPlayer2.firefox.instance_1_42"]'
  check "yielded: and not to own playback" eq "$(nreq own.transport)" "0"
  s2shot own-yielded-1x
  load own-resumed
  check "hand-back: BACK TO PYXAMP: RESUMING, green" eq "$(describe | jq -c '[.marquee, .lcd]')" '["BACK TO PYXAMP: RESUMING","green"]'
  s2shot own-handback-1x
  sleep 2.2
  check "hand-back: gone after 2 s" eq "$(field marquee)" "2. Stereolab - Miss Modular (4:51)"
  load own-yield-stays
  check "a long interruption: STAYS PAUSED" test -n "$(field marquee | grep -E '\*\*\*  PYXAMP STAYS PAUSED$')"

  echo "== step 2: queue rows and context menus"
  load library-mixed; clear_commands
  click_sp 100 "$(row_y 1)"
  check "a click selects a queue row" eq "$(field sel)" "occ-2"
  kb key enter
  check "Enter plays it: queue.jump" eq "$(lastreq queue.jump)" '{"occurrence":"occ-2"}'
  kb key delete
  check "Del removes it: queue.remove" eq "$(lastreq queue.remove)" '{"occurrence":"occ-2"}'
  click_sp 100 "$(row_y 0)" rclick; sleep 0.4
  check "right click on a Pyxis row opens its menu" eq "$(describe | jq -c '[.menu, .menuItems]')" '["row",["Play","Add to playlist","Start station","Item info","Remove"]]'
  check "it asks Pyxis for its playlists" eq "$(lastreq library.open)" '{"source":"pyxis","node":"playlists"}'
  kb key down; kb key down; kb key right; sleep 0.4
  s2shot row-add-to-playlist-1x 200
  kb key enter; sleep 0.4
  check "Add to playlist > Long drive north: playlist.add" eq "$(lastreq playlist.add)" '{"source":"pyxis","playlist":"playlist:long-drive","item":"track:Dots and Loops:1"}'
  check "the display confirms it" eq "$(field marquee)" "ADDED TO PLAYLIST: LONG DRIVE NORTH"
  click_sp 100 "$(row_y 0)" rclick; sleep 0.3; kb key down; kb key down; kb key down; kb key enter; sleep 0.4
  check "Start station from a song: station.start with the song as seed" eq "$(lastreq station.start)" '{"source":"pyxis","seed":"track:Dots and Loops:1"}'
  check "the display names the seed" eq "$(field marquee)" "STATION FROM BRAKHAGE (TRACK)"

  echo "== step 2: album mode"
  focus_face; kb key g
  check "G: album mode, titled by source" eq "$(describe | jq -c '[.albumMode, .plTitle]')" '[true,"PYXIS QUEUE BY ALBUM"]'
  check "album rows: the playing album says now 4/4" eq "$(row 0)" "1. Stereolab - Dots and Loops (1997) | now 4/4  19:16"
  check "a single track stays a track row" eq "$(row 1)" "2. Talk Talk - I Believe in You | 6:20"
  check "a partial run counts its tracks" eq "$(row 2)" "3. Stereolab - Emperor Tomato Ketchup (1996) | 3 tracks  13:49"
  s2shot album-mode-1x
  kb key down; kb key down
  check "Down moves the album selection (not volume)" eq "$(describe | jq -c '[.plSel, (.volume|tostring)]')" '[2,"0.8"]'
  kb key enter
  check "Enter opens the album's tracks" eq "$(describe | jq -c '[.albumOpen, .plHead]')" '["occ-6","< Stereolab - Emperor Tomato Ketchup (1996) | 3 tracks"]'
  s2shot album-open-1x
  clear_commands; kb key down; kb key enter
  check "Enter on a track there plays it" eq "$(lastreq queue.jump)" '{"occurrence":"occ-7"}'
  kb key escape
  check "Esc goes back to the albums" eq "$(describe | jq -c '[.albumOpen, .plSel]')" '["",2]'
  click_sp 247 $((PL_TOP + 10))
  check "the title-bar button turns album mode off" eq "$(field albumMode)" "false"

  echo "== step 2: library view"
  kb key l
  check "L opens the library: every source with its status" eq "$(describe | jq -c '[.plTitle, .plRows]')" '["LIBRARY",["Pyxis | 10 groups >","Ossicle | connecting","Jellyfin | 4 groups >"]]'
  s2shot library-root-1x
  kb key down; kb key enter
  check "a connecting source says so and stays" eq "$(describe | jq -c '[.marquee, .libPath]')" '["OSSICLE IS STILL CONNECTING",[]]'
  kb key up; kb key enter
  check "Enter: Pyxis's groups exactly as reported" eq "$(describe | jq -c '[.plHead, .plCount, .plRows[0], .plRows[5], .plRows[9]]')" '["< Library > Pyxis",10,"Discovery | 37 >","Tag: shoegaze | 23 >","Playlists | 3 >"]'
  s2shot library-groups-1x
  ctl '{"pageDelayMs":1500}' >/dev/null
  clear_commands; kb key down; kb key enter; sleep 0.4
  check "Enter on Collection: library.open; Loading... until the page comes" eq "$(describe | jq -c '[.plRows[0]]')$(lastreq library.open)" '["Loading..."]{"source":"pyxis","node":"placement:collection"}'
  s2shot library-loading-1x
  ctl '{"pageDelayMs":0}' >/dev/null; sleep 1.6
  check "the page's albums are drawn; the count stays unknown when absent" eq "$(describe | jq -c '[.plHead, .plRows[0], .plRows[7]]')" '["< Library > Pyxis > Collection | 412","Stereolab - Dots and Loops (1997) | 10 >","Unknown artist - Untitled | >"]'
  check "the More row in view loads the next page with its cursor" eq "$(lastreq library.open)" '{"source":"pyxis","node":"placement:collection","cursor":"collection-2"}'
  check "and appends it" eq "$(describe | jq -c '[.plCount, .plRows[9]]')" '[10,"Slowdive - Souvlaki (1993) | 10 >"]'
  s2shot library-albums-1x
  kb key enter; sleep 0.4
  check "Enter on an album opens it" eq "$(describe | jq -c '[.plHead, .plRows[0]]')" '["< Library > Pyxis > Collection > Dots and Loops","1. Brakhage | 4:30"]'
  clear_commands; kb key enter; kb key down; kb key insert
  check "Enter plays a track; Ins queues one" eq "$(requests_of item.play | jq -c '[.[].payload | .item + " " + .mode]')" '["track:Dots and Loops:1 now","track:Dots and Loops:2 last"]'
  click_sp 100 "$(row_y 2 1)" rclick; sleep 0.4
  check "right click on a library track: play, queue, add to playlist, station" eq "$(describe | jq -c .menuItems)" '["Play","Play next","Queue","Add to playlist","Start station"]'
  kb key escape
  kb key backspace; kb key backspace; kb key backspace
  check "Backspace climbs back to the root" eq "$(describe | jq -c '[.libPath, .plSel]')" '[[],0]'
  kb key down; kb key down; kb key enter; kb key down; kb key down; kb key down; kb key enter; sleep 0.4
  check "Jellyfin > Videos: the hand-off rows" eq "$(describe | jq -c '[.plHead, .plRows[0]]')" '["< Library > Jellyfin > Videos | 128","Alien (1979) | 1:57:00"]'
  s2shot library-videos-1x
  kb key escape; kb key escape; kb key escape
  check "Esc at the root goes back to the queue" eq "$(field view)" "queue"
  click_sp 147 97
  check "eject opens the library" eq "$(field view)" "library"
  kb key escape

  echo "== step 2: jump box"
  clear_commands; kb key j; kb type stereo; sleep 0.6
  check "J: typing searches every source" eq "$(lastreq search)" '{"query":"stereo"}'
  check "results carry their source tags" eq "$(describe | jq -c '[.plHead, [.plRows[] | split(" ")[0]]]')" '["Jump: stereo_ | 6 in 3 sources",["PYX","PYX","PYX","OSS","JF","JF"]]'
  check "no key reached transport or own playback" eq "$(own_requests)" "[]"
  kb key down; kb key down; kb key down
  check "a result from another source says which pauses" eq "$(field plFoot)" "Enter: play from Ossicle; Pyxis pauses"
  s2shot jump-results-1x
  kb key enter; sleep 0.3
  check "Enter plays it from that source" eq "$(lastreq item.play)" '{"source":"ossicle","item":"episode:21","mode":"now"}'
  check "the display says what paused" eq "$(field marquee)" "PYXIS PAUSED. PLAYING FROM OSSICLE"
  check "and the jump box closes" eq "$(field view)" "queue"
  kb key j; kb type zzq; sleep 0.6
  check "nothing found says so; Jellyfin's state is told" eq "$(describe | jq -c '[.plRows[0], .plHead]')" '["Nothing found for \"zzq\".","Jump: zzq_ | 0 found"]'
  kb key escape

  echo "== step 2: prompt line"
  clear_commands; click_sp 113 414; sleep 0.4; kb key down; kb key down; kb key down; kb key enter; sleep 0.3
  check "MISC > Save queue as playlist opens the prompt line" eq "$(describe | jq -c '[.plTitle, .plHead, .plFoot]')" '["SAVE AS PLAYLIST","Name: _ | 8 tracks","Enter: save in Pyxis   Esc: cancel"]'
  kb type "sunday xzcvb"; kb key left; kb key up; kb key delete
  check "typed letters and arrows stay in the prompt" eq "$(describe | jq -c '[.prompt, .volume]')" '["sunday xzcvb",0.8]'
  check "nothing reached transport, seek or volume" eq "$(describe >/dev/null; own_requests)$(commands)" "[][]"
  s2shot prompt-save-playlist-1x
  kb key backspace; kb key enter; sleep 0.3
  check "Enter saves: playlist.save" eq "$(lastreq playlist.save)" '{"source":"pyxis","name":"sunday xzcv"}'
  check "and confirms" eq "$(field marquee)" "SAVED AS PLAYLIST: SUNDAY XZCV"
  click_sp 113 414; sleep 0.4; kb key down; kb key down; kb key down; kb key enter; sleep 0.4; kb key enter; sleep 0.3
  check "an empty name saves nothing" eq "$(describe | jq -c '[.marquee, .prompt]')" '["NO NAME: NOTHING SAVED",null]'
  kb chord alt+e
  click_sp 238 140; sleep 0.4; for _ in 1 2 3 4 5; do kb key down; done; kb key enter; sleep 0.3
  check "PRESETS > Save as new preset unfolds the playlist for its prompt" eq "$(describe | jq -c '[.plOpen, .plTitle]')" '[true,"SAVE EQ PRESET"]'
  s2shot prompt-eq-preset-1x
  kb type mine; kb key enter; sleep 0.3
  check "it saves the preset and folds again" eq "$(describe | jq -c '[.plOpen, .marquee]')" '[false,"EQ PRESET SAVED: MINE"]'
  kb chord alt+e

  echo "== step 2: item info"
  kb key i
  check "I: item info of own playback" eq "$(describe | jq -c '[.plTitle, .plRows[0], .plRows[1]]')" '["ITEM INFO","Pyxis (host)","This computer (here)"]'
  s2shot item-info-own-1x
  kb key escape

  echo "== step 2: spoken word"
  load own-ossicle-playing; clear_commands
  check "spoken word: speed and skip intervals shown" eq "$(describe | jq -c '[.spoken, .speed, .plTitle]')" '[true,1.5,"OSSICLE UP NEXT"]'
  check "played dimmed with a tick; part-played shows the time left" eq "$(describe | jq -c '[.plRows[0], .plRows[3]]')" '["1. The Allusionist - The cassette comeback | played","4. Dune - Frank Herbert | -10:05:00"]'
  s2shot spoken-word-1x
  kb key z
  check "Z skips back 15 s (own.seek) and says -15 S" eq "$(lastreq own.seek | jq -c '[.occurrence, (.positionMs > 620000 and .positionMs < 640000)]')$(field marquee)" '["occ-2",true]-15 S'
  kb key b
  check "B skips forward 30 s" eq "$(lastreq own.seek | jq -c '(.positionMs > 665000 and .positionMs < 690000)')$(field marquee)" 'true+30 S'
  clear_commands; kb key r; kb key s
  check "R and S do nothing for spoken word" eq "$(requests_of 'own.repeat|own.shuffle')" "[]"
  check "and say why" eq "$(field marquee)" "SHUFFLE: Shuffle is not for spoken word"
  click_sp 194 62; sleep 0.4
  check "the speed slider sends own.speed in 0.1 steps" eq "$(lastreq own.speed)" '{"rate":1.6}'
  check "and the display says SPEED" eq "$(field marquee)" "SPEED: 1.6X"
  click_sp 100 "$(row_y 2)" rclick; sleep 0.3
  check "right click on an episode: mark as played" eq "$(describe | jq -c .menuItems)" '["Play","Mark as played","Mark as unplayed (off)","Item info","Remove from up next"]'
  kb key down; kb key down; kb key enter; sleep 0.3
  check "Mark as played: item.markPlayed" eq "$(lastreq item.markPlayed)" '{"source":"ossicle","item":"episode:13","played":true}'
  kb key g
  check "album mode groups up next by show; a book is one row" eq "$(describe | jq -c '[.plTitle, .plRows]')" '["OSSICLE UP NEXT BY SHOW",["1. The Allusionist | 2 episodes, 1 played  1:06:10","2. Song Exploder - Mixtapes | 25:00","3. Dune - Frank Herbert | book  -10:05:00","4. The Allusionist - Old radio | 32:00"]]'
  s2shot spoken-word-by-show-1x
  kb key g

  echo "== step 2: position conflict"
  load own-ossicle-conflict; clear_commands
  check "conflict: POSITION CONFLICT, no clock" eq "$(describe | jq -c '[.marquee, .clock]')" '["POSITION CONFLICT",""]'
  check "conflict: one row with both positions and devices" eq "$(row 1)" "2. The Allusionist - Liner notes [Myoko 10:40] [Pixel 20:10]"
  kb key right
  check "Right picks the other position and does not seek" eq "$(own_requests)" "[]"
  s2shot conflict-1x
  kb key enter; sleep 0.3
  check "Enter keeps it: conflict.resolve" eq "$(lastreq conflict.resolve)" '{"source":"ossicle","conflict":"conflict-1","keep":"phone"}'
  check "and confirms" eq "$(field marquee)" "KEPT PIXEL 20:10 (SYNCED)"

  echo "== step 2: retries and failures"
  sleep 2.1   # the last confirmation goes
  load_at own-retry-first "$T0"
  check "first failure: CANNOT PLAY: RETRY 1 IN 2 S, red light" eq "$(describe | jq -c '[.marquee, .status]')" '["CANNOT PLAY: RETRY 1 IN 2 S","error"]'
  check "no clock, no position while it waits" eq "$(describe | jq -c '[.clock, .positionMs]')" '["",null]'
  s2shot retry-first-1x
  load_at own-retry-failed $((T0 - 2000))
  check "the track given up: SKIPPED: TRACK CANNOT PLAY" eq "$(field marquee)" "SKIPPED: TRACK CANNOT PLAY"
  sleep 2.1
  check "the next one counts down from 10 s" test -n "$(field marquee | grep -E '^CANNOT PLAY: RETRY 2 IN (10|9|8) S$')"
  check "the failed row is red, the retrying one says so" eq "$(describe | jq -c '[.plRows[1], .plRows[2]]')" '["2. Stereolab - Miss Modular | cannot play","3. Stereolab - The Flower Called Nowhere | retrying"]'
  s2shot retry-failed-1x
  sleep 2.1
  load own-engine-unavailable
  check "engine unavailable: in the title, red light" eq "$(describe | jq -c '[.marquee, .status]')" '["PLAYBACK UNAVAILABLE: mpv is not installed","error"]'

  echo "== step 2: stress"
  load own-pyxis-long
  check "240 items: the current one (120) is scrolled into view" test "$(describe | jq '.plCount')" -eq 240
  s2shot long-queue-1x
  load library-sources; kb key l
  check "an unreachable source is marked" eq "$(row 2)" "Jellyfin | unreachable"
  s2shot library-unreachable-1x
  kb key escape
  check "nothing plays with sources: says what to do" eq "$(field marquee)" "PYXAMP  ***  NOTHING PLAYS. PRESS J OR L TO FIND SOMETHING"

  echo "== step 2: 2x"
  ipc toggle; sleep 0.3; focus_monitor "$MON2"; ipc toggle; sleep 0.6
  focus_face
  for f in own-pyxis-playing own-pyxis-console own-ossicle-playing own-ossicle-conflict own-retry-failed; do load "$f"; sleep 0.3; s2shot "$f-2x"; done
  load_at own-yielded $((T0 - 86000)); s2shot own-yielded-2x
  load library-mixed; kb key g; s2shot album-mode-2x; kb key g
  kb key l; kb key up; kb key up; kb key enter; s2shot library-groups-2x
  kb key down; kb key enter; sleep 0.4; s2shot library-albums-2x
  kb key escape; kb key escape; kb key escape; kb key escape
  kb key j; kb type stereo; sleep 0.6; s2shot jump-results-2x; kb key escape
  click_sp 100 "$(row_y 0)" rclick; sleep 0.4; kb key down; kb key down; kb key right; sleep 0.4; s2shot row-add-to-playlist-2x 200; kb key escape; kb key escape
  click_sp 113 414; sleep 0.4; kb key down; kb key down; kb key down; kb key enter; sleep 0.4; kb type sunday; s2shot prompt-save-playlist-2x; kb key escape
  check "2x: k = 2" eq "$(field k)" "2"
  magick "$OUT/library-groups-2x.png" -sample 50% "$T/s2down.png"
  check "2x: the library view is the 1x one scaled by exactly 2 (playlist list)" eq \
    "$(magick compare -metric AE "$T/s2down.png[243x145+12+252]" "$OUT/library-groups-1x.png[243x145+12+252]" null: 2>&1 | cut -d' ' -f1)" "0"
  ipc toggle; sleep 0.3; focus_monitor "$MON1"; ipc toggle; sleep 0.6
}

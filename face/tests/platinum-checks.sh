# platinum-checks.sh: the Platinum face in the headless session. Which face the
# desktop theme picks (live, from the same files the desktop reads), the
# step-1 checks of face/tools/verify at the Platinum face's own targets (keys,
# pointer, folds and their 17-sp headers, windowshade, the zoom, collapse and
# close boxes, menus, layer, Super+Z, 2x), drags mapped from each control's
# own rectangle, the three appearances, and a theme switch that keeps state
# and playback. Sourced by face/tools/verify-platinum inside a running session
# (session.sh, helpers.sh, faces.sh) with check/eq/pass/fail defined.

pshot() { # pshot NAME [EXTRA_W]: a surface crop at the current monitor, and its describe
  local mon; mon=$(field screen)
  shot "pt-$1" "$mon"; surface_crop "pt-$1" "$1" "${2:-0}" "${3:-0}"
  describe > "$OUT/$1.json"
}
# The virtual pointer's motion reaches Qt one event late: a second small move delivers the first.
hover_sp() { at_sp "$1" "$2"; sleep 0.1; at_sp "$(($1 + 1))" "$2"; sleep 0.3; }
menu_closed() { [ "$(field menu)" = "" ] || { kb key escape; sleep 0.2; }; [ "$(field menu)" = "" ] || kb key escape; }
near() { jq -n --argjson v "$1" --argjson lo "$2" --argjson hi "$3" '$v >= $lo and $v <= $hi' | grep -qx true; }

platinum_checks() {
  echo "== Platinum face: the desktop theme picks it"
  check "theme platinum: the Platinum face, light" eq "$(describe | jq -c '[.themeName, .face, .appearance]')" '["platinum","platinum","light"]'
  check "1x: k = 1, surface 300x515 (main 150, EQ 122, playlist 82 + 16 x 10, frame 1)" eq "$(geometry)" "16 16 300 515"

  echo "== idle"
  check "idle: own state, not amber" eq "$(field lcd)" "green"
  pshot idle-1x

  echo "== following: keys (the same commands as the Winamp face)"
  load following-full
  check "following: amber" eq "$(field lcd)" "amber"
  clear_commands; click_pt focus
  kb key x; kb key c; kb key z; kb key b; kb key v
  check "X C Z B V send play, pause, previous, next, stop to mpv" eq "$(commands | jq -c '[.[].tag]')" '["transport.play","transport.pause","transport.previous","transport.next","transport.stop"]'
  clear_commands; kb key r; kb key s; kb key up; kb key right
  check "R cycles repeat all -> one" eq "$(commands | jq -c '.[0].payload')" '{"mode":"one"}'
  check "S turns shuffle off" eq "$(commands | jq -c '.[1].payload')" '{"enabled":false}'
  check "Up sets the system volume +2%" eq "$(requests_of 'system\.volume|own\.volume' | jq -c '[.[] | [.tag, .payload.value]]')" '[["system.volume",0.62]]'
  check "Right seeks +5 s with the track id" eq "$(commands | jq -c '.[2] | [.tag, .payload.trackId, (.payload.positionMs > 85000 and .payload.positionMs < 100000)]')" '["transport.seek","/org/mpris/MediaPlayer2/mpv/track/3",true]'

  echo "== following: pointer, at the Platinum targets"
  clear_commands; click_pt play; click_pt next
  check "click Play and Next" eq "$(commands | jq -c '[.[].tag]')" '["transport.play","transport.next"]'
  clear_commands; click_sp 41 136; click_sp 114 136
  check "click Shuffle and Repeat (sticky buttons)" eq "$(commands | jq -c '[.[].tag]')" '["shuffle.set","repeat.set"]'

  echo "== drags map from each control's own rectangle (lib/drag.js, lib/sliders.js)"
  # The volume target is 224..294 sp; the thumb's centre travels 7..62. A click at
  # 244 sp (20.5 sp in) is 25 %. The old Winamp mapping, (x + mx - 114) / 54, reads 100 % here.
  clear_commands; click_sp 244 105; sleep 0.4
  check "a click at 25 % of the volume slider sends 0.25 (old mapping: 1.0)" eq "$(requests_of system.volume | jq -c '[.[].payload.value] | unique')" '[0.25]'
  clear_commands; at_sp 244 105; ptr down; for x in 252 262 268 272 272; do at_sp "$x" 105; sleep 0.08; done; sleep 0.2; ptr up; sleep 0.4
  v=$(requests_of system.volume | jq '.[-1].payload.value')
  check "dragging the volume thumb to 75 % sends about 0.75 ($v)" near "$v" 0.72 0.78
  clear_commands; click_sp 177 91; sleep 0.3
  p=$(commands | jq '.[-1].payload.positionMs')
  check "a click at 60 % of the seek slider seeks to 60 % of 2:51 ($p ms)" near "$p" 101000 105000
  load own-pyxis-playing; sleep 0.3; clear_commands
  click_pt balance_right; sleep 0.4
  b=$(requests_of balance.set | jq '.[-1].payload.value')
  check "balance: a click right of centre, as on the Winamp face ($b)" near "$b" 0.75 0.89
  click_pt balance_centre; sleep 0.4
  check "balance snaps to the centre" eq "$(requests_of balance.set | jq -c '.[-1].payload')" '{"value":0}'
  clear_commands; click_sp 69 $((150 + 41 + 3 + 15)); sleep 0.4
  check "EQ: a click at the 60 Hz slider's +6 dB tick sends +6" eq "$(requests_of eq.set | jq -c '.[-1].payload.bandsDb[0]')" "6"
  clear_commands; click_sp 69 $((150 + 41 + 3 + 60)); sleep 0.4
  check "EQ: a click at its bottom sends -12" eq "$(requests_of eq.set | jq -c '.[-1].payload.bandsDb[0]')" "-12"
  click_pt presets; sleep 0.4; kb key down; kb key enter; sleep 0.4; menu_closed
  check "PRESETS > Flat" eq "$(requests_of eq.set | jq -c '.[-1].payload.bandsDb')" '[0,0,0,0,0,0,0,0,0,0]'
  load following-full; sleep 0.3

  echo "== capability loss"
  load following-limited
  clear_commands; click_pt focus; kb key b; kb key z; kb key right; click_pt next
  check "unsupported next, previous, seek send nothing" eq "$(commands)" "[]"
  kb key b; sleep 0.2
  check "the display says why (the model's words; the LCD writes them in sentence case)" eq "$(field marquee)" "NEXT: Chromium does not offer this"
  pshot capability-loss-1x
  at_sp 150 113; sleep 0.1; at_sp 151 113; sleep 1.2; pshot balloon-dimmed-next-1x 220 40

  echo "== folds keep their 17-sp header; windowshade; the boxes"
  load following-full; click_pt focus
  click_sp 20 158
  check "the EQ header folds the EQ: 105 sp shorter, the header stays" eq "$(describe | jq -c '[.eqOpen]')|$(geometry)" '[false]|16 16 300 410'
  click_sp 20 175
  check "the playlist header folds the playlist (the header stays)" eq "$(describe | jq -c '[.plOpen]')|$(geometry)" '[false]|16 16 300 185'
  pshot folded-1x
  click_sp 287 8
  check "the collapse box is windowshade" eq "$(describe | jq -c '[.shade]')|$(geometry)" '[true]|16 16 300 53'
  pshot shade-folded-1x
  kb chord alt+w; kb chord alt+g; kb chord alt+e
  check "Alt+W, Alt+G, Alt+E: back to 515" eq "$(geometry)" "16 16 300 515"
  kb chord alt+w
  check "windowshade with EQ and playlist open: 18 + 122 + 242 + 1" eq "$(geometry)" "16 16 300 383"
  pshot shade-1x
  kb chord alt+w
  click_sp 273 8
  check "the zoom box is Ctrl+D: k = 2" eq "$(describe | jq -c '[.dbl, .k, .physW, .physH]')" '[true,2,600,1030]'
  click_sp 273 8
  check "the zoom box again: k = 1" eq "$(describe | jq -c '[.dbl, .k]')|$(geometry)" '[false,1]|16 16 300 515'
  kb chord ctrl+d
  check "Ctrl+D: the same" eq "$(describe | jq -c '[.k, .physW, .physH]')" "[2,600,1030]"
  pshot following-ctrl-d-1x-monitor
  kb chord ctrl+d

  echo "== menus: the desktop's Platinum menus, the same items"
  click_pt menu; sleep 0.4
  check "O opens the main menu" eq "$(field menu)" "main"
  check "with the Winamp face's items" eq "$(describe | jq -c '.menuItems[0:4]')" '["Play on (off)","Time elapsed","Time remaining","Repeat"]'
  pshot main-menu-1x 160 300
  kb key escape; sleep 0.2
  check "Esc closes it" eq "$(field menu)" ""
  kb chord ctrl+d; sleep 0.3; click_sp 14 29; sleep 0.5
  pshot main-menu-ctrl-d 160 300
  menu_closed; kb chord ctrl+d
  click_sp 14 73; sleep 0.4
  check "V opens the visualizer menu" eq "$(field menu)" "vis"
  pshot vis-menu-1x 160 100
  menu_closed
  # Repeat is the 4th item: 26-px rows and 11-px separators, under the O button's bottom (y 35).
  clear_commands; click_pt menu; hover_sp 47 153; at_sp 200 160; sleep 0.3; pshot repeat-submenu-1x 160 300
  # The submenu opens beside Repeat, 4 px over the menu's right edge; One track is its 3rd row.
  click_sp 268 205
  check "O menu > Repeat > One track (pointer, then the submenu by keys)" eq "$(commands | jq -c '[.[] | select(.tag == "repeat.set") | .payload.mode]')" '["one"]'
  menu_closed

  echo "== layer, close box, Super+Z"
  click_sp 14 40
  check "A puts Pyxamp on the desktop (layer bottom)" eq "$(layer_level)" "$MON1 1"
  click_sp 14 40
  check "A again: on top" eq "$(layer_level)" "$MON1 2"
  click_sp 10 8
  check "the close box hides Pyxamp" eq "$(geometry)" ""
  ipc toggle; sleep 0.6
  check "Super+Z brings it back where it was" eq "$(geometry)" "16 16 300 515"

  echo "== 2x"
  ipc toggle; sleep 0.3; focus_monitor "$MON2"; ipc toggle; sleep 0.6
  check "2x: k = 2 at scale 2, 300x515 logical" eq "$(describe | jq -c '[.k, .physW, .physH, .logicalW, .logicalH]')" "[2,600,1030,300,515]"
  load following-paused; click_pt focus
  # O menu > Reduced motion: 15 rows and 4 separators under the O button (y 35 + 5 + 393 + 13).
  click_pt menu; sleep 0.3; click_sp 47 446; sleep 0.3; menu_closed
  check "O menu > Reduced motion" eq "$(field reducedMotion)" "true"
  pshot paused-reduced-motion-2x
  ipc toggle; sleep 0.3; focus_monitor "$MON1"; ipc toggle; sleep 0.6
  pshot paused-reduced-motion-1x
  magick "$OUT/paused-reduced-motion-2x.png" -sample 50% "$T/pdown.png"
  # The display holds only whole-sp art and 1-bit LCD type; chrome text is smooth at 12k px by design.
  d=$(magick compare -metric AE "$T/pdown.png[269x57+24+23]" "$OUT/paused-reduced-motion-1x.png[269x57+24+23]" null: 2>&1 | cut -d' ' -f1)
  check "2x: the display is the 1x display scaled by exactly 2 ($d px differ)" eq "$d" "0"
  click_pt menu; sleep 0.3; click_sp 47 446; sleep 0.3; menu_closed
  check "reduced motion off" eq "$(field reducedMotion)" "false"

  echo "== appearances follow the desktop's theme and theme-extra files, live"
  load own-pyxis-playing; sleep 0.3
  desktop_theme platinum dark off; sleep 2.6
  check "dark: dark grey" eq "$(field appearance)" "grey"
  shot app-grey "$MON1"; check "dark grey face #3a3a3a" eq "$(pixel app-grey $((16 + 160)) $((16 + 136)))" "#3a3a3a"
  check "dark grey: black LCD, green ink, own audio" eq "$(pixel app-grey $((16 + 160)) $((16 + 72)))" "#0a0e09"
  desktop_theme platinum dark on; sleep 2.6
  check "dark + extra: extra dark" eq "$(field appearance)" "black"
  shot app-black "$MON1"; check "extra dark face #000000 (flat)" eq "$(pixel app-black $((16 + 160)) $((16 + 136)))" "#000000"
  desktop_theme platinum light on; sleep 2.6
  check "light + extra: light (extra dark is for dark mode only)" eq "$(field appearance)" "light"
  shot app-light "$MON1"; check "light face #dddddd, sage LCD" eq "$(pixel app-light $((16 + 160)) $((16 + 136)))$(pixel app-light $((16 + 160)) $((16 + 72)))" "#dddddd#b4c0a2"
  load following-full; sleep 0.5; shot app-amber "$MON1"
  check "following: amber backlight" eq "$(pixel app-amber $((16 + 160)) $((16 + 72)))" "#e9b24a"
  desktop_theme platinum light off; sleep 2.6

  echo "== the face rule, live (lib/desktop-theme.js)"
  desktop_theme solaris light off; sleep 2.6
  check "a name not in the registry resolves to platinum" eq "$(describe | jq -c '[.themeName, .face]')" '["platinum","platinum"]'
  desktop_theme win98 light off; sleep 2.6
  check "win98: its pyxampFace, winamp" eq "$(describe | jq -c '[.face, .physW]')" '["winamp",275]'
  jq '[.[] | del(.pyxampFace)]' "$PYXAMP_DESKTOP_THEMES" > "$T/themes.new" && mv -f "$T/themes.new" "$PYXAMP_DESKTOP_THEMES"; sleep 2.6
  check "win98 with no pyxampFace in the registry: the Platinum face" eq "$(field face)" "platinum"
  mv -f "$PYXAMP_DESKTOP_THEMES" "$T/themes.off"; sleep 2.6
  check "no registry at all: the id decides, win98 is the Winamp face" eq "$(field face)" "winamp"
  write_registry "$PYXAMP_DESKTOP_THEMES"; desktop_theme platinum light off; sleep 2.6
  check "back to platinum" eq "$(field face)" "platinum"
}

# A theme switch while playing: before, during (win98) and after, with the
# position and the view state kept and no request sent.
theme_swap_check() {
  echo "== a desktop theme switch keeps state and playback"
  load own-pyxis-playing; click_pt focus; click_row 3; sleep 0.3; kb chord alt+g
  clear_commands
  local before during after
  pshot swap-1-platinum-before
  before=$(describe | jq -c '{face, positionMs, view, sel, eqOpen, plOpen, dbl, shade, status, marquee: .marquee, source}')
  desktop_theme win98 light off; sleep 2.6
  pshot swap-2-winamp-during
  during=$(describe | jq -c '{face, positionMs, view, sel, eqOpen, plOpen, dbl, shade, status, marquee: .marquee, source}')
  desktop_theme platinum light off; sleep 2.6
  pshot swap-3-platinum-after
  after=$(describe | jq -c '{face, positionMs, view, sel, eqOpen, plOpen, dbl, shade, status, marquee: .marquee, source}')
  printf 'before %s\nduring %s\nafter  %s\nrequests %s\ncommands %s\n' "$before" "$during" "$after" "$(requests)" "$(commands)" > "$OUT/theme-swap.txt"
  check "the switch went platinum -> winamp -> platinum" eq "$(echo "$before$during$after" | jq -sc '[.[].face]')" '["platinum","winamp","platinum"]'
  check "a row is selected before the switch" eq "$(echo "$before" | jq -r .sel)" "occ-4"
  check "the selection, folds, view and size are kept" eq "$(echo "$before$during$after" | jq -sc '[.[] | [.sel, .view, .eqOpen, .plOpen, .dbl, .shade]] | unique | length')" "1"
  check "it still plays the same track" eq "$(echo "$before$during$after" | jq -sc '[.[] | [.status, .marquee, .source]] | unique | length')" "1"
  check "the position ran on through both switches" test "$(echo "$before$during$after" | jq -s '.[0].positionMs < .[1].positionMs and .[1].positionMs < .[2].positionMs')" = true
  check "the switch sent no command and no playback request" eq "$(commands)$(requests_of 'own\..*|queue\..*|item\..*|transport\..*')" "[][]"
  kb chord alt+g
}

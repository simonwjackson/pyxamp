# faces.sh: where each face draws the controls the checks press (surface sp,
# the pointer lands on the middle of the sp). The step-2 checks run on both
# faces from this table, so one script proves both faces send the same
# commands and requests. Sourced after session.sh and helpers.sh.
#
# use_face winamp|platinum picks the table; FACE_KIND names it.

declare -A PT
use_face() {
  FACE_KIND=$1
  if [ "$1" = platinum ]; then
    # PlatinumMain.qml, PlatinumEq.qml (at y 150), PlatinumPlaylist.qml (at y 272), lib/sliders.js.
    PT=(
      [focus]="160 136" [play]="55 113" [next]="151 113" [eject]="182 113"
      [volume]="258 105" [seek]="100 91" [balance_right]="280 119" [balance_centre]="258 119" [speed]="260 119"
      [eq_band1]="69 196" [presets]="260 177" [menu]="14 29" [menu_first]="47 53" [off_menus]="262 300"
      [misc]="194 497" [row_x]="120 0"
    )
    # The 2x check compares whole-sp art: the scroll bar (chrome text is smooth Chicago at 12k px).
    LIST_CROP="15x192+278+291"
    CONSOLE_PX="50 67 #4a5840"         # the console flat line on the LCD (light, own audio: mid ink)
  else
    # MainSection.qml, EqSection.qml (at y 116), PlaylistSection.qml (at y 232).
    PT=(
      [focus]="150 110" [play]="50 97" [next]="119 97" [eject]="147 97"
      [volume]="141 62" [seek]="100 77" [balance_right]="205 62" [balance_centre]="196 62" [speed]="194 62"
      [eq_band1]="85 161" [presets]="238 140" [menu]="10 8" [menu_first]="40 25" [off_menus]="262 300"
      [misc]="113 414" [row_x]="100 0"
    )
    LIST_CROP="243x145+12+252"
    CONSOLE_PX="50 51 #b0c8b0"
  fi
}
pt_x() { local p=(${PT[$1]}); echo "${p[0]}"; }
pt_y() { local p=(${PT[$1]}); echo "${p[1]}"; }
click_pt() { click_sp "$(pt_x "$1")" "$(pt_y "$1")" "${2:-}"; }
at_pt() { at_sp "$(pt_x "$1")" "$(pt_y "$1")"; }
# A playlist row's middle, where the face draws it (describe plRowY).
row_y() { describe | jq -r ".plRowY[$1]"; }
click_row() { click_sp "$(pt_x row_x)" "$(row_y "$1")" "${2:-}"; }
# The library's More row into view. The Platinum outline shows the open levels
# above the page, so the row sits lower: its scroll bar's down arrow brings it in.
reveal_more() {
  [ "$FACE_KIND" = platinum ] || return 0
  local i; for i in 1 2 3 4 5 6 7 8; do click_sp 285 476; done; sleep 0.5
}
# Album mode off by the face's own control: the Winamp title-bar button, the Platinum Misc menu item.
album_mode_control() {
  if [ "$FACE_KIND" = platinum ]; then click_pt misc; sleep 0.4; kb key down; kb key down; kb key enter; sleep 0.3
  else click_sp 247 $((232 + 10)); fi
}

# session.sh: a private headless desktop for the face's checks. Sourced by
# face/tools/verify. Everything lives under $T (a fresh temporary directory):
#   headless labwc -> nested Hyprland (lua config) -> two HEADLESS monitors
#   (1920x1080 at scale 1, 2880x1800 at scale 2), the replay server on a
#   private socket, one virtual pointer, one virtual keyboard, and the face as
#   `qs -c pyxamp`.
# Nothing touches the user's session: XDG_RUNTIME_DIR, the Hyprland instance,
# the D-Bus address, XDG_CONFIG_HOME and XDG_STATE_HOME all point into $T.
# Never opens a seat, a DRM device or libinput (it checks, and stops if so).

FACE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# A tool from $ENV, PATH, or the newest matching store path (already installed).
# An explicit $ENV is used as given: if it is not executable, stop; never fall back.
find_tool() {
  local var=$1 bin=$2 glob=$3 v
  v=${!var:-}
  if [ -n "$v" ]; then
    [ -x "$v" ] || { echo "verify: $var=$v is not an executable" >&2; return 1; }
    echo "$v"; return
  fi
  if command -v "$bin" >/dev/null 2>&1; then command -v "$bin"; return; fi
  v=$(ls -d /nix/store/$glob/bin/$bin 2>/dev/null | sort -V -t- -k3 | tail -1)
  if [ -n "$v" ]; then echo "$v"; return; fi
  echo "verify: no $bin found; set $var to its path" >&2; return 1
}

wait_for() { # wait_for TRIES CMD...: retry every 0.25 s
  local n=$1; shift
  for _ in $(seq "$n"); do "$@" && return 0; sleep 0.25; done
  return 1
}

session_up() {
  QS=$(find_tool QS quickshell '*-quickshell-0.*') || return 1
  HYPRLAND=$(find_tool HYPRLAND Hyprland '*-hyprland-0.5[6-9]*') || return 1
  HYPRCTL=$(dirname "$HYPRLAND")/hyprctl
  LABWC=$(find_tool LABWC labwc '*-labwc-*') || return 1
  echo "verify: quickshell $QS" >&2; echo "verify: hyprland $HYPRLAND" >&2; echo "verify: labwc $LABWC" >&2

  # Nix and rust-script keep their caches where they always do (faster, and
  # nothing is written into $T after it is removed).
  TOOL_CACHE=${XDG_CACHE_HOME:-$HOME/.cache}
  # A short path: Hyprland's event socket path must stay under 108 bytes.
  T=$(mktemp -d /tmp/pxf-XXXXXX)
  R=$T/r
  mkdir -m 700 "$R"
  mkdir -p "$T/logs" "$T/shots" "$T/config/hypr" "$T/config/quickshell" "$T/state" "$T/cache" "$T/home"
  ln -s "$FACE_DIR" "$T/config/quickshell/pyxamp"
  cp "$FACE_DIR/tests/hyprland.lua" "$T/config/hypr/hyprland.lua"
  export XDG_RUNTIME_DIR=$R XDG_CONFIG_HOME=$T/config XDG_STATE_HOME=$T/state XDG_CACHE_HOME=$T/cache HOME=$T/home
  export DBUS_SESSION_BUS_ADDRESS=unix:path=$T/no-bus
  export PYXAMP_SOCKET=$R/pyxamp-replay/control.sock PYXAMP_FACE_STATE=$T/state/pyxamp/face.json
  unset WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE DISPLAY
  : > "$T/pids"

  WLR_BACKENDS=headless WLR_RENDERER=gles2 WLR_LIBINPUT_NO_DEVICES=1 \
    nice -n 10 "$LABWC" > "$T/logs/labwc.log" 2>&1 < /dev/null & echo $! >> "$T/pids"
  wait_for 40 test -S "$R/wayland-0" || { echo "verify: labwc did not start" >&2; return 1; }
  LIBSEAT_BACKEND=seatd SEATD_SOCK=/nonexistent/seatd.sock AQ_DRM_DEVICES=/nonexistent/card \
    WAYLAND_DISPLAY=wayland-0 nice -n 10 "$HYPRLAND" -c "$T/config/hypr/hyprland.lua" > "$T/logs/hypr.log" 2>&1 < /dev/null & echo $! >> "$T/pids"
  hypr_ready() { local s; s=$(ls "$R/hypr" 2>/dev/null | head -1); [ -n "$s" ] && [ -S "$R/hypr/$s/.socket.sock" ]; }
  wait_for 60 hypr_ready || { echo "verify: Hyprland did not start" >&2; return 1; }
  HYPRLAND_INSTANCE_SIGNATURE=$(ls "$R/hypr" | head -1)
  export HYPRLAND_INSTANCE_SIGNATURE WAYLAND_DISPLAY=wayland-1
  sleep 1
  hc output create headless >/dev/null; sleep 0.5
  hc output create headless >/dev/null; sleep 0.5
  local heads; heads=$(hc -j monitors all | jq -r '.[].name' | grep '^HEADLESS' | sort -V)
  MON1=$(echo "$heads" | sed -n 1p); MON2=$(echo "$heads" | sed -n 2p)
  [ -n "$MON1" ] && [ -n "$MON2" ] || { echo "verify: no headless monitors" >&2; return 1; }
  hc eval "hl.monitor({ output = \"$MON1\", mode = \"1920x1080@60\", position = \"0x0\", scale = \"1\" })" >/dev/null
  hc eval "hl.monitor({ output = \"$MON2\", mode = \"2880x1800@60\", position = \"1920x0\", scale = \"2\" })" >/dev/null
  sleep 0.5
  hc eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null
  sleep 0.5
  hc dismissnotify >/dev/null   # the "launched without start-hyprland" banner would sit in every shot
  if grep -q "drm: Starting backend\|Seat opened\|libinput: New device" "$R"/hypr/*/hyprland.log 2>/dev/null; then
    echo "verify: UNSAFE: the nested Hyprland opened a seat, DRM or libinput; stopping" >&2; session_down; return 9
  fi

  XDG_CACHE_HOME=$TOOL_CACHE "$FACE_DIR/tools/replay-server" --socket "$PYXAMP_SOCKET" --control "$T/replay.ctl" --fixture idle > "$T/logs/replay.log" 2>&1 & echo $! >> "$T/pids"
  XDG_CACHE_HOME=$TOOL_CACHE "$FACE_DIR/tools/wl-pointer" --control "$T/pointer.ctl" --extent 3360x1080 > "$T/logs/pointer.log" 2>&1 & echo $! >> "$T/pids"
  xkbcli compile-keymap --layout us > "$T/us.xkb"
  XDG_CACHE_HOME=$TOOL_CACHE "$FACE_DIR/tools/wl-keyboard" --control "$T/keyboard.ctl" --keymap "$T/us.xkb" > "$T/logs/keyboard.log" 2>&1 & echo $! >> "$T/pids"
  helpers_ready() { [ -S "$T/replay.ctl" ] && [ -S "$T/pointer.ctl" ] && [ -S "$T/keyboard.ctl" ]; }
  # The first run of wl-keyboard compiles it (rust-script), so allow a few minutes.
  wait_for 720 helpers_ready || { echo "verify: replay server, pointer or keyboard did not start; see $T/logs" >&2; return 1; }
}

hc() { "$HYPRCTL" --instance 0 "$@"; }

face_start() {
  QT_QPA_PLATFORM=wayland "$QS" -c pyxamp > "$T/logs/qs.log" 2>&1 < /dev/null &
  FACE_PID=$!
  echo "$FACE_PID" >> "$T/pids"
  wait_for 80 describe_ok || { echo "verify: the face did not start; see $T/logs/qs.log" >&2; return 1; }
  sleep 0.5
}
face_gone() { ! kill -0 "$FACE_PID" 2>/dev/null; }
face_stop() { [ -n "${FACE_PID:-}" ] && kill "$FACE_PID" 2>/dev/null; wait_for 20 face_gone; FACE_PID=""; }
ipc() { "$QS" -c pyxamp ipc call pyxamp "$@"; }
describe() { ipc describe; }
describe_ok() { describe >/dev/null 2>&1; }
field() { describe | jq -r ".$1"; }

ctl() { printf '%s\n' "$1" | socat -t 2 - "UNIX-CONNECT:$T/replay.ctl"; }
ptr() { printf '%s\n' "$*" | socat -t 5 - "UNIX-CONNECT:$T/pointer.ctl" | grep -q '^ok$'; }
kb() { printf '%s\n' "$*" | socat -t 5 - "UNIX-CONNECT:$T/keyboard.ctl" | grep -q '^ok$'; sleep 0.15; }

# The face's layer surface geometry in layout px: "x y w h", or empty when hidden.
layer_geometry() {
  hc -j layers | jq -r '[.[] | .levels[] | .[] | select(.namespace == "pyxamp")] | .[0] | select(. != null) | "\(.x) \(.y) \(.w) \(.h)"'
}
# "MONITOR LEVEL" of the face's layer surface (level 2 = top, 1 = bottom).
layer_level() {
  hc -j layers | jq -r 'to_entries[] | .key as $m | .value.levels | to_entries[] | select(any(.value[]; .namespace == "pyxamp")) | "\($m) \(.key)"'
}
monitor_scale() { hc -j monitors | jq -r ".[] | select(.name == \"$1\") | .scale"; }
monitor_x() { hc -j monitors | jq -r ".[] | select(.name == \"$1\") | .x"; }

# Point at, or click, a point of the surface in skin pixels.
at_sp() {
  local g k scale lx ly lw lh
  g=$(describe); k=$(echo "$g" | jq -r .k); scale=$(monitor_scale "$(echo "$g" | jq -r .screen)")
  read -r lx ly lw lh <<< "$(layer_geometry)"
  ptr "move $(jq -n "$lx + ($1 + 0.5) * $k / $scale") $(jq -n "$ly + ($2 + 0.5) * $k / $scale")"
}
# The pointer first moves off and back: after a popup closes, Qt needs a fresh
# enter before it takes a click (a resting real mouse is not different).
click_sp() { at_sp "$(($1 + 2))" "$(($2 + 1))"; sleep 0.05; at_sp "$1" "$2"; sleep 0.1; ptr "${3:-click}"; sleep 0.3; }

# Focus a monitor the way a person does: move the pointer onto it (follow_mouse).
focus_monitor() {
  local x; x=$(monitor_x "$1")
  ptr "move $((x + 400)) 500"; sleep 0.4
}

shot() { grim -o "$2" "$T/shots/$1.png"; } # shot NAME MONITOR: physical px

session_down() {
  [ -n "${T:-}" ] || return 0
  if [ -f "$T/pids" ]; then
    tac "$T/pids" | while read -r p; do kill "$p" 2>/dev/null; done
    sleep 1
    tac "$T/pids" | while read -r p; do kill -9 "$p" 2>/dev/null; done
  fi
  # Anything else started inside the session carries its runtime dir.
  local p
  for p in $(pgrep -u "$(id -u)"); do
    { tr "\0" "\n" < "/proc/$p/environ"; } 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=$R" && kill "$p" 2>/dev/null
  done
  return 0
}

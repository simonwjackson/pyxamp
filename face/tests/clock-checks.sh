# clock-checks.sh: the face's clock under wall-clock jumps, and rebasing on
# snapshots. Sourced by face/tools/verify and face/tools/verify-clock, inside a
# running session (session.sh) with check/eq/fail/load defined.
#
# The face runs with tests/wallclock-shift.c preloaded: only its wall clock
# moves, by an offset the check writes to a file while the face plays. The
# shown position must follow real elapsed time. (The earlier clock, which
# stepped Date.now, counted each +1 s jump: it failed this check.)

clock_checks() {
  local shim shift_file rc_start p0 p1 t0 t1 dp dt i
  shim=$T/wallclock-shift.so; shift_file=$T/wallclock-shift
  nix --extra-experimental-features "nix-command flakes" shell nixpkgs#gcc --command \
    cc -shared -fPIC -O2 -o "$shim" "$FACE_DIR/tests/wallclock-shift.c" || { fail "the wall-clock shim builds"; return; }
  echo 0 > "$shift_file"
  face_stop
  FACE_ENV=(LD_PRELOAD="$shim" WALLCLOCK_SHIFT_FILE="$shift_file")
  face_start; rc_start=$?; FACE_ENV=()
  [ "$rc_start" -eq 0 ] || { fail "the face starts with the wall-clock shim"; return; }
  load following-playing; sleep 1
  p0=$(field positionMs); t0=$(date +%s%3N)
  for i in 1 2 3 4; do echo "$((i * 1000))" > "$shift_file"; sleep 0.4; done
  sleep 0.4
  p1=$(field positionMs); t1=$(date +%s%3N)
  dp=$((p1 - p0)); dt=$((t1 - t0))
  check "four +1 s wall-clock jumps do not move the position (moved $dp ms in $dt ms)" test $((dp - dt)) -lt 500 -a $((dt - dp)) -lt 500
  echo -3600000 > "$shift_file"; sleep 0.5
  p0=$(field positionMs); sleep 1; p1=$(field positionMs)
  check "a wall clock set back an hour neither stops nor rewinds it ($((p1 - p0)) ms in 1 s)" test $((p1 - p0)) -gt 700 -a $((p1 - p0)) -lt 1500
  load following-full; sleep 0.3
  p0=$(field positionMs)
  check "a snapshot rebases the position, also backwards (to $p0 ms)" test "$p0" -ge 83000 -a "$p0" -lt 84500
  load following-paused; sleep 0.3
  p0=$(field positionMs); sleep 1; p1=$(field positionMs)
  check "paused: the position stays as observed" eq "$p0 $p1" "254000 254000"
  face_stop
  face_start || fail "the face restarts without the shim"
}

# Pyxamp face

The Quickshell face of Pyxamp: its own config, run as `qs -n -c pyxamp`
(ADR 0005). It draws the daemon's state and sends commands over the daemon's
control socket. It never reads MPRIS itself.

## Layout

| Path | Kind | What it is |
|---|---|---|
| `shell.qml` | root | Config entry: clock, preferences, daemon link, face, IPC `pyxamp` (`toggle`, `show`, `hide`, `describe`) |
| `DaemonLink.qml` | binding | The only data edge: socket client, handshake, latest snapshot and visualizer frame, commands, reconnect |
| `Prefs.qml` | binding | Local view preferences, `$XDG_STATE_HOME/pyxamp/face.json` |
| `MonoClock.qml` | system | The monotonic clock: QtQuick FrameAnimation elapsedTime (QElapsedTimer), paused (never stopped) while nothing needs time |
| `Face.qml` | composition root | Derives the view model once (`lib/view.js`), owns local view state and every action |
| `PyxampPanel.qml` | window | The one layer-shell surface; scales the column by k physical px per skin px |
| `MainSection.qml`, `ShadeSection.qml`, `EqSection.qml`, `PlaylistSection.qml`, `Visualizer.qml` | shapes | The sections at Winamp 2's coordinates (docs/mock/pyxamp.js) |
| `PopupMenu.qml`, `MenuPanel.qml`, `Tooltip.qml`, `Tips.qml` | system | Win9x menus and tooltips (xdg popups of the surface) |
| `SkinSprite.qml`, `Text5.qml`, `SysText.qml`, `Hotspot.qml`, `SkinButton.qml`, `TitleButton.qml`, `TitleDrag.qml` | system | Sprites, the two bitmap fonts, pointer targets |
| `lib/*.js` | pure | Protocol codec, view model, fit, visualizer physics, keys, preferences, EQ curve, text layout |
| `lib/sprites.js`, `assets/skin.png` | generated | By `tools/gen-skin` from `docs/mock/skin.js` and W95FA Grid; see `assets/NOTICE.md` |
| `fixtures/*.json` | generated | Step-1 wire states for the replay server, by `tools/make-fixtures` |
| `lib/wire-schema.js` | generated | Validators compiled from the canonical socket schema (`contracts/pyxamp.schema.json`) by `tools/gen-wire-validators`; `lib/protocol.js` validates every server line with them, then adapts it |
| `tools/verify` | check | The face's gate (below) |
| `tools/verify-clock`, `tests/clock-checks.sh`, `tests/wallclock-shift.c` | check | Wall-clock jumps and snapshot rebasing; the shim moves only the face's wall clock |
| `tools/smoke-daemon` | check | The face against the real `pyxamp-daemon` and the daemon's native MPRIS test player on a private bus |
| `tools/replay-server` | test server | A real protocol-v1 socket server that serves fixtures; for checks only |
| `tools/wl-pointer`, `tools/wl-keyboard` | test input | One virtual pointer and one virtual keyboard for the headless session |

## Runtime

- Socket: `$XDG_RUNTIME_DIR/pyxamp/control.sock`. `PYXAMP_SOCKET` names
  another socket; only `tools/verify` sets it, to reach its replay server.
- Preferences: `$XDG_STATE_HOME/pyxamp/face.json` (non-sensitive only).
  `PYXAMP_FACE_STATE` names another file (checks only).
- Show and hide: `qs -c pyxamp ipc call pyxamp toggle` (SUPER+Z). It shows on
  the monitor Hyprland reports as focused, at the place last left there.
- Scale: k = round(monitor scale), doubled by Ctrl+D. The column is drawn at
  k physical px per skin px; the compositor scale is divided out once
  (`lib/fit.js`). A short monitor shortens the playlist first, then refuses
  Ctrl+D (16 px margin).

## Checks

```sh
face/tools/verify            # unit tests, drift, headless screenshots and interactions
```

It starts a private headless desktop under `/tmp/pxf-*` (headless labwc, a
nested Hyprland with two monitors at scale 1 and 2, the replay server, a
virtual pointer and keyboard) and runs the real face against it. It never
touches the user's session or bus. Screenshots and filmstrips stay in
`$PYXAMP_VERIFY_OUT` (default: a new `/tmp/pyxamp-face-verify-*`).

It uses the installed Quickshell, Hyprland (0.56 or later, lua config) and
labwc. `QS`, `HYPRLAND` and `LABWC` choose them; an explicit value is used as
given and never replaced by another binary. Other tools come from its Nix
shebang. The first run compiles `tools/wl-keyboard` (rust-script).

The real-daemon smoke runs when both paths are given (the combined gate sets
both; `verify` then fails if the smoke fails). Run it alone the same way:

```sh
PYXAMP_DAEMON_BIN_DIR=/path/to/bin PYXAMP_PLAYER_FIXTURE=/path/to/mpris-player-fixture face/tools/smoke-daemon
```

It uses a private D-Bus session and runtime dir, the production socket path
inside it, and a PipeWire remote that does not exist (capture must report
unavailable). `face/tools/verify-clock` runs only the clock checks.

Regenerate after changing the mock's pixel maps or the fixtures:

```sh
face/tools/gen-skin
face/tools/make-fixtures
face/tools/gen-wire-validators            # after the daemon's schema changes (--schema PATH, or PYXAMP_SCHEMA)
```

## Known limits

- Menu items act on press, not release: under Hyprland 0.56 the release of a
  virtual pointer reaches a layer surface's popup late or not at all. A real
  mouse is not affected, but press-to-pick works for both.
- Hyprland sends the headless session's virtual pointer no
  `wl_pointer.frame`, so `tools/wl-pointer` follows each button with a motion
  to the same place; drags lag by one event in the checks.
- The face extrapolates position between snapshots; a snapshot re-bases it,
  so a late snapshot can step the clock back by up to the daemon's refresh
  interval.

# Pyxamp face

The Quickshell face of Pyxamp: its own config, run as `qs -n -c pyxamp`
(ADR 0005). It draws the daemon's state and sends commands over the daemon's
control socket. It never reads MPRIS itself.

It has two built-in faces, both renderers of the one state in `Face.qml`
(ADR 0006): the Winamp face and the Platinum face. The desktop theme picks one
(`DesktopTheme.qml`, `lib/desktop-theme.js`; the rule is in docs/brief.md,
"Which face"), and a switch changes only which sections draw: keys,
commands, requests, preferences and playback stay.

## Layout

| Path | Kind | What it is |
|---|---|---|
| `shell.qml` | root | Config entry: clock, preferences, daemon link, face, IPC `pyxamp` (`toggle`, `show`, `hide`, `describe`) |
| `DaemonLink.qml` | binding | The only data edge: socket client, handshake, latest snapshot and visualizer frame, commands, reconnect |
| `Prefs.qml` | binding | Local view preferences, `$XDG_STATE_HOME/pyxamp/face.json` |
| `MonoClock.qml` | system | The monotonic clock: QtQuick FrameAnimation elapsedTime (QElapsedTimer), paused (never stopped) while nothing needs time |
| `Face.qml` | composition root | Derives the view models once (`lib/view.js` for the main section, `lib/playlist.js` for the playlist section), owns local view state and every action: step-1 commands while a foreign player is followed, `own.*` and the other step-2 requests otherwise. Holds the face kind and appearance, and the playlist scroll per face and view |
| `DesktopTheme.qml` | binding | Reads and watches the desktop's `theme-name`, `theme`, `theme-extra` and theme registry (`$PYXAMP_DESKTOP_THEMES` or `$XDG_CONFIG_HOME/quickshell/mountainous/themes.json`); `lib/desktop-theme.js` decides the face and the appearance |
| `PyxampPanel.qml` | window | The one layer-shell surface; scales the column by k physical px per skin px; loads the face's column |
| `MainSection.qml`, `ShadeSection.qml`, `EqSection.qml`, `PlaylistSection.qml`, `Visualizer.qml` | shapes | The Winamp face's sections at Winamp 2's coordinates (docs/mock/pyxamp.js) |
| `PlatinumColumn.qml`, `PlatinumMain.qml`, `PlatinumShade.qml`, `PlatinumEq.qml`, `PlatinumPlaylist.qml`, `PlatinumRow.qml`, `PlatinumVis.qml`, `PlatinumTitle.qml` | shapes | The Platinum face: one 300-sp Platinum window, sections that keep a 17-sp disclosure header when folded, the LCD, the Finder list view and outline (the approved mock's Platinum renderer) |
| `PlatinumRects.qml`, `PlatinumButton.qml`, `PlatinumSlider.qml`, `PlatinumText.qml`, `PlatinumLcdText.qml` | system | Whole-sp art (`lib/platinum.js`), bevel buttons, sliders, smooth ChicagoFLF chrome text, 1-bit LCD type |
| `PopupMenu.qml`, `MenuPanel.qml`, `PlatinumMenuPanel.qml`, `Tooltip.qml`, `PlatinumBalloon.qml`, `Tips.qml` | system | Menus and tips (xdg popups of the surface): Win9x menus and tooltips at k, or the desktop's Platinum menus and Balloon Help at the monitor's own scale |
| `SkinSprite.qml`, `Text5.qml`, `SysText.qml`, `Hotspot.qml`, `SkinButton.qml`, `TitleButton.qml`, `TitleDrag.qml` | system | Sprites, the two bitmap fonts, pointer targets |
| `lib/*.js` | pure | Protocol codec, view model (following or own playback), playlist model (queue, album mode, library view, jump box, item info, prompt line), fit (both faces), visualizer physics, keys, preferences, EQ curve and presets, text layout; slider drags from each control's own rectangle (`drag.js`, `sliders.js`); the desktop theme rule (`desktop-theme.js`); the Platinum face's tokens, art, outline, sentence case and LCD type layout (`platinum-tokens.js`, `platinum.js`, `outline.js`, `lcdcase.js`, `lcdtext.js`) |
| `assets/ChicagoFLF.ttf`, `assets/licenses/ChicagoFLF-README.txt` | font | ChicagoFLF 2.1 (public domain, the same file as mountainous's `chicagoflf` package), loaded by the face itself, so it never depends on the host's fonts |
| `lib/lcdfont.js`, `assets/lcd-font.png` | generated | By `tools/gen-lcd-font`: ChicagoFLF 12 in 1 bit, one band per LCD ink |
| `lib/sprites.js`, `assets/skin.png` | generated | By `tools/gen-skin` from `docs/mock/skin.js` and W95FA Grid; see `assets/NOTICE.md` |
| `fixtures/*.json` | generated | Wire states (and `pages`, the answers to `library.open` and `search`) for the replay server, by `tools/make-fixtures` |
| `lib/wire-schema.js` | generated | Validators compiled from the canonical socket schema (`contracts/pyxamp.schema.json`) by `tools/gen-wire-validators`; `lib/protocol.js` validates every server line with them, then adapts it |
| `tools/verify` | check | The face's gate (below) |
| `tools/verify-clock`, `tests/clock-checks.sh`, `tests/wallclock-shift.c` | check | Wall-clock jumps and snapshot rebasing; the shim moves only the face's wall clock |
| `tools/verify-step2`, `tests/step2-checks.sh` | check | Step 2 alone: own playback, sources, queue, album mode, library, jump box, prompt line, spoken word, conflicts, retries, EQ and balance; screenshots at 1x and 2x |
| `tests/helpers.sh` | check | Shared check helpers: commands and requests the replay server got, shots and crops |
| `tools/verify-platinum`, `tests/platinum-checks.sh` | check | The Platinum face: the theme picks it (live), the step-1 checks at its own targets, drags from each control's rectangle, appearances, a theme switch that keeps playback, and the step-2 checks on it |
| `tests/faces.sh` | check | Where each face draws the controls the step-2 checks press, so those checks run on both faces |
| `tools/platinum-boards` | check | Screenshots of the Platinum face by the mock's groups 1-8, in light, dark grey and extra dark (shots only) |
| `tools/smoke-daemon` | check | The face against the real `pyxamp-daemon` and the daemon's native MPRIS test player on a private bus |
| `tools/replay-server` | test server | A real protocol-v1 socket server that serves fixtures. It answers step-2 requests (pages from a fixture's `pages`) and can move a fixture's wall-clock fields to now (`rebaseWall`). For checks only |
| `tools/wl-pointer`, `tools/wl-keyboard` | test input | One virtual pointer and one virtual keyboard for the headless session |

## Runtime

- Socket: `$XDG_RUNTIME_DIR/pyxamp/control.sock`. `PYXAMP_SOCKET` names
  another socket; only `tools/verify` sets it, to reach its replay server.
- Preferences: `$XDG_STATE_HOME/pyxamp/face.json` (non-sensitive only).
  `PYXAMP_FACE_STATE` names another file (checks only).
- Face and appearance: the desktop's `$XDG_STATE_HOME/mountainous/theme-name`,
  `theme` and `theme-extra`, and the theme registry
  (`$XDG_CONFIG_HOME/quickshell/mountainous/themes.json`, or
  `$PYXAMP_DESKTOP_THEMES`). Read only; watched, and read again every 2 s.
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
touches the user's session or bus, and never the GPU's card or a seat: labwc,
Hyprland and the face run in bubblewrap with a `/dev` that holds only
`/dev/dri/renderD128`, Hyprland gets nonexistent `AQ_DRM_DEVICES` and
`SEATD_SOCK`, and `drm-proof.txt` records Hyprland's log lines and an fd scan
of every session process. The session's desktop theme is its own
(`$T/state/mountainous`, `$T/themes.json`); it starts on `win98`, the Winamp
face. `face/tools/verify-platinum` runs the Platinum face's checks the same way. Screenshots and filmstrips stay in
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
unavailable). `face/tools/verify-clock` runs only the clock checks, and
`face/tools/verify-step2` only the step-2 checks. `face/tools/verify-platinum`
runs the unit tests, the Platinum face's checks and the step-2 checks on the
Platinum face (`PYXAMP_PLATINUM_PARTS="checks"` or `"step2"` runs one part).

## Step 2 in the face

- The main section shows the followed foreign player (`player`, amber, step-1
  commands) while there is one, else the active source's session (`own`,
  green, `own.*` requests). While own playback yielded, the amber title ends
  with the resume rule; when the foreign player goes, the display says
  "BACK TO PYXAMP: RESUMING" or "STAYS PAUSED" for 2 s.
- Wall-clock deadlines (yield, retry) are counted from the snapshot's arrival
  on the monotonic clock, so a wall-clock jump between snapshots cannot move
  them.
- The playlist model (`lib/playlist.js`) does not depend on the clock: its
  rows are rebuilt only when a snapshot, a key or a page changes them.
- The volume slider, Up and Down are the system volume (`snapshot.systemVolume`,
  `system.volume`) in every state: following, own playback, console mode and
  idle. A boost above 100 % shows as a full slider; the tooltip says the real
  value and "(muted)". Up at or above 100 % sends nothing, so it never lowers a boost.
- EQ and balance always go to the engine (`eq.set`, `balance.set`), and once
  quietly on every connect. AUTO sends Voice while spoken word is the active
  source. The built-in presets live once, in `lib/eq.js`.
- For spoken word, Prev and Next (and Z, B) send `own.seek` by the reported
  skip intervals, so the face does not depend on what a source does with
  `own.transport previous` or `next`. The arrows keep the 5 s seek.

Regenerate after changing the mock's pixel maps or the fixtures:

```sh
face/tools/gen-skin
face/tools/make-fixtures
face/tools/gen-wire-validators            # after the daemon's schema changes (--schema PATH, or PYXAMP_SCHEMA)
face/tools/gen-lcd-font                   # after the LCD inks (lib/platinum-tokens.js) or the font change
```

## The Platinum face

- 1 sp is one skin pixel. The art is whole-sp rectangles (`lib/platinum.js`)
  at k physical px per sp, so every bevel is k px. Chrome text is ChicagoFLF
  drawn smooth at 12k px; LCD text is ChicagoFLF 12 in 1 bit, scaled by k.
- Menus and Balloon Help draw at the monitor's own scale; Ctrl+D does not
  double them.
- Its tokens are the desktop's (`lib/platinum-tokens.js` names the source,
  mountainous `modules/quickshell/PlatinumTokens.qml`); only the LCD values and
  the label chips are new.
- The LCD writes the model's upper-case messages in sentence case
  (`lib/lcdcase.js`). A short message that does not fit wraps onto line 2.
- The library view is a Finder outline of the model's own rows: keys move in
  the deepest open level, as on the Winamp face; a click on a higher row
  selects it there and folds what is below it.
- The grow box sets the playlist height in the same steps as the preference
  (`plN`, 0 to 6). The Winamp face has no grow box.

## Known limits

- Menu items act on press, not release: under Hyprland 0.56 the release of a
  virtual pointer reaches a layer surface's popup late or not at all. A real
  mouse is not affected, but press-to-pick works for both.
- Hyprland sends the headless session's virtual pointer no
  `wl_pointer.frame`, so `tools/wl-pointer` follows each button with a motion
  to the same place; drags lag by one event in the checks.
- In the checks, a popup can take the virtual pointer's after-click motion as
  a hover on its first row, so the checks drive submenus with the pointer
  (as step 1 does) and use the keyboard only where that hover cannot matter.
- The face extrapolates position between snapshots; a snapshot re-bases it,
  so a late snapshot can step the clock back by up to the daemon's refresh
  interval.

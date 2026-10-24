# Direction: Pyxamp, pass 1

The brief settles the look: Winamp 2's default skin at a squint, redrawn. This file holds only what the brief leaves open, and the motion spec.

## Plan (tokens, before code)

- **Unit.** One skin pixel (1 sp). The surface is drawn at 1 sp into a 1× buffer and scaled by a whole number k with nearest-neighbour (`imageSmoothingEnabled = false`, `image-rendering: pixelated`). k = round(monitor scale) × (Ctrl+D ? 2 : 1). Every metric below is in sp.
- **Colour (own values, Winamp roles).**
  | Role | Hex | Note |
  |---|---|---|
  | Chassis | #34344E → #23233A | slate, one colour per row, darker at the bottom |
  | Bevel light / dark | #66668A / #10101C | 1 sp lines |
  | LCD | #000000 with #191926 dots | dots on every other sp, the "screen texture" |
  | LCD text, own audio | #22E422 | marquee, digits, kbps, playlist text |
  | LCD text, followed player | #FFB020 | amber: the one bold choice (see Principles) |
  | Button face | #BCC5D6, light #EEF2F9, shade #7A8299, edge #262A3A, glyph #2E3654 | |
  | Title stripes, active | #EFE2B4 / #A88C4A on #3A301A | inactive: #8A8AA0 / #4E4E64 |
  | EQ gold | #E8B830 | dB labels, presets |
- **Type.** Three faces, all bitmaps on the sp grid:
  1. *Pyx 5×6*: my own 5×6 cell font (A–Z, 0–9, 28 signs). Marquee, kbps/kHz, button labels, EQ labels. Lower case maps to upper case, as `text.bmp` did.
  2. *Pyx 9×13*: my own seven-segment digits with bevelled ends, for the clock.
  3. *W95FA Grid* (OFL, built in the win98 mock) at 12.5 px = 1 font px per sp, alpha thresholded to one colour. Playlist rows, popup menus, tooltips. Winamp drew these with the system font too.
  Plus a 7-row label set (k b p s H z m o n t e r i d) for "kbps", "kHz", "mono", "stereo", "bit", "db".
- **Layout.** Winamp's docked column, 275 sp wide: main 116, EQ 116, playlist 116 + 29·n (default n = 3, 203 sp). Shade: main 14 sp. Every control sits at Winamp 2's coordinates (researcher.md), so the squint holds.

```
┌ ✦ ═════════ PYXAMP ═════════ ▁▔✕ ┐  title 14
│ O┌──────────┐ ┌ marquee 31 ch ──┐ │
│ A│▶ -12:34  │ └─────────────────┘ │
│ I│ ▂▅▇▃▂▁▁  │ [320]kbps [44]kHz mono stereo
│ D│ spectrum │ ═══◘═══  ══◘══ [EQ][PL]
│ V└──────────┘                     │
│ ════════════◘═══ position ══════  │
│ [|◀][▶][‖][■][▶|] [⏏] [SHUFFLE][↻]  ✦
└────────────────────────────────────┘
┌ ═════ PYXAMP EQUALIZER ═════  ▁✕ ┐
│ [ON][AUTO]  curve graph  [PRESETS] │
│  ┃   +12 db  ┃┃┃┃┃┃┃┃┃┃            │
│ PREAMP       60 … 16K              │
└────────────────────────────────────┘
┌ ═════ PYXIS QUEUE ═════════════ ▁✕ ┐
│ 1. Artist - Title           3:45  ▮│
│ …                                  │
│ [ADD][REM][SEL][MISC] 0:00/43:40 [LIB]
└────────────────────────────────────┘
```

- **Principles.**
  1. *The squint is the contract.* Coordinates, proportions and colour roles are Winamp 2's. New controls reuse an existing sprite slot (balance → speed, LIST OPTS → LIB, kbps → bit depth) instead of adding chrome.
  2. *One bold move: the LCD changes colour when Pyxamp shows a player that is not its own.* Green = Pyxamp's own audio. Amber = a followed foreign player. That is the glance test for "following someone else".
  3. *The marquee is the status line*, the Winamp way: errors, speed, skip amounts, the device, hand-back messages.
  4. *Nothing new opens a window.* Library, jump box, item info draw inside the playlist section.
- **Data (stress set).** Long title (marquee), 0 / 1 / 1,240 queue items, a source unreachable, a stream error, lossless 24/96 and 16/44, mono podcast, played and part-played spoken word, 3 and 9 library groups, a jump query with 0 and with 3 sources of hits.

### Review against the brief

- First plan: a coloured source badge (P / J / O) in the main LCD. It read as new chrome and broke the squint. Changed: the active source shows in the playlist title ("PYXIS QUEUE") and in the library; the main section stays Winamp.
- First plan: lossless shown as "1K4" kbps. Not readable. Changed: the kbps box shows the bit depth and its label becomes "bit" ("24 bit 96 kHz").
- First plan: a second colour for console mode (blue LCD). Two tints dilute the one bold move. Changed: console stays green; the device leads the marquee and the vis is a flat line.

## Motion

| Element | Kind | Trigger | Property | From → to | Start | Duration | Easing |
|---|---|---|---|---|---|---|---|
| Marquee | Idle | text longer than 31 ch | text x offset | 0 → −(len+7)·5 sp, then repeats | on track change | 5 sp per 220 ms step (loop = (len+7)·220 ms) | steps(1) |
| Spectrum bars | Idle | own or followed audio plays | bar height | target per 33 ms tick | – | falls 1.25 sp per tick | linear |
| Spectrum peaks | Idle | a bar rises above its peak | peak y | holds 6 ticks, then falls | – | speed 0.12 sp/tick, × 1.12 per tick | accelerating |
| EQ / PL fold | none (approved change to decision 20) | EQ or PL button, Alt+G / Alt+E | swaps at once, as Winamp 2 | – | – | 0 ms | – |
| SUPER+Z show / hide | none (approved change to decision 20) | SUPER+Z | appears or disappears at once, at the place last left | – | – | 0 ms | – |
| Windowshade | none | shade button, Alt+W | swaps at once, as Winamp 2 | – | – | 0 ms | – |
| Menus, tooltips, jump box | none | click, key | appear at once | – | – | 0 ms | – |

Reduced motion (`prefers-reduced-motion: reduce` or the mock switch): the marquee does not scroll (the first 31 characters show; the full text is in the tooltip). The visualizer stays off until you pick a mode in the V menu, and its area says "VIS OFF" in the skin's 5×6 font, dim LCD green ("OFF" in the windowshade's mini vis), so it does not look broken. Folds, windowshade and SUPER+Z are instant in every mode.

All motion reads one clock. `window.seek(seconds)` holds every frame (marquee, vis) at that time; `window.play()` releases it.

### Takes (measured)

Re-recorded after the approved change to decision 20 (no fold or fade animation).

- `shots/motion/take-620x960-2x.webm` (30 fps tile: `work/take30.png`): the EQ fold and unfold each go from one state to the other between two neighbouring frames, with no frame in between. SUPER+Z: the mean brightness of a 40×40 crop goes 16 → 40 → 16 with no value in between, so the hide and the show are instant.
- `shots/motion/take-620x960-2x-reduced-motion.webm` (`work/take30-rm.png`): the same, and the visualizer area shows "VIS OFF".
- `shots/motion/frames-seeked.png`: the marquee steps 5 sp per 220 ms; after a pause the peaks hold about 200 ms, then fall faster; the frame after EQ, PL, Alt+W or SUPER+Z is already the end state; reduced motion shows "VIS OFF" and a still marquee.
- Not taken: a take at 1×.

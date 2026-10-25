# Pyxamp design brief

Settled with Simon in a design interview on 2026-10-24. Terms are defined in `CONTEXT.md`. Hard-to-reverse choices are in `docs/adr/`. Research behind it: `~/.cache/research/winamp-pyxis-20261024T153523/`.

## What it is

An everyday desktop audio player for Pyxis (music), Jellyfin (music, video by hand-off) and Ossicle (podcasts and narrated books). At a squint it is Winamp 2 with its default skin; on the Platinum desktop it wears the Platinum face instead (see Faces). Up close it has new features. It is not called Winamp and ships no Nullsoft art.

## Look

- Two built-in faces that follow the desktop theme: the Winamp face (a redrawn Winamp 2 default skin, pixel by pixel) and the Platinum face (see Faces). No `.wsz` skin support (ADR 0002, ADR 0006).
- One surface: main, EQ and playlist sections in Winamp's docked column. EQ and PL fold sections in and out. Windowshade and Ctrl+D double size stay (ADR 0004).
- Scale is the monitor's scale rounded to a whole number; Ctrl+D doubles it again.
- Only popup menus, tooltips, a handed-off Jellyfin video, and later MilkDrop appear outside the surface. The jump box and item info are drawn inside the playlist section.

## Faces (approved 2026-10-25)

Pyxamp has two built-in faces, both renderers of one state model (ADR 0006). A face changes how a behaviour looks, never which behaviours exist. Where other sections of this brief describe the look (Win98-grey menus and tooltips, W95FA Grid, upper-case marquee text, a green display in console mode, three title bars, the drill-down library), they describe the Winamp face. The Platinum face replaces those parts as below.

### Which face

- The face reads `$XDG_STATE_HOME/mountainous/theme-name`, the file that mountainous's `Theme.qml` reads. `platinum` gives the Platinum face. `win98` or any unknown theme gives the Winamp face.
- Preferred mechanism: a `pyxampFace` field per theme in mountainous's `modules/quickshell/themes.json` (open item). Until it exists, the face checks the theme id.
- The appearance (light, dark grey, extra dark) follows the desktop's `theme` and `theme-extra` state files.
- A theme change switches the face at once and keeps all state, because the daemon owns playback.
- No face override: the O menu has no Face item.
- Code, settings and visible strings say "Platinum face". They never say "MacAMP" or "Mac" (ADR 0006 gives the reasons). The window title is "Pyxamp".

### Platinum face

Summarised from the approved mock's `DIRECTION.md` and `REPORT.md` (`/tmp/macamp-mock/`, temporary). Key shots are kept in `docs/assets/platinum-face/`, and the research behind the look is in `docs/assets/platinum-face/REFERENCES.md`. The face takes its chrome, its LCD in a bevel and its disclosure triangle from AppleCD Audio Player, and its list from SoundJam, not from the real MacAMP.

**Principles**

- It belongs to the desktop, not to a skin. The chrome uses the bar's tokens, its font and its menus. The only new colours are inside the display.
- The display's colour says whose audio it is, as on the Winamp face: sage or green for Pyxamp's own audio, amber for a followed player.
- Mac idioms replace Winamp idioms one for one. No behaviour is added or removed.
- Nothing opens a window. The library, the jump box and item info draw inside the playlist section.

**Unit, scale and type**

- 1 sp is one skin pixel, as on the Winamp face. The art is whole-sp rectangles at k = round(monitor scale) × (Ctrl+D ? 2 : 1), so every bevel is exactly k physical px.
- Menus and Balloon Help are the exception: they draw at the monitor's own scale (1.5 on Myoko), and Ctrl+D does not double them.
- ChicagoFLF only. Chrome text is smooth, at 12 sp (title, list rows), 11 sp (buttons, columns) and 10 sp (small labels).
- LCD text is ChicagoFLF 12 thresholded to 1 bit, then scaled by k. 12 px is the only size that thresholds cleanly.
- The clock is a seven-segment digit, 9 × 15 sp, with ghost segments.

**Tokens.** The chrome tokens are mountainous's `PlatinumTokens.qml`. Only the LCD values and the label chips are new.

| Token | Light | Dark grey | Extra dark |
|---|---|---|---|
| face | #DDDDDD | #3A3A3A | #000000 (flat, no bevels) |
| bevel hi / lo | #FFFFFF / #999999 | #565656 / #1E1E1E | none |
| frame / button edge | #000000 / #000000 | #000000 / #000000 | #5C5C5C / #8C8C8C |
| button / pressed or on | #E6E6E6 / #BDBDBD | #444444 / #242424 | #000000 / #262626 |
| text / text-2 / off | #000 / #3A3A3A / #8A8A8A | #ECECEC / #B8B8B8 / #767676 | #E6E6E6 / #A4A4A4 / #5C5C5C |
| highlight / text | #333399 / #FFF | #5858D0 / #FFF | #5858D0 / #FFF |
| list / sorted column | #FFFFFF / #EEEEEE | #2C2C2C / #333333 | #000000 / #121212 |
| menus | #DDDDDD, black edge, #222 hard shadow | #3A3A3A | #3A3A3A (menus stay dark grey, as on the bar) |
| LCD, own audio (new) | sage, reflective: bg #B4C0A2, ink #1C2616, ghost #A9B697 | black glass: bg #0A0E09, ink #A6E89C | bg #000000, ink #A6E89C |
| LCD, following (new) | amber backlight: bg #E9B24A, ink #2A1904 | bg #0E0A04, ink #FFB020 | bg #000000, ink #FFB020 |
| lamp on | #2FBF2F | #48D848 | #48D848 |
| label chips (new) | Pyxis #E07020, Jellyfin #3A6AD0, Ossicle #2A9A3A | lighter variants | lighter variants |

**Window**

- One Platinum window, 300 sp wide (the Winamp face is 275), with a 16-sp windoid title bar.
- Close box (left): hide, like Super+Z. Zoom box: Ctrl+D. Collapse box: windowshade. In shade, the title bar holds a mini LCD and a mini transport.
- The clutterbar is five small bevel buttons (O A I D V) beside the display.
- Heights: main 150 sp, EQ 122 sp, playlist 82 + 16 × rows sp, with rows = 4 + 2 × plN. The default plN of 3 gives 10 rows and 515 sp in total.
- A folded EQ or playlist keeps its 17-sp disclosure header ("▸ Equalizer"). Folds stay instant.
- Extra dark is a flat black face with #5C5C5C/#8C8C8C outlines and no bevels, like the bar.

**Display**

- A real LCD behind a sunken Platinum bezel.
- Light: a reflective sage LCD with dark ink. Dark grey and extra dark: a black LCD with green ink. Amber always means "following another player", in every appearance.
- Seven-segment clock with ghost segments. An "h" annunciator shows only when the clock shows hours.
- Spectrum: 19 single-ink bars of 7 segments, with a mid-tone peak segment.
- Title: 1-bit Chicago in mixed case, on a 168-sp line. It holds 1.5 s, then scrolls at 24 sp/s in whole-sp steps.
- One rate line: "320 kbps  44 kHz", "24 bit  96 kHz". There are no fixed character boxes, so "176 kHz" fits.
- "mono" and "stereo" annunciators light up.
- A third-line annunciator names whose audio it is: the followed app or the console device ("Firefox", "Living room"). It shows at all times while Pyxamp follows a player or drives a console, so the name does not scroll away with the title.
- Mixed case: the face converts the model's upper-case messages to sentence case ("Speed: 1.5x", "Back to Pyxamp: stays paused"). It keeps a small dictionary of proper nouns (Pyxamp, Pyxis, YouTube, device names). A short message that overflows wraps onto the LCD's second line for its 2 s, so it does not scroll.

**Controls**

- Transport: square Platinum bevel buttons.
- Shuffle, Repeat, EQ, List: sticky bevel buttons with an AppleCD-style lamp and a sunk bevel when on. "Repeat 1" names the third state in words.
- Volume, balance or speed, and seek: Platinum sliders with pointed thumbs. Seek and speed have a progress-bar fill.
- EQ: On and Auto buttons, the curve on an LCD, a Presets pop-up, and 11 vertical Platinum sliders (preamp and 10 bands) with ticks and a fill from 0 dB.

**List view (playlist section)**

- A Finder list view: a header line ("14 items, 4:10 of 1:09:31"), column titles (#, Name, Time), and a shaded sorted column.
- A speaker glyph marks the current item. While Pyxamp follows another player, a pause glyph and "paused" show instead.
- A Platinum scroll bar and a grow box. The grow box drags in playlist steps (2 rows = one plN).
- Pop-up buttons Add▾ Remove▾ Select▾ Misc▾ replace ADD/REM/SEL/MISC. A Library button replaces LIB.
- The playlist section's mini transport and mini clock are dropped, because they duplicate the main section.
- Library: a Finder outline. Every open level stays visible, with triangles and indents. The keys are the same: Right or Enter unfolds, Left or Backspace folds back to the parent and keeps the parent selected (the Winamp face jumps to the first row), Esc at the root goes back to the queue. A click on a triangle unfolds; a click on a row at a higher level folds what is below it. Deep levels indent and cut long names.
- Jump box: a sunken "Find:" field with a highlight-colour focus ring and the result count in the field. A "Where" column shows a colour chip (the label chips above) and the source name. The hint line sits at the bottom.

**Menus**

- The desktop's own Platinum menus: ChicagoFLF 14, #333399 highlight, hard shadow.
- They draw at the monitor's scale. Ctrl+D does not double them, as with native menus in Winamp 2. At k = 4 they look small next to the surface; this is accepted.
- In extra dark, menus stay dark grey, as on the bar.
- Production reuses the bar's `PlatinumMenuFrame` and `PlatinumMenuList` (open item).

**Balloon Help**

- Balloon Help replaces tooltips: a white (light) or dark balloon with a tail, in Chicago 12, at the monitor's scale.
- With reduced motion, the title does not scroll. It is cut with an ellipsis, and the full text is in the balloon. The spectrum area says "vis off" in the LCD's mid tone until a mode is picked in the V menu.

**Motion.** The same as the Winamp face. Folds, windowshade, Ctrl+D, Super+Z, menus and balloons change at once (0 ms). The paused clock and the jump-field caret blink at 1 s. Short messages show for 1–2.5 s, then go at once.

**Size and fit.** The same rule as the Winamp face (the playlist gets shorter first, then Ctrl+D is refused), measured against the usable height under the top bar.

| Screen | k | Size (physical px) | Playlist |
|---|---|---|---|
| Myoko, 1× | 2 | 600×1030 | 10 rows |
| Myoko, Ctrl+D | 4 | 1200×1676 | 4 rows (3 steps shorter) |
| 1080p, 1× | 1 | 300×515 | 10 rows |
| 1080p, Ctrl+D | 2 | 600×966 | 8 rows (1 step shorter) |

On Myoko with Ctrl+D the playlist has 4 rows and the surface reaches the Control Strip's row. Simon accepted this.

**Shots** (in `docs/assets/platinum-face/`, copied from the mock; Chromium, not Quickshell)

| What | File |
|---|---|
| Winamp face and Platinum face, same state | `winamp-vs-platinum-2x.png`, `live-winamp-face-same-desktop.png` |
| State boards, light, 2× | `1-pyxis-host-2x.png`, `2-pyxis-console-2x.png`, `5-followed-player-2x.png`, `6-playlist-section-2x.png`, `7-eq-section-2x.png` |
| Dark grey and extra dark | `dark-grey-main.png`, `extra-dark-main.png` |
| Following, in situ, three appearances | `live-following-light.png`, `live-following-dark-grey.png`, `live-following-extra-dark.png` |
| Menus and Balloon Help | `live-play-on-menu.png`, `live-balloon-help.png` |
| 300% zoom (title bar, display, Play, EQ slider) | `zoom-300-light.png` |
| Myoko with Ctrl+D (k = 4) | `myoko-2x-k4-light.png` |

## Desktop

- A Quickshell face on a layer-shell surface, above normal windows. The clutterbar's A button switches between on top and on the desktop under the windows.
- SUPER+Z shows and hides it on the focused monitor, at the place last left there. At login it comes back as it was left.
- The face draws; a daemon plays through mpv and keeps playing through shell reloads (ADR 0003).
- Pyxamp is a proper MPRIS player. The media keys go to Pyxamp, which sends them to the followed player (replaces the `playerctl` binds in mountainous).
- Universal remote: the main section follows the player that started most recently, including a browser tab.
- Yield: when any foreign MPRIS player starts, Pyxamp pauses. It resumes after a short interruption (up to 2 minutes) and stays paused after a longer one.
- The visualizer shows the followed player's audio, captured from PipeWire. Spectrum and oscilloscope only; MilkDrop later.

## Sources

- One active source at a time; the playlist section shows that source's own queue (ADR 0001). Playing from another source pauses the old one at its source and keeps its place.
- Album mode: a toggle shows the queue as one row per album ("Stereolab – Dots and Loops (1997)  10 tracks  1:09:31"); Enter opens its tracks. For Ossicle it groups up next by show; a book is already one row.
- Pyxis playlists in Pyxamp: play or queue from the library view, save the current queue as a playlist (MISC, name in the prompt line), and right-click a track > Add to playlist. No rename, delete or reorder in Pyxamp.
- Browsing: the playlist section switches between the queue and the library view. J opens the jump box, which searches all three sources.
- The library view draws whatever groups each source reports. Nothing below a source's name is hard-coded, because Pyxis groups are dynamic (placements, signals, tags). First-level groups as of 2026-10-24:
  - Pyxis: Albums, Hot albums, Playlists, Bookmarks, Stations. Stations stay separate from albums.
  - Jellyfin: Artists, Albums, Playlists, Videos (hand-off only).
  - Ossicle: Up next, Shows, Books.
- Credentials come from mountainous secrets. No sign-in screen.
- Online only in v1. Offline downloads later.

### Pyxis

- Pyxamp is both a host (plays here) and a console (drives a session elsewhere, such as Sonos).
- O menu > Play on picks the device. The marquee says where it plays.
- In console mode the visualizer is a flat line and the marquee shows the playing device.
- Pyxis has no call today that lists its groups. Placements are a fixed enum (Discovery, Collection, Archive, Dismissed), and Hot is a separate signal. Dynamic groups need a Pyxis contract change, like the ones below.
- The Pyxis contract must gain: seek, repeat mode (off, all, one; the R key and the button cycle), and shuffle as a mode that can be undone. These are changes in the Pyxis repo (`services/pyxis/src/rpc/contract.rs`), under its plan's rules.

### Jellyfin

- Audio plays in Pyxamp. Video is listed and handed off to an mpv window.
- Pyxamp reports plays and progress to Jellyfin, for music and for handed-off video.

### Ossicle (spoken word)

- Resume position, playback speed, skip intervals and played flags, all synced through Ossicle.
- The balance slider becomes a speed slider while spoken word plays; the marquee shows "SPEED: 1.5x".
- Prev and Next skip back and forward by Ossicle's intervals; change episodes in the playlist section.
- Mark as played: right-click an item, or the MISC menu.
- Shuffle and Repeat are drawn dimmed and do nothing.

## Build and delivery (settled 2026-10-24)

- Daemon in Rust; face over one Unix socket (JSON lines); its own Quickshell instance (ADR 0005).
- Every desktop host gets Pyxamp through mountainous's desktop layer (today: Myoko, Ibuki, Hiyori).
- The repo is public on GitHub. Mountainous takes it as a flake input. Creating the GitHub repo needs Simon's approval.
- A track that cannot play is retried after 2 s and 10 s, then skipped, and marked red in the list.
- Typing (EQ preset names) uses a prompt line over the playlist section, like the jump box. A Pyxis station starts from right-click > Start station on a song or an artist (`source.station.create` takes a seed, not a URL).
- Jellyfin apps cannot cast to Pyxamp in v1.

## Pyxis changes (settled 2026-10-24)

These go into the Pyxis rewrite plan as new units (`work/items/active/20260821123211-pyxis-v2-rewrite/plan.md`), under that repo's rules.

- The core reports library groups: placements, computed signals such as Hot, and tags. Each group has a label, a count and a kind. Clients draw what the core reports.
- Seek as a session command.
- Session modes: `repeat` (off, all, one) and `shuffle` (on, off). The core keeps the original order, so turning shuffle off restores it. `queue.shuffle` is removed. Every host (web player, Pyxamp, the server's Sonos host) implements both.
- Playlists stay. They are a primitive (a named list of tracks), not the hub. VISION.md's line changes to: "Not a playlist manager: playlists exist as a simple primitive, a named list of tracks, but albums are the unit and placements organise the library. No surface promotes playlists or makes them the hub. Stations and source playlists are discovery tools."
- The core sends the group order: album groups first (placements, Hot, tags), then Bookmarks, Stations, and Playlists last.
- Pyxamp needs a call that adds tracks to an existing playlist. The contract has only `library.playlist.create` and `library.playlists.list` today (verified).

## Ossicle behaviour (settled 2026-10-24)

- Speed range 0.5x to 3.0x and skip intervals 1 to 300 s come from Ossicle's settings, with per-show overrides (verified in `services/ossicle/src/settings.rs`).
- A position conflict shows as one row in the playlist section with both positions and devices; Enter keeps one. The display says "POSITION CONFLICT" until it is resolved.
- Sponsor segments are skipped when Ossicle's `sponsorSkip` is on; the display says "SKIPPED SPONSOR".

## Build order (settled 2026-10-24)

1. Face and daemon as a universal remote: follows any MPRIS player, the visualizer from PipeWire, yield, media keys, SUPER+Z. Needs no source and no Pyxis change.
2. Pyxis host and console. Needs the Pyxis contract units, which run in parallel in the Pyxis repo.
3. Ossicle.
4. Jellyfin, with the video hand-off.

Mock pass 2 added the states decided after pass 1. The approved mockup lives in `docs/mock/` (open `docs/mock/index.html`; `docs/mock/REPORT.md` lists choices 1–40). Choices 26–40 from pass 2 are the defaults for the build. Where pass 1's stream-error state ("X TO RETRY") and the retry rule disagree, the build uses the retry display (choice 40).

## EQ

10 bands plus preamp, applied to Pyxamp's own audio only.

## Open items (from the Platinum face, 2026-10-25)

- **Face-relative drag mapping.** Every drag must map from its control's own rectangle, not from hard-coded Winamp coordinates. Today `face/MainSection.qml` maps volume with `(x + mx - 114) / 54` and seek with `(x + mx - 30) / 219`. Fix this for the Winamp face before the Platinum face is built (ADR 0006).
- **The `pyxampFace` registry field.** Add a `pyxampFace` field to each theme in mountainous's `modules/quickshell/themes.json` (`platinum` → Platinum face, `win98` → Winamp face). This is a change in the mountainous repo. Until it lands, the face checks the theme id. As of 2026-10-25, `themes.json` and `PlatinumMenuFrame.qml` are on mountainous's `cairn` branch, not on its `main`.
- **Reusable Platinum menu components.** Let an app's popup reuse mountainous's `PlatinumMenuFrame` and `PlatinumMenuList` outside the bar, so Pyxamp's menus are the desktop's own. This is a change in the mountainous repo. The menu scale (the monitor's scale, not k) is untested on a real layer-shell popup.
- **Not settled: a missing `theme-name` file.** "Any unknown theme" gives the Winamp face, but mountainous's `Theme.qml` treats a missing or unknown file as `platinum`. Simon must pick which face a missing file gives.

## Mockup pass 1 (approved 2026-10-24)

The mockup is in `/tmp/pyxamp-mock/` (throwaway). Its `REPORT.md` lists 25 choices that this brief did not settle. Simon approved all of them, with two changes. The ones that shape the build:

- While Pyxamp follows a foreign player, the display (clock, title, status light) turns amber. The title starts with the app's name, and it states the resume rule ("RESUMES IF THIS STOPS BY 1:26" or "STAYS PAUSED"). When the foreign player stops, the display says "BACK TO PYXAMP: RESUMING" or "STAYS PAUSED" for 2 s.
- A Jellyfin video in an mpv window counts as a followed player, so the music yields.
- In console mode the display stays green, the title starts with the device's name, and the visualizer is a flat line.
- Lossless shows the bit depth ("24 bit 96 kHz"), and the kHz box holds 3 characters. A clock past 99:59 shows hours and minutes with an "H".
- Repeat one draws a "1" by a shorter loop. R cycles all, one, off.
- For spoken word, Prev and Next show their skip interval. The arrow keys keep the 5 s seek. Speed runs from 0.5x to 3.0x in 0.1 steps, with a teal bar and a notch at 1.0x.
- LIST OPTS becomes LIB. LIB, L and Eject open the library view. The playlist title names what it shows ("PYXIS QUEUE", "LIBRARY").
- Jump box results carry a source tag (PYX, JF, OSS). A result from another source says which source pauses.
- Played items are dimmed, with a tick and "played". Part-played items show the time left.
- Menus and tooltips are Win98-grey (Winamp 2 used native Windows menus). The playlist and the menus use W95FA Grid, which is SIL OFL 1.1 with no Reserved Font Name, so a modified copy may ship with its notice and licence.
- Close and minimize only hide Pyxamp; playback goes on in the daemon.
- When the EQ does nothing (console, following), the curve graph says so ("NO EQ ON FIREFOX") and the sliders dim. AUTO picks a preset by kind of audio.
- Folding a section, windowshade, and SUPER+Z are all instant (change from the mock's 250 ms fold and fades).
- With reduced motion the title does not scroll, and the visualizer stays off until a mode is picked; the vis area then says "VIS OFF" (change from the mock).
- Fit: on a short monitor the playlist gets shorter first, then Ctrl+D is refused, with a 16 px margin.
- Errors show in the title with a red status light.
- The mark is a gold compass star.

The text entry and retry policy that pass 1 left open are settled under Build and delivery.

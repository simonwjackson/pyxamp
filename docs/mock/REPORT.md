# Pyxamp mock: report, pass 1 and pass 2

## Amendments, pass 2

Pass 2 adds only the states decided after pass 1 (brief sections "Sources", "Pyxis changes", "Ossicle behaviour", "Build and delivery"). The pass-1 states are not changed. **One visible exception:** the playlist section's title bar now has a third button (album mode), so a live render of any pass-1 state shows it. I did not regenerate the pass-1 shots; they stay the approved record.

**View it:**
- `sheet.html?g=10&k=2`: all pass-2 states at 2×.
- `sheet.html?g=10&k=1&only=am-tracks,am-pyxis,am-pyxis-sel,am-pyxis-open,am-full,am-ossicle,am-ossicle-open`: album mode at 1×.
- `index.html`: the mock-controls picker has a new group, "10. Pass 2". The live keys are G (album mode), Enter/Esc in album mode, Left/Right/Enter on a position conflict, and typing in the prompt line.

**New shots:**
- `shots/states/10-pass2-2x.png`: 30 states.
- `shots/states/10-album-mode-1x.png`: 7 states.
- `shots/check-output.txt`: 82 of 82 pass.

**The new states (board 10):**

| Topic | States |
|---|---|
| Album mode | `am-tracks` (off: 27 track rows), `am-pyxis` (one row per album; whole albums, a partial album, single tracks; the playing album white with a play mark), `am-pyxis-sel`, `am-pyxis-open` (Enter: its tracks), `am-full` (button pressed), `am-ossicle` (runs of one show; the book one row), `am-ossicle-open`, `am-misc` |
| Pyxis playlists | `pl-root` (Pyxis reports 10 groups), `pl-groups` (core order: Discovery, Collection, Archive, Dismissed, Hot, Tag: shoegaze, Tag: late night, a rule, Bookmarks, Stations, Playlists), `pl-playlists`, `pl-open` (a playlist in the library view), `pl-save-prompt` (MISC > Save queue as playlist... and the prompt line), `pl-saved`, `pl-addto` (queue row > Add to playlist > playlists), `pl-addto-lib` (the same from a library track) |
| Prompt line | `pr-eq` (Presets > Save as new preset...), `pr-eq-folded` (the prompt unfolds a folded playlist section) |
| Start station | `st-song` (right-click a song), `st-artist` (an artist result in the jump box), `st-album` (an album in the library: "Start station from Stereolab"), `st-started` (the station plays) |
| Ossicle position conflict | `cf-row` (one row: "This laptop 12:30" / "Pixel 40:02"; the display says POSITION CONFLICT), `cf-pick` (Right picks Pixel), `cf-kept` (Enter kept it) |
| Sponsor skip | `sp-skip` ("SKIPPED SPONSOR"; the segments are dim gold marks on the position bar) |
| Failed track | `ft-retry1` ("CANNOT PLAY: RETRY 1 IN 2 S"), `ft-retry2` ("RETRY 2 IN 7 S"), `ft-skipped` (the next track plays; the failed row is red, "cannot play") |

**Removed:** the pass-1 idea of a station URL entry (hard item 7 and suggested issue 4 below). A station starts from a seed only.

**New code:**
- `pyxamp.js`: `albumRows`, `queueGroups`, `coreRows`, the prompt line in `plRows`, `decorateRow`, `settle()` (the retry clock and the sponsor skip), `startStation`, `keepPosition`.
- `input.js`: keys and clicks for these.
- `data.js`: a mixed Pyxis queue, a mixed Ossicle queue, the core groups, playlists, stations.
- `states.js`: group 10.

**Checks:** `bin/check.sh` passes **82 of 82**, with 0 page errors in all 5 sessions. The new checks:
- **Album mode:** G turns it on; the title-bar button turns it off and on; Down selects the next album; Enter opens it; Esc goes back.
- **Prompt line:** MISC > Save queue as playlist opens it. Typed letters go into it, and Z and X do not reach transport. Backspace edits. Esc cancels. Enter saves, and the display says "SAVED AS PLAYLIST: SUN". Presets > Save as new preset opens the same line, and Esc closes it.
- **Add to playlist:** right-click a track > hover "Add to playlist" opens the submenu; picking "Long drive north" shows "ADDED TO PLAYLIST: LONG DRIVE NORTH".
- **Position conflict:** the display says "POSITION CONFLICT"; Right picks Pixel; Enter keeps it and plays ("KEPT PIXEL 40:02").
- **Failed track:** "RETRY 1" shows first, then "RETRY 2" after 2 s; after 12 s the next track plays and the row is marked failed.

I read every cell of board 10 at 2× and the album cells at 1×. That review found and fixed three things:
- Album rows were cut short: the year is now dropped first.
- Long confirmations were cut at 31 characters: they now scroll.
- The prompt label hid the typed name: the label is now "Name", and the text is cut from the left.

### New choices for Simon (default first, then its cost)

26. **Album mode toggle.** Default: a third title-bar button in the playlist section (a 5×5 record glyph, left of shade/close), drawn pressed while album mode is on. Also the G key, and MISC > "Album mode" for Pyxis. Why there: the main section already has three title-bar buttons at x 244/254/264, so the playlist gets the same three-button silhouette. The bottom bar has no free slot without shrinking the time box. The toggle also sits next to the title, which names the view ("PYXIS QUEUE BY ALBUM"). Cost: in Winamp, title-bar buttons are window controls, and the glyph is small. The button now shows in every playlist state.
27. **What groups.** Default: only consecutive queue items from one album (or one show) merge into a row, so the row order stays the true play order. Cost: the same album or show can appear twice (The Talk Show does, in `am-ossicle`).
28. **Album row text.**
    - Default: "1. Stereolab - Dots and Loops (1997)" with "10 tracks  1:09:31" on the right. A partial album says "5 of 17 tracks". The playing album says "now 4/10" and has a play mark. A single track stays a track row.
    - When a row is too narrow, the year goes first, then the title is cut. Rows are numbered by album; an opened album shows queue numbers.
    - Cost: the brief's example row does not fit 243 sp with its year (it reads "Stereolab - Dots and Loops   now 4/10  1:09:31"). There are two numbering schemes.
29. **Up/Down in album mode.** Default: they move the row selection, as in the library view; in the track queue they stay volume (Winamp). Cost: the same keys mean different things in the two queue views.
30. **Ossicle show rows.** Default: "3 episodes, 1 played  2:03:57"; a book row is "book  -10:07:55" (time left). Cost: no per-episode detail until Enter.
31. **Core groups.** Default: draw labels and counts exactly as reported; tags as "Tag: shoegaze"; one thin rule between the album groups and Bookmarks/Stations/Playlists; Dismissed shown if the core sends it. Cost: the rule is a new element, and the board assumes the core reports Dismissed (not checked).
32. **Prompt line.**
    - Default: the title bar names the action ("SAVE AS PLAYLIST", "SAVE EQ PRESET"). The head row reads "Name: Sunday afternoon_" with a summary on the right ("27 tracks"), and a hint row sits at the bottom.
    - The list below stays visible but does nothing while the prompt is open. The text is cut from the left so the caret stays in view. Names are limited to 40 characters. An empty name saves nothing ("NO NAME: NOTHING SAVED").
    - Cost: while the prompt is open, the list cannot be used.
33. **Prompt when the playlist section is folded.** Default: the prompt unfolds the section and folds it again after Enter or Esc. Cost: the column jumps twice.
34. **Add to playlist.** Default: in the queue row menu (Pyxis only) and on library track rows. The submenu lists the playlists in Pyxis's order and has no "New playlist" item. The display says "ADDED TO PLAYLIST: NAME". Cost: a new playlist can only come from "Save queue as playlist".
35. **Start station.**
    - Default: a song row gets "Start station" (the song is the seed). An album row gets "Start station from <artist>". An artist result in the jump box gets "Start station". Enter on a Stations row starts that station.
    - It replaces what plays now with the station's queue, and the display says "STATION FROM BRAKHAGE (SONG)".
    - Cost: I did not check whether Pyxis keeps the old session queue anywhere.
36. **Position conflict row.**
    - Default: the item's own row becomes the choice: the title cut short, then two boxes, "This laptop 12:30" and "Pixel 40:02". Left/Right picks, Enter keeps, and a click on a box keeps it.
    - Playback waits, paused. The clock and the position thumb are hidden. The display says "POSITION CONFLICT" without scrolling until the conflict is resolved, then "KEPT PIXEL 40:02 (SYNCED)".
    - Cost: the arrow keys do not seek while a conflict is open, the title is cut hard, and playback waits for the choice.
37. **Sponsor segments.** Default: "SKIPPED SPONSOR" for 2 s; the segments are dim gold marks in the position groove. Cost: on a 5-hour episode a 90 s segment is about 1 sp wide, so the marks barely show (the board uses a 38-minute episode).
38. **Failed track display.**
    - Default: "CANNOT PLAY: RETRY 1 IN 2 S" (counting down), then "RETRY 2 IN 10 S". The status light is red, and the visualizer, clock and thumb are off. The row's time reads "retrying" in red.
    - After the skip, the row is red with "cannot play", and the display says "SKIPPED: TRACK CANNOT PLAY".
    - Cost: the mock assumes retry 1 fails at once, so the 10 s count starts at 2 s.
39. **Long confirmations.** Default: a confirmation longer than 31 characters scrolls from its start instead of being cut. Cost: the logo's 32-character message now scrolls. None of the approved stills changes.
40. **The pass-1 stream error ("X TO RETRY").** Default: in the build, the settled retry display replaces it; the approved state `x-stream-err` stays in the mock as drawn. Cost: board 8 shows wording that the build will not use.

### Hard or contradictory (pass 2)

- The brief's album-row example ("Stereolab – Dots and Loops (1997)  10 tracks  1:09:31") is wider than the 243-sp list in W95FA Grid. Something has to give (decision 28).
- "Groups up next by show" does not say whether the runs must be consecutive. Grouping all of a show's episodes would reorder what plays (decision 27).
- The pass-1 library boards (group 6) show the old 5-group Pyxis list. Board 10's core order supersedes them for the build.
- The brief does not say whether playback waits during a position conflict (decision 36).
- "Add to playlist" needs a Pyxis call that does not exist yet (the brief notes this).

### Not verified (pass 2)

- How Pyxis treats the current queue when a station starts.
- How Ossicle reports sponsor segments, and when it reports a conflict (at item start, or mid-play).
- All pass-2 data is fake: the mixed queues, the core groups and counts, the playlists, the stations, the conflict positions and the sponsor segments.


## Pass 1 amendments (approved by Simon)

Simon approved pass 1 and all 25 defaults, with two changes. Both are applied:

1. **Decision 20.** Folding or unfolding the EQ and PL sections is instant, and SUPER+Z shows and hides Pyxamp instantly, with no fade, as Winamp 2 did. Windowshade stays instant. The 250 ms fold and the 120/80 ms fades are removed from `pyxamp.js` (`foldFrac` and `surfaceAlpha` are now on/off).
2. **Decision 21.** With reduced motion, the visualizer stays off until a mode is picked in the V menu. Its area now says "VIS OFF" in the skin's 5×6 font, in dim LCD green ("OFF" in the windowshade's mini vis), so it does not look broken.

Also: **the W95FA licence is checked.** W95FA is SIL OFL 1.1 (FontsArena, Alina Sava), and its OFL.txt declares no Reserved Font Name. So the modified copy named "W95FA Grid" may ship bundled, with the copyright notice and the licence included. This closes the licence part of decisions 15 and 16.
Flag (not checked further): the licence file copied into this mock, `fonts/W95FA-OFL.txt`, still has the template header (`Copyright (c) <dates>, <Copyright Holder>`). Before shipping, take the copyright line from the upstream W95FA release.

Regenerated for these changes:
- `shots/motion/take-620x960-2x.*` and `take-620x960-2x-reduced-motion.*`: the old takes showed the fold and the fades.
- `shots/motion/frames-seeked.png`: the fold and fade strips are replaced by an "instant" strip, and the reduced-motion strip shows "VIS OFF".
- `shots/09-live-reduced-motion-vis-off.png`: new.
- `shots/check-output.txt`: 58 of 58 pass.

The state boards, size shots, sprite sheets and the other live stills are unchanged. None of them was drawn mid-fold or with reduced motion.

Pass 1 is done. One surface (main section, EQ section, playlist section) is drawn on a 1-sp grid and scaled by whole numbers only. Every sprite is my own pixel map in `skin.js`. The states cover the 9 groups of the task. 54 of 54 interaction checks pass. The recorded take shows the marquee, the spectrum, the EQ fold and SUPER+Z, with and without reduced motion.

## Open it

    xdg-open /tmp/pyxamp-mock/index.html

- **Live surface.** Click the desktop first, then use the keys: Z X C V B, J (jump box), L (library view), R (repeat cycle), S (shuffle), Ctrl+D, Ctrl+T, Alt+W (shade), Alt+G (EQ), Alt+E (playlist), Alt+3 (item info), Super+Z, Esc. Right-click a playlist row for its menu. Drag the sliders.
- **Mock controls** (right panel, outside the surface): pick any state, the monitor scale (1 or 2), Ctrl+D, SUPER+Z, reduced motion, hold the clock.
- **URL:** `index.html?state=<id>&mon=1|2&dbl=1&hold=1&rm=1&ui=0&t=<seconds>`.
- **Other pages:**
  - `sheet.html?g=1..8&k=1|2[&only=id,id]`: state boards.
  - `sizes.html?screen=myoko|fhd&dbl=0|1&fold=none|eq|pl|both|shade`: one physical screen.
  - `motion.html`: every motion row, frame by frame, from the seekable clock.
  - `sprites.html`: all sprite sheets at 4×.
- **Scripts (`bin/`):**
  - `sheets.sh`: state boards.
  - `shots.sh`: live stills.
  - `sizes.sh`: screens.
  - `sprites.sh`: writes `sprites/*.png` at 1×.
  - `take.sh`: the recording; `RM=1` records the reduced-motion take.
  - `check.sh`: interaction checks.
- **Files:**
  - `DIRECTION.md`: tokens, the review against the brief, the motion table.
  - `skin.js`: palette, fonts, sprites.
  - `data.js`: fake data.
  - `pyxamp.js`: state, `layout`, `plan`, renderer, menus, actions.
  - `input.js`: keys and pointer.
  - `states.js`: the state catalogue.

## Shot list

**State boards** (`shots/states/`, each cell labelled with its state id):

| File | Contents |
|---|---|
| `1-pyxis-host-2x.png`, `1-pyxis-host-1x.png` | Playing, paused (lit and dark blink frames), stopped, long title (start, 5.5 s, the `  ***  ` separator), elapsed, remaining, 320 kbps 44 kHz, lossless 24 bit 96 kHz, spectrum, oscilloscope, vis off, repeat off/all/one, shuffle on/off, R feedback, windowshade, whole surface |
| `2-pyxis-console-2x.png` | Console on "Living room": device in the marquee, flat-line vis, stereo lit; 6 s later; "Kitchen" single speaker with mono lit; paused; **O menu > Play on** (console and host); the switch moment; whole surface with the EQ note |
| `3-spoken-word-2x.png` | Podcast, narrated book (hours clock), speed drag ("SPEED: 1.5X"), "-15 S", "+30 S", up next with two played items and a part-played book, right-click menu with "Mark as played", MISC menu, item info |
| `4-jellyfin-2x.png` | Jellyfin music (16 bit 44 kHz); library Jellyfin > Videos with "mpv" tags and tooltip; the hand-off (mpv window outside the surface) |
| `5-followed-player-2x.png`, `5-followed-player-1x.png` | Following a Firefox YouTube tab (amber LCD, Pyxamp's row says "paused"); the marquee tail with the resume rule; over 2:00; shade; EQ note; hand-back in 3 frames; hand-back after a long interruption |
| `6-playlist-section-2x.png`, `6-playlist-section-1x.png` | Queue; library root; Pyxis with 5 groups; Pyxis with 9 groups; Ossicle with 3 groups; Pyxis > Albums; Pyxis > Albums > Dots and Loops; Ossicle > Books; jump box empty, with hits from 3 sources, cross-source hint, nothing found; item info; ADD menu |
| `7-eq-section-2x.png` | EQ on, off, dragging 1K (marquee value), PRESETS menu, AUTO with spoken word (Voice), console note |
| `8-edge-states-2x.png` | Jellyfin unreachable (library root, active source, jump box); nothing queued; 1,240 items scrolled; stream error (and scrolling); buffering; A "on the desktop" vs "on top" with an ordinary window |

**Sizes** (`shots/sizes/`, physical pixels):

| File | Screen | Pyxamp |
|---|---|---|
| `myoko-2x-default.png` | 2880×1800, scale 2 | k = 2, 550×870 |
| `myoko-4x-ctrl-d.png` | same | k = 4, 1100×1624; the playlist is one 29-sp step shorter to fit |
| `myoko-2x-eq-folded.png`, `myoko-2x-both-folded.png`, `myoko-2x-shade-folded.png` | same | EQ folded; EQ and PL folded; windowshade with both sections open |
| `fhd-1x-default.png`, `fhd-2x-ctrl-d.png`, `fhd-1x-pl-folded.png`, `fhd-2x-ctrl-d-shade.png` | 1920×1080, scale 1 | k = 1, k = 2, PL folded, k = 2 shaded |

**Live stills** (`shots/`):
- `01-live-default-2x.png`, `02-live-default-1x.png`: the page with the mock-controls panel.
- `03-live-following-firefox.png`, `04-live-console-play-on.png`, `05-live-spoken-word-menu.png`, `06-live-jump-box.png`, `07-live-desktop-mode.png`.
- `08-live-dpr2-myoko.png`: 2880×1800 at DPR 2.
- `09-live-reduced-motion-vis-off.png`: reduced motion; the vis area says "VIS OFF".
- `squint-sharp.png`, `squint-blurred.png`: the squint test.

**Sprites:**
- `sprites/*.png`: 10 sheets at 1×: `text5x6`, `title`, `digits9x13`, `labels`, `cbuttons`, `shufrep`, `sliders`, `titlebar`, `viscolor`, `lcd-misc`.
- `shots/sprites-4x.png`: the same sheets at 4×.

**Motion** (`shots/motion/`):
- `take-620x960-2x.webm`, `.gif`, `.filmstrip.png` (7.8 s). Steps: 3.5 s of a long title scrolling and the spectrum, click EQ (fold, instant), click EQ (unfold, instant), Super+Z (hide, instant), Super+Z (show, instant).
- `take-620x960-2x-reduced-motion.*`: the same steps with `prefers-reduced-motion: reduce`.
- `frames-seeked.png`: each motion row frame by frame from `seek(t)`: marquee steps, spectrum bars and peaks falling after pause, spectrum while playing, the instant fold, windowshade and SUPER+Z (the next frame is the end state), reduced motion with "VIS OFF".

## Decisions for Simon (default first, then its cost)

1. **How a glance shows "following another player".** Default: the whole LCD (clock, marquee, status) turns amber, and the marquee starts with the app ("FIREFOX: …"). Pyxamp's own current row in the playlist reads "paused". Cost: a second LCD colour, and amber can read as a warning. The app prefix scrolls away on long titles, so the colour carries the cue.
2. **Hand-back wording.** Default: while following, the marquee tail says the rule: "PYXAMP PAUSED, RESUMES IF THIS STOPS BY 1:26", or "STAYS PAUSED (OVER 2:00)". When the foreign player stops, the LCD turns green and "BACK TO PYXAMP: RESUMING" or "BACK TO PYXAMP: STAYS PAUSED" shows for 2 s. Cost: long marquee text. The countdown needs the daemon to know when the foreign playback started.
3. **A handed-off Jellyfin video counts as a followed player.** Default: amber, "MPV: ALIEN (1979)", and Pyxamp's music yields. Cost: the brief does not say whether Pyxamp's own mpv window is "foreign". If it is not, the yield rule needs an exception.
4. **Console look.** Default: the LCD stays green; the marquee starts with the device ("LIVING ROOM: 3. Stereolab - …"); the vis is a flat line; mono/stereo come from the session. Cost: the device name scrolls away on long titles. A switch shows "PLAYS ON LIVING ROOM" for 1.5 s.
5. **Lossless and high sample rates.** Default: the kbps box shows the bit depth and its label becomes "bit" ("24 bit 96 kHz"). The kHz box is 3 characters wide (17 sp, not Winamp's 2-character box), so 176 and 192 fit. Cost: a small change to Winamp's anatomy. The lossless bitrate moves to item info.
6. **Clock past 99:59.** Default: mm:ss up to 99:59, then hh:mm with a small "H" (a book at 6:02:40 shows "06:02 H"). Cost: an unfamiliar form, and the seconds are not visible.
7. **Repeat one.** Default: a shorter loop plus a "1" in the same 28×15 button; R and the button cycle all → one → off. Cost: one-track mode is reachable only by cycling (or O menu > Repeat).
8. **Spoken-word skip buttons.** Default: Prev/Next draw arrows plus the interval ("15", "30"). The marquee says "-15 S" / "+30 S". Left/Right keep Winamp's 5 s seek. Cost: two skip sizes. The 15/30 values are invented (see Not verified).
9. **Speed slider.** Default: 0.5× to 3.0× in 0.1 steps, linear, teal tube with a notch at 1.0×. Cost: 1.0× sits at 20 % of the travel. Ossicle's real range is not checked.
10. **Where the library view lives.** Default: Winamp's LIST OPTS slot becomes **LIB** (with an LED); L toggles it; the eject button also opens it. Cost: no list save/load (the source owns the queue anyway), and eject changes meaning.
11. **Playlist title.** Default: it names what the list shows: "PYXIS QUEUE", "JELLYFIN QUEUE", "OSSICLE UP NEXT", "LIBRARY", "JUMP TO ITEM", "ITEM INFO". Cost: the constant "PYXAMP PLAYLIST" of the squint goes away.
12. **Jump box layout.** Default: one row per result, each with a source tag (PYX, JF, OSS) instead of group headers, so 8 hits fit at the default height. A hint line sits at the bottom. Enter plays, Ins or Shift+Enter queues. A hit from another source says "Enter: play from Jellyfin; Pyxis pauses". Cost: the tag takes 21 sp of each row.
13. **Library rows.** Default: a sticky breadcrumb row ("< Library > Pyxis > Albums"); counts and ">" on the right; the active source in white at the root; Backspace, Left or Esc go up. Cost: the breadcrumb uses one row.
14. **Played and part-played items.** Default: a played item is dimmed, gets a tick, and its time reads "played". A part-played item shows the time left ("-10:07:55"). Cost: a played item no longer shows its length.
15. **Popup menus and tooltips.** Default: Win9x-style grey menus and yellow tooltips in W95FA Grid, at the same whole-number scale as the surface (Winamp 2 used the system's menus). Cost: they look like Windows, not like Quickshell. Licence checked: W95FA is SIL OFL 1.1 with no Reserved Font Name, so "W95FA Grid" may ship bundled with its notice and licence.
16. **Playlist text.** Default: W95FA Grid at 12.5 px = 1 font pixel per sp, thresholded to one colour, 13-sp rows. Cost: the text is slightly larger than Winamp's Arial 8.
17. **Close and minimize.** Default: both hide the surface like SUPER+Z; playback goes on in the daemon. Cost: the face cannot quit Pyxamp. A "Quit" item would be needed somewhere.
18. **EQ when it changes nothing.** Default: in console or follow mode the curve graph says "NO EQ ON LIVING ROOM" / "NO EQ ON FIREFOX" and the sliders dim. They stay editable. Cost: the note covers part of the curve.
19. **EQ AUTO.** Default: a preset per kind of audio (spoken word → Voice, music → the user's preset). Winamp's AUTO matched file names. Cost: a new meaning behind an old label.
20. **Motion (approved with a change).** The EQ/PL fold, windowshade and SUPER+Z are all instant, as in Winamp 2. Only the marquee and the visualizer move. Cost: nothing shows where a folded section went; the button LED is the only cue.
21. **Reduced motion (approved with a change).** The marquee does not scroll (the full text is in the tooltip). The visualizer is off until a mode is picked in the V menu, and its area says "VIS OFF" ("OFF" in the windowshade). Cost: the label uses dim LCD green, so it competes a little with the clock above it.
22. **Fit on a monitor (`plan()` in pyxamp.js).** Default: keep 16 logical px free above and below. If the column does not fit, the playlist first shrinks in 29-sp steps; if it still does not fit, Ctrl+D is refused on that monitor. Cost: the playlist height can differ between monitors. On Myoko with Ctrl+D the playlist loses one step (1624 of 1800 px). The 16 px margin is a guess.
23. **Visualizer style.** Default: thick bands (19 bars of 3 sp), peaks on, my own viscolor ramp. Cost: thin bands and falloff speed are only listed in the V menu.
24. **Errors.** Default: in the marquee, with a red status light: "STREAM ERROR: PYXIS RETURNED HTTP 503. X TO RETRY". An unreachable source is marked in the library root and in the jump box. Cost: the "RETRYING IN 30 S" policy is invented.
25. **The mark.** Default: a gold four-point compass star (Pyxis is the mariner's compass), 9×9 in the title bar and 13×15 where Winamp's corner logo was. Cost: it reads as a "+" at 1×.

## Hard or contradictory in the brief

1. **Followed-player cue vs the marquee.** The brief puts the followed player's title in the marquee, but marquee text scrolls. Any text-only cue ("FIREFOX:") is off-screen for most of a long loop. A non-text cue (decision 1) was needed.
2. **"SPEED: 1.5x".** The Winamp-style 5×6 font is upper case only, so it reads "SPEED: 1.5X".
3. **Winamp's boxes are too small for this data.** The kbps box has 3 characters and the kHz box has 2, which cannot show lossless or 192 kHz. A 5-digit mm:ss clock cannot show a 16-hour book (decisions 5 and 6).
4. **The visualizer vs reduced motion.** The visualizer is content, but it is also idle motion, and the motion-mockup rules say idle motion stops under reduced motion. The brief does not settle which rule wins (decision 21).
5. **The EQ applies to Pyxamp's own audio only**, but in console and follow modes the EQ section still shows the same controls. It must say that it does nothing (decision 18).
6. **Is Pyxamp's own mpv video window a "foreign player"?** The brief does not say, and the answer decides whether music yields to a video (decision 3).
7. **Nothing may open outside the surface, but some actions need typed input** (an EQ preset name, a playlist name). Settled since: a prompt line over the playlist section (pass 2). Stations start from a seed (right-click > Start station), so there is no station URL entry anywhere.
8. **Ctrl+D on Myoko.** 2 × 2 = 4 makes the column 1740 px tall (of 1800), so the brief's "Ctrl+D doubles it again" needs a fit rule (decision 22).
9. **Close with a daemon.** Winamp's close button quits. Here playback lives in the daemon, so close can only hide (decision 17).

## Checks run

- `bin/check.sh`: **58 of 58 pass** on the live page (1600×1000, DPR 1, k = 2), 0 page errors. Output: `shots/check-output.txt`.
  - Z, X, C, V, B change the item and the play state.
  - R cycles all → one → off → all; S toggles shuffle.
  - Ctrl+D changes k from 2 to 4 and back; Ctrl+T switches to time remaining.
  - J opens the jump box, typed letters go into it (not to transport), Esc closes it.
  - The EQ and PL buttons and Alt+G / Alt+E fold and unfold at once: the surface height is 435 → 319 → 435 (and 232) within the 120–150 ms the script waits after the action.
  - The O, V, PRESETS and MISC menus open. Hovering "Play on" opens the submenu. Esc closes the submenu first, then the menu. A click on the desktop closes a menu.
  - Right-click on a row opens its menu, and Esc closes it.
  - L, Enter, Enter goes to Library > Pyxis > Albums; Backspace ×2 and Esc go back to the queue.
  - Alt+3 opens item info and Esc closes it. Alt+W toggles the windowshade. Super+Z hides and shows; right after the hide, the surface has no hit regions. A shows "ON THE DESKTOP".
  - Spoken word: Z moves back 15 s and shows "-15 S"; B shows "+30 S"; R and S do nothing; dragging the speed slider shows "SPEED: 1.xX". Right-click > "Mark as played" marks an unplayed item.
  - Reduced motion: the vis area draws "VIS OFF"; V menu > Spectrum turns the vis on and the label goes away.
- **Pixel-exact (verified):** I sampled each surface down by k and scaled it back up, then compared it with the original. 0 pixels differed for `fhd-2x-ctrl-d` (k = 2), `myoko-2x-default` (k = 2), `myoko-4x-ctrl-d` (k = 4) and `08-live-dpr2-myoko` (DPR 2). The 1× surface has 126 colours. All text is thresholded, so there are no blend pixels.
- **Motion against the spec (DIRECTION.md):**
  - EQ fold and unfold (re-recorded take, 30 fps tile `work/take30.png`): the section changes between two neighbouring frames, with no frame in between.
  - SUPER+Z: the mean brightness of a 40×40 crop goes 16 → 40 → 16 across the hide and the show, with no value in between, in both takes. So both are instant.
  - Marquee: the seeked frames show one 5-sp step per 220 ms. I did not measure it in the recording.
  - Peaks: after a pause, each peak holds about 200 ms, then falls faster. Seen in the seeked frames.
  - Reduced-motion take: the same instant changes; the vis area shows "VIS OFF" (also in `frames-seeked.png` and `09-live-reduced-motion-vis-off.png`).
- **Visual review:** I read every board at 2×, the 1× boards for groups 1, 5 and 6, the sizes shots at Myoko 4× and at 1080p, the sprite sheets, the squint pair and the live stills. That review found and fixed:
  - "PYXAMP" read as "PTIAMP": a new title font.
  - Overlapping skip glyphs.
  - The repeat-one icon.
  - "mpv" drawn twice on video rows.
  - The jump box hiding Ossicle hits: tags instead of headers.
  - The EQ note overflowing the graph.
  - A partial row showing under the jump box hint.
- **No repository changes.**
  - `git status` in `~/code/github/simonwjackson/pyxamp` is clean.
  - I wrote nothing to the mountainous `cairn` worktree. Its `git status` shows changes from other sessions (a `.pi-web` handoff, `docs/plans/media-portability.md`, `.forgerie/`). They are not mine.
  - I did not run `nix flake check` and did not deploy anything.
- **Reference handling:** I took one screenshot of webamp.org to compare proportions. I kept it outside the mock (`/tmp/pyxamp-refview/`) and deleted it afterwards. No pixel of it is in any page, sprite or shot.

## Not verified

- Nothing ran in Quickshell or on Myoko. The Myoko and 1080p images are browser renders at those pixel sizes.
- Qt must draw W95FA Grid at 1 font px per sp without hinting. Not tested.
- These values are invented or assumed, not checked:
  - Ossicle's skip intervals (15 s back, 30 s forward) and its speed range.
  - Whether Pyxis reports mono/stereo for a Sonos session.
  - Whether Firefox's MPRIS gives a length for a YouTube video.
  - Whether mpv publishes MPRIS (it needs the mpv-mpris script).
- All data is fake: titles, counts, devices, group names, the 1,240-item queue and the jump results. In the library, every Pyxis album opens "Dots and Loops", and every Ossicle show list ends at "(fake data ends here)".
- The fake spectrum is a deterministic function, not an FFT. Peak physics follows the research (hold, then accelerating fall), with my own constants.

## Suggested issues (for the parent to file)

1. Pyxis contract: a call that lists library groups per source (this mock assumes one). The brief already notes the gap. High.
2. Ossicle: confirm the skip intervals, the speed range, and how played flags sync. Medium.
3. Font: ship W95FA Grid with the W95FA copyright notice and OFL 1.1 text (licence checked; no Reserved Font Name). Low.
4. Done in pass 2: the prompt line, for preset and playlist names.

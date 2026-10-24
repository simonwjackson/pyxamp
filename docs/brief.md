# Pyxamp design brief

Settled with Simon in a design interview on 2026-10-24. Terms are defined in `CONTEXT.md`. Hard-to-reverse choices are in `docs/adr/`. Research behind it: `~/.cache/research/winamp-pyxis-20261024T153523/`.

## What it is

An everyday desktop audio player for Pyxis (music), Jellyfin (music, video by hand-off) and Ossicle (podcasts and narrated books). At a squint it is Winamp 2 with its default skin. Up close it has new features. It is not called Winamp and ships no Nullsoft art.

## Look

- One redrawn default skin, pixel by pixel in Winamp 2's style. No `.wsz` skin support (ADR 0002).
- One surface: main, EQ and playlist sections in Winamp's docked column. EQ and PL fold sections in and out. Windowshade and Ctrl+D double size stay (ADR 0004).
- Scale is the monitor's scale rounded to a whole number; Ctrl+D doubles it again.
- Only popup menus, tooltips, a handed-off Jellyfin video, and later MilkDrop appear outside the surface. The jump box and item info are drawn inside the playlist section.

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

## EQ

10 bands plus preamp, applied to Pyxamp's own audio only.

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

Open items, not designed yet: text entry inside the surface (EQ preset names, station URLs) and the retry policy for errors.

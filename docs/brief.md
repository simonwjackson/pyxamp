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

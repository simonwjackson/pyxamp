# Pyxamp

Pyxamp is Simon's everyday desktop audio player. At a squint it looks like classic Winamp 2. It plays from his own services and shows what other players on the desktop are doing.

## Language

### Sources and queues

**Source**:
A service that Pyxamp plays from: Pyxis (music), Jellyfin (music, and video by hand-off), or Ossicle (podcasts and narrated books).
_Avoid_: Backend, provider, plugin

**Active source**:
The one source that Pyxamp plays from at a given moment. Exactly one source is active.

**Queue**:
The play order that the active source owns. Pyxamp shows a queue and edits it through the source. It never keeps a queue of its own.
_Avoid_: Playlist (when you mean the data), up-next

**Playlist editor**:
The Winamp window that shows the active source's queue.

**Spoken word**:
Items from Ossicle. They have a synced resume position, a playback speed, skip intervals, and a played flag. Music does not have these.
_Avoid_: Audio content, episodes (when books are included)

**Library window**:
The classic Winamp 2 window, drawn in the Minibrowser's frame, that lists the three sources as a drill-down (Pyxis > Albums > an album). It docks with the other windows. The playlist editor can switch to the same view.
_Avoid_: Media Library, minibrowser, browser

**Jump box**:
The small skinned search box that J opens. It searches all three sources at once and plays or queues the result.
_Avoid_: Search window, omnibox

**Hand-off**:
Opening a Jellyfin video in a separate player window outside Pyxamp. Pyxamp lists the video but never draws it.

### Pyxis roles

**Host**:
The device that renders a Pyxis session's audio and owns its transport. Pyxamp is a host when it plays Pyxis audio itself.

**Console**:
A device that shows and controls a Pyxis session that a different host plays. Pyxamp is a console when it drives a session that plays on Sonos or another computer.

### The desktop

**Foreign player**:
Any other media player on the desktop that publishes itself over MPRIS, such as a browser tab that plays YouTube.
_Avoid_: External app, other source

**Followed player**:
The player whose title, time and transport Pyxamp's main window shows and controls. This is Pyxamp's own playback, or a foreign player.

**Yield**:
Pyxamp pauses its own playback because a foreign player started playing.

**Short interruption**:
A foreign playback that ends within the resume limit. After a short interruption, Pyxamp resumes on its own. After a longer one, Pyxamp stays paused.

**Resume limit**:
The longest foreign playback that still counts as a short interruption: 2 minutes.

# Pyxamp

Pyxamp is Simon's everyday desktop audio player. At a squint it looks like classic Winamp 2. It plays from his own services and shows what other players on the desktop are doing.

## Language

### Sources and queues

**Source**:
A service that Pyxamp plays from: Pyxis (music), Jellyfin (music, and video by hand-off), or Ossicle (podcasts and narrated books).
_Avoid_: Backend, provider, plugin

**Active source**:
The one source that Pyxamp plays from at a given moment. Exactly one source is active. A source that stops being active is paused at that source and keeps its place.

**Queue**:
The play order that the active source owns. Pyxamp shows a queue and edits it through the source. It never keeps a queue of its own.
_Avoid_: Playlist (when you mean the data), up-next

**Pyxamp window**:
The one surface Pyxamp draws. It stacks three sections in Winamp's docked column: the main section, the EQ section and the playlist section. The EQ and PL buttons fold sections in and out.
_Avoid_: Main window (for the whole surface), player window

**Playlist section**:
The bottom section of the Pyxamp window. It shows the queue or the library view, and holds the jump box and item info.
_Avoid_: Playlist editor, playlist window

**Spoken word**:
Items from Ossicle. They have a synced resume position, a playback speed, skip intervals, and a played flag. Music does not have these.
_Avoid_: Audio content, episodes (when books are included)

**Library view**:
The drill-down list of the three sources (Pyxis > Albums > an album) that the playlist section shows in place of the queue.
_Avoid_: Media Library, minibrowser, library window

**Jump box**:
The search line that J opens over the playlist section. It searches all three sources at once and plays or queues the result.
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
The player whose title, time and transport Pyxamp's main section shows and controls, and that the media keys act on. It is the player that started playing most recently, or Pyxamp's own playback when no foreign player plays.

**Yield**:
Pyxamp pauses its own playback because a foreign player started playing.

**Short interruption**:
A foreign playback that ends within the resume limit. After a short interruption, Pyxamp resumes on its own. After a longer one, Pyxamp stays paused.

**Resume limit**:
The longest foreign playback that still counts as a short interruption: 2 minutes.

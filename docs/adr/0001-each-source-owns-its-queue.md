# Each source owns its queue; one source plays at a time

Pyxis, Jellyfin and Ossicle each already keep a queue that their other apps and devices see: the Pyxis session queue, Jellyfin play queues, and Ossicle's synced up-next. Pyxamp shows and edits the active source's own queue and never keeps a mixed queue of its own. With a mixed queue, Pyxamp would become a fourth owner, the phone apps and Sonos would disagree with it, and Pyxis console control would stop working. The cost is the Winamp habit of putting anything into one playlist: switching source switches the playlist.

## Exception: Jellyfin (2026-10-25)

Jellyfin keeps no play queue that its other clients share: an API client gets only per-session remote control and SyncPlay groups. So the Jellyfin connector keeps the Jellyfin queue itself, inside the daemon. It is still one source's queue, never mixed with another source, and switching source still switches the playlist. The cost: Jellyfin's own apps do not see this queue, and it does not survive a daemon restart.

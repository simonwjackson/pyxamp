# Step 2–4 seam

This seam let five workers build Pyxamp's own playback and its three sources at the same time. It fixes the wire, the daemon module boundaries, the configuration and the fixtures. The seam commit (`pyxamp-seam`) had stub modules; all of them are now built. Decisions made during the build are under [Build decisions](#build-decisions). The brief (`docs/brief.md`), terms (`CONTEXT.md`), ADRs and the approved mockup (`docs/mock/`) stay settled.

## Wire

`daemon/src/protocol.rs` is canonical. `contracts/pyxamp.schema.json` and `face/lib/wire-schema.js` are generated (`cargo run --bin pyxamp-schema -- --write`, `face/tools/gen-wire-validators`). Never edit them by hand. The face and daemon ship in one package, so the additions keep protocol version 1.

The snapshot keeps its step-1 fields. `player` still means the followed **foreign** MPRIS player and only `mpris.rs` writes it. Step 2 adds:

| Field | Meaning | Written by |
|---|---|---|
| `library` | `noSources`, or `sources` with one `SourceSummary` per configured source: status (`connecting`, `ready`, `unavailable`) and first-level `groups` exactly as the source reports them | session, from each source |
| `queue` | The active source's queue: one `QueueItem` per occurrence, with `albumKey` for album mode and `status` (`pending`, `current`, `played`, `retrying`, `failed`) | active source |
| `own` | The active source's session (`OwnPlayback`): output (local or a remote device), track and measured format, transport, capabilities, position, volume, repeat, shuffle, speed, skip intervals, retry, `yielded` (resume deadline, or absent for "stays paused"), display `note` | active source; session adds `yielded` |
| `outputs` | Pyxis "Play on" outputs | active source |
| `conflicts` | Ossicle position conflicts | active source |
| `engine` | Own engine status | engine |
| `systemVolume` | The default audio sink's `volume` (wpctl's cubic scale, 0..3; above 1 is boost) and `muted`; absent when unknown | `system_volume.rs` |

The face shows `player` while a foreign player is followed (amber), else `own`. Unknown values stay absent.

New client message `request` `{ id, action }`. The daemon always answers with `result` (`accepted` or `rejected`). `library.open` and `search` first send a `page` `{ id, source?, node?, query?, items, nextCursor?, total? }` with the same id.

| Action | Payload | Routed to |
|---|---|---|
| `library.open` | `source`, `node?` (absent: root groups; else a group id or an openable item id), `cursor?` | that source |
| `search` | `query` | every ready source, merged, 5 s per source |
| `item.play` | `source`, `item`, `mode` (`now`, `next`, `last`) | that source; becomes active |
| `station.start` | `source`, `seed` | that source; becomes active |
| `queue.jump`, `queue.remove` | `occurrence` | active source |
| `output.select` | `source`, `output` | that source |
| `playlist.save` | `source`, `name` | that source |
| `playlist.add` | `source`, `playlist`, `item` | that source |
| `item.markPlayed` | `source`, `item`, `played` | that source |
| `conflict.resolve` | `source`, `conflict`, `keep` | that source |
| `own.transport` | `action` (`play`, `pause`, `playPause`, `stop`, `previous`, `next`) | active source |
| `own.seek` | `positionMs`, `occurrence` (fences a stale seek) | active source |
| `own.volume`, `own.repeat`, `own.shuffle`, `own.speed` | as named; speed 0.5–3.0 | active source |
| `eq.set` | `on`, `preampDb`, `bandsDb[10]`, all −12…12 | engine |
| `balance.set` | `value` −1…1 | engine |
| `system.volume` | `value` 0…1 | session: `wpctl set-volume @DEFAULT_AUDIO_SINK@` |

Switching sources: before a play or station on another source, the session sends the old source `own.transport pause` with an id starting `internal:`. The old source pauses at its end and keeps its place and its session, so switching back resumes in place.

`pyxampctl request '<payload>'` sends one request and prints the page and result.

## Daemon modules

```
daemon/src/config.rs          sources.toml loading, secret files
daemon/src/engine/mod.rs      EngineHandle, EngineCommand, EngineState, EngineEvent
daemon/src/engine/mpv/        engine::mpv::run(EngineContext, active watch)
daemon/src/sources/mod.rs     SourceContext, SourceHandle, SourceState, SourceSession, retry_delay, is_hand_off
daemon/src/sources/pyxis/     sources::pyxis::run(SourceContext<PyxisConfig>)
daemon/src/sources/ossicle/   sources::ossicle::run(SourceContext<OssicleConfig>)
daemon/src/sources/jellyfin/  sources::jellyfin::run(SourceContext<JellyfinConfig>)
daemon/src/session.rs         routing, active source, snapshot merge
```

A source is an actor. It publishes `SourceState` (summary, plus its session) on `ctx.state`, answers each `SourceRequest` exactly once, and only drives the engine while `ctx.active` names it. Engine requests carry `owner: Some(source)`; the session uses `None` for EQ, balance and yield. `token` names one load; ignore events for an old token. A source that plays elsewhere (Pyxis console mode) does not use the engine.

`main.rs` starts the engine, one actor per configured source, and the session. Workers keep the `run` signatures and do not edit `main.rs`, `lib.rs`, `socket.rs`, `protocol.rs`, `config.rs` or `sources/mod.rs`. If one of these must change, ask the parent; the parent changes the seam and every worker rebases.

Shared HTTP dependencies are already in `daemon/Cargo.toml`: `reqwest` (rustls, json, stream), `tokio-tungstenite` (rustls), `toml`, `url`. A worker may add a dependency only its own module uses. Say so in the report; the parent resolves `Cargo.lock` at merge.

## Configuration

`$PYXAMP_SOURCES`, else `$XDG_CONFIG_HOME/pyxamp/sources.toml`, else `~/.config/pyxamp/sources.toml`. A missing file or section means the source is absent. Inline secrets are refused. Mountainous writes this file and the secret files.

```toml
[pyxis]
url = "https://pyxis.example.ts.net"
token_file = "/run/secrets/pyxamp/pyxis-token"
device_name = "Myoko"            # optional

[ossicle]
url = "https://ossicle.example.ts.net"
token_file = "/run/secrets/pyxamp/ossicle-token"   # optional

[jellyfin]
url = "https://jellyfin.example.ts.net"
api_key_file = "/run/secrets/pyxamp/jellyfin-api-key"
user = "simon"
```

Read secrets with `config::read_secret` when connecting. Never log them.

## Fixtures

`face/tools/make-fixtures` writes the replay fixtures; `pyxamp-validate-fixtures` checks them through the canonical serde types. Step-2 fixtures: `library-sources` (with `pages`), `own-pyxis-playing`, `own-pyxis-console`, `own-yielded`, `own-ossicle-conflict`, `own-retry-failed`. A fixture's optional `pages` are the replay server's answers to `library.open` and `search`.

## Ownership

| Worker | Owns | Must not touch |
|---|---|---|
| engine | `daemon/src/engine/**`, `daemon/src/session.rs`, `daemon/src/presence.rs`, `daemon/src/policy.rs`, engine tests, mpv on the packaged daemon's PATH in `flake.nix` (Jellyfin video hand-off uses the same mpv) | sources, face |
| pyxis | `daemon/src/sources/pyxis/**`, its tests and test tools | other sources, engine, face |
| ossicle | `daemon/src/sources/ossicle/**`, its tests; Ossicle repo changes in its own worktree | other sources, engine, face |
| jellyfin | `daemon/src/sources/jellyfin/**`, its tests | other sources, engine, face |
| face | `face/**` except `face/lib/wire-schema.js` | daemon |

Tests go under `daemon/tests/<worker>_*.rs` or inside the owned module, so test files never collide.

## Build decisions

- **Protocol version.** The additions keep version 1: face and daemon ship together.
- **Retry.** The active source runs the retry: after an engine `Failed`, it retries after 2 s, then 10 s (`sources::retry_delay`), shows `own.retry` and `retrying`, and on a third failure marks the occurrence `failed` and moves on. The engine emits `Failed` exactly once per load and never retries or skips by itself.
- **Activation.** The engine reads the same `active` watch the session writes (`engine::mpv::run(ctx, active)`) and refuses a source that is not active with code `inactive`; owner `None` (the session) is always allowed. A source that must start without a request from this computer (a Pyxis console telling this host to play) bumps `SourceState::activation`; the session pauses the old source and activates it.
- **Play on before the first play.** `SourceState::outputs` carries a source's outputs; the snapshot shows them when no active session lists any.
- **Video hand-off.** A Jellyfin video opens its own mpv window with the mpv-mpris script from `PYXAMP_MPV_MPRIS`. `sources::is_hand_off` routes it without pausing or switching the active source and without cancelling a yield, so own music yields to the followed window and resumes after a short video. The engine's own mpv runs with `--no-config --load-scripts=no` and is never followed.
- **Engine hooks outside the engine's files.** `main.rs` creates the `active` watch before the engine and passes the session's request sender to `presence::Player`; `mpris.rs` falls back to `engine::own_capture` only while no foreign player is followed.
- **Pyxis U38.** "Play next" (`queue.add` with `index`) and seeking a station or search track need Pyxis U38; on an older core they fail with a clear reason.
- **System volume.** The main section's volume slider, Up and Down, and "VOLUME: n%" act on the system volume (the default PipeWire sink) in every state; they no longer set a followed player's or a source's own volume. `system_volume.rs` reads it on its own PipeWire connection, driven by events: the `default` metadata, sink `Props` and device `Route` params, chosen the way WirePlumber's mixer API (and `wpctl get-volume`) chooses. `system.volume` runs `wpctl` (WirePlumber is on the packaged daemon's PATH), one set at a time, 2 s each; a set still waiting when a newer one arrives is superseded and answered accepted. Pyxamp never sets the volume at start-up. Pyxamp's own MPRIS `Volume` is unchanged: it still mirrors and sets the shown player's volume (foreign player or own source), because an MPRIS client asks a player for that player's volume. `own.volume` and step-1 `volume.set` stay on the wire for MPRIS and `pyxampctl`.
- **Jellyfin queue.** Jellyfin has no play queue that its other clients share, so the Jellyfin connector keeps that source's queue (ADR 0001 records the exception).

## Acceptance per worker

- **engine:** isolated mpv plays local test files; gapless preload; EQ and balance change a known tone measurably; speed; own stream captured for the visualizer without mixing an unrelated stream; yield with a controlled clock (pause once, resume within 120 s, else stay paused); retry after 2 s and 10 s, then skip and mark failed; Pyxamp's own MPRIS reflects own playback when nothing foreign is followed; switching sources pauses the old one.
- **pyxis:** `pyxis-rpc-v3` with `x-pyxis-contract` and bearer token; realtime hello with `contractId`; host mode through the engine; console mode for a remote output; groups as reported; paging; seek, repeat, shuffle, playlist append and save, stations. Tested against a locally run Pyxis service, never the live server. The combined gate pins the Pyxis commit that will be deployed with this change (`PYXIS_GATE_REV` in `daemon/tools/verify`); that commit lands on Pyxis `main` before deployment.
- **ossicle:** Up next, Shows, Books; resume, speed, skip intervals, played flags, sponsor skip, conflicts, all through Ossicle; shuffle and repeat unsupported. Tested against a local Ossicle.
- **jellyfin:** Artists, Albums, Playlists, Videos; audio through the engine; video in a separate mpv window that is followed, so music yields; play and progress reports. Tested against a local instance or recorded fixture, no live writes.
- **face:** every step-2 view and state from the brief and mockup choices, against these fixtures, with real headless Quickshell screenshots and interaction checks at 1x and 2x.

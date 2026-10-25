# Step 2–4 seam

This seam lets five workers build Pyxamp's own playback and its three sources at the same time. It fixes the wire, the daemon module boundaries, the configuration and the fixtures. It has no behaviour: every new module is a stub that reports "unavailable". The brief (`docs/brief.md`), terms (`CONTEXT.md`), ADRs and the approved mockup (`docs/mock/`) stay settled.

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

Switching sources: before a play or station on another source, the session sends the old source `own.transport pause` with an id starting `internal:`. The old source pauses at its end and keeps its place and its session, so switching back resumes in place.

`pyxampctl request '<payload>'` sends one request and prints the page and result.

## Daemon modules

```
daemon/src/config.rs          sources.toml loading, secret files
daemon/src/engine/mod.rs      EngineHandle, EngineCommand, EngineState, EngineEvent
daemon/src/engine/mpv/mod.rs  engine::mpv::run(EngineContext)          stub
daemon/src/sources/mod.rs     SourceContext, SourceHandle, SourceState, SourceSession
daemon/src/sources/pyxis/     sources::pyxis::run(SourceContext<PyxisConfig>)       stub
daemon/src/sources/ossicle/   sources::ossicle::run(SourceContext<OssicleConfig>)   stub
daemon/src/sources/jellyfin/  sources::jellyfin::run(SourceContext<JellyfinConfig>) stub
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

## Acceptance per worker

- **engine:** isolated mpv plays local test files; gapless preload; EQ and balance change a known tone measurably; speed; own stream captured for the visualizer without mixing an unrelated stream; yield with a controlled clock (pause once, resume within 120 s, else stay paused); retry after 2 s and 10 s, then skip and mark failed; Pyxamp's own MPRIS reflects own playback when nothing foreign is followed; switching sources pauses the old one.
- **pyxis:** `pyxis-rpc-v3` with `x-pyxis-contract` and bearer token; realtime hello with `contractId`; host mode through the engine; console mode for a remote output; groups as reported; paging; seek, repeat, shuffle, playlist append and save, stations. Tested against a locally run Pyxis service from Pyxis `main`, never the live server.
- **ossicle:** Up next, Shows, Books; resume, speed, skip intervals, played flags, sponsor skip, conflicts, all through Ossicle; shuffle and repeat unsupported. Tested against a local Ossicle.
- **jellyfin:** Artists, Albums, Playlists, Videos; audio through the engine; video in a separate mpv window that is followed, so music yields; play and progress reports. Tested against a local instance or recorded fixture, no live writes.
- **face:** every step-2 view and state from the brief and mockup choices, against these fixtures, with real headless Quickshell screenshots and interaction checks at 1x and 2x.

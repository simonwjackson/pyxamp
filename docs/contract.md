# Approved build contract: Pyxamp step 1 and Pyxis support

Workflow run: 8145e0180c15fe474023427dc4a22b01bac7905e2353db76ceb41a66752f91aa. The user approved the Contract stage. This document carries the approved contract and concrete field conventions for parallel implementation. It is not a workflow file.

## Repositories and settled design

- Pyxamp: /home/simonwjackson/code/github/simonwjackson/pyxamp, approved base 3008b9e. Read docs/brief.md, CONTEXT.md, docs/adr/*.md, and docs/mock/REPORT.md. The brief and mock are settled; do not reopen them. Step 1 is a universal remote, not own playback.
- Pyxis: /home/simonwjackson/code/github/simonwjackson/pyxis, starting base a3be14a. Read AGENTS.md and the whole work/items/active/20260821123211-pyxis-v2-rewrite/plan.md and its Decision Log, work.md, and contracts/generated/pyxis.ts. New support units belong in that rewrite plan.
- Mountainous: /home/simonwjackson/code/github/simonwjackson/mountainous/.worktrees/cairn. No worker edits it. No deployment, publication, pushes, or landing in this stage. Never run nix flake check anywhere, including wrappers or scripts.
- Bedrock engineering documents apply. Test adapters are real contract implementations with configurable data/behavior, not Mock*/Stub*/Fake* classes. Each worker runs focused checks only. The parent runs full combined verification once per build/review stage.

## File ownership

face worker: face/ in its own Pyxamp worktree, including QML, face tests, fixture replay, screenshot tools and generated sprites/font assets/notices under face/. Do not edit daemon/, root build files, docs/ or nix/.

daemon worker: daemon/, contracts/ (generated socket schema), root Rust/build/tooling metadata, flake.nix, flake.lock and nix/ in its own Pyxamp worktree. Also record this contract at docs/contract.md and add accurate build/test README instructions. Do not edit face/ or overwrite the brief, glossary, ADRs or mock. Keep codegen/tools for socket schema in daemon/ so its ownership is clear.

pyxis worker: its own Pyxis worktree only. Do not edit Pyxamp or Mountainous. One commit per new rewrite-plan unit with its tests and plan update.

All workers create a worktree in their target repository, work and commit there, and report absolute worktree path, branch, base SHA, commit SHAs, checks and limitations. The default harness worktree is Mountainous and is NOT the target repository.

## Pyxamp wire contract (version 1)

A same-user Unix socket at $XDG_RUNTIME_DIR/pyxamp/control.sock. Create its directory mode 0700 and socket 0600. Bind only within the current user's runtime directory. Do not silently fall back to a world-accessible /tmp socket or unlink a live daemon socket. Permit fragmented reads/multiple lines per read. Bound line length (1 MiB) and outgoing queues; a malformed client must not kill the daemon.

Messages are UTF-8 NDJSON with an explicit `_tag` and `payload`. The daemon's Rust serde/schemars types are canonical; generate contracts/pyxamp.schema.json from them with a drift check. QML consumes those shapes, not a different hand-written protocol. No required message is inferred from mock JavaScript internals.

Client messages (complete step-1 set):
- hello: {version:1}. Must be first on each connection. No commands or state subscription before it.
- command: {id:string,target:{busName:string,owner:string},action:{_tag:...,payload:...}}. IDs are nonempty, at most 128 characters. Every action carries the target shown when the gesture began. Owners are unique D-Bus owner names, not just reusable service names.

Action variants (all payloads are objects):
- transport.play {}, transport.pause {}, transport.playPause {}, transport.stop {}, transport.previous {}, transport.next {}.
- transport.seek {positionMs:nonnegative integer,trackId:string}. Absolute position. The track ID must still match mpris:trackid at execution.
- volume.set {value:number}, range 0..1, applied to that player's MPRIS Volume, never the default system sink.
- repeat.set {mode:"off"|"all"|"one"}; shuffle.set {enabled:bool}.

Server messages (complete step-1 set):
- ready {version:1,instanceId:string}. Reply to supported hello; instanceId changes on daemon restart.
- state {revision:nonnegative integer,player:PlayerState,library:{_tag:"noSources"},queue:[]}. Send complete snapshot immediately after ready and on state changes. A new instance resets revision. There are no actual queue or library items in step 1.
- visualizer {target?:{busName,owner},sequence:nonnegative integer,status:"ready"|"idle"|"unavailable",columns:number[75],waveform:number[75],reason?:string}. Values are integers 0..255. columns are spectrum amplitude values; waveform is centered at 128. At most 60 frames/s, latest frame replaces unsent old frames. Idle/unavailable frames have columns zero and waveform 128. The UI must distinguish no signal from unavailable capture; do not invent animated data.
- result {id:string,outcome:{_tag:"accepted",payload:{}}|{_tag:"rejected",payload:{code:string,message:string}}}. Accepted means the native remote operation completed without a D-Bus error, NOT that desired transport state was observed. State always comes from actual MPRIS observations, not optimistic mutation.
- error {code:string,message:string}. Used for framing/handshake/schema failures; unsupported protocol closes the connection. Command-specific errors use result and do not disconnect a healthy client.

PlayerState is one tagged union:
- {_tag:"idle",payload:{}} for no player to follow.
- {_tag:"unavailable",payload:{code:string,message:string}} for D-Bus/service failure.
- {_tag:"following",payload:{target:{busName,owner},identity:string,track:Track,transport:"playing"|"paused"|"stopped"|"unknown",capabilities:Capabilities,positionMs?:integer,volume?:number,repeat?:"off"|"all"|"one",shuffle?:bool,rate?:number}}.
Track fields: title:string, artist:string, id?:string, album?:string, durationMs?:integer, artworkUrl?:string, bitrateKbps?:number, sampleRateHz?:number, bitDepth?:number, channels?:number. Missing metadata stays absent; never infer stereo/lossless/bit depth merely to light a mock indicator. A missing duration makes remaining-time/seek unsupported.
Capabilities has booleans play,pause,stop,previous,next,seek,volume,repeat,shuffle. Map to actual player support. The face dims unsupported controls and exposes the reason, rather than silently pretending they work.

The face extrapolates observed position only while playing using its local monotonic elapsed time and playback rate; clamp to known duration. The daemon periodically refreshes actual Position, no more than 4 times/s, and handles Seeked. On daemon loss the face discards control authority, shows DAEMON DISCONNECTED and reconnects for a fresh snapshot. Never automatically replay a non-idempotent action after reconnect. Reject noTarget, staleTarget, staleTrack, unsupported, invalid and native failures as typed errors.

## Daemon behavior

Use Rust with zbus for MPRIS and pipewire-rs for capture. Register org.mpris.MediaPlayer2.pyxamp, with Identity Pyxamp, and mirror the followed player's supported metadata/properties. Forward media controls only to the captured real target. Exclude all of Pyxamp's own MPRIS identities from discovery and event handling. No forwarding loops.

Follow the most recent actual transition into Playing, not metadata changes. When it stops, pauses or disappears, follow the most recently active still-playing player. With none playing, return to own playback, which is idle in step 1. On initial discovery with several playing services, use a deterministic bus-name tie break; MPRIS does not provide their start times. Process removal and D-Bus owner changes invalidate old commands. Track the selected player's capabilities and metadata changes.

Yield policy is pure, clock-driven and tested with a controllable real playback adapter. When own playback was playing, a foreign start pauses it once. A continuous chain of overlapping foreign playback is one interruption. Resume only after all foreign playback ends within 120 seconds of the first foreign start. Explicit user cancellation, own source/session change or expired time cancels automatic resume; never restart music that was already paused. No actual own audio/mpv adapter in step 1.

Visualizer captures the followed player's actual PipeWire stream(s), never a default-sink monitor or whole desktop mix. Match using live process/application identity and owner metadata; multiple matching nodes for the SAME known player can be handled together. Ambiguous browser tabs/process mappings must report unavailable rather than capture the wrong sound. Clear samples on target change. A disappearing node cannot leave old bars attributed to a new player. Port FFTNullsoft from Webamp/WACUP and preserve Nullsoft's BSD-3 copyright/license with provenance, plus notices for any MIT code reused. No code from the restricted 2024 Winamp source release.

## QML face behavior

A separate `qs -c pyxamp` config. One layer-shell surface with main/EQ/playlist sections stacked like docs/mock. The face does not use Quickshell Mpris as a second state owner. It consumes daemon snapshots and sends commands.

Use the existing original pixel maps in docs/mock/skin.js to generate sprites reproducibly. No Nullsoft/Webamp bitmap assets. Draw at integer physical scale, nearest neighbor; account for compositor scale exactly once. Compare at 1x and 2x, and test monitor-fit policy and Ctrl+D. The mock's public screenshot is reference evidence, not permission to hard-code screenshot pixels as UI.

Amber following display, EQ note NO EQ ON <APP>, real spectrum and oscilloscope/off, elapsed/remaining clock, scrolling title, native-looking menus/tooltips, windows shaded/folded instantly. Reduced motion stops title scrolling and starts with VIS OFF until the user selects a mode. Everything else follows the brief and mock decisions 1-40. At rest the mock says no sources yet in library, queue and jump box; no fake content in production.

Quickshell IPC target `pyxamp`, method `toggle`: `qs -c pyxamp ipc call pyxamp toggle`. Show on focused monitor at remembered position, hide instantly. A changes top/bottom layer. Close/minimize hide only. Persist local non-sensitive view preferences and per-monitor positions across restart. Persisted data needs no service credentials in step 1.

Playback controls target the followed player; unsupported actions remain disabled. J/L open noSources views inside playlist section; I displays actual followed-item metadata there. Keys must not leak through any active text input/menu. Future source mock states are rendered only through an explicit fixture/replay interface sharing the production state adapter, not enabled by production environment detection.

## Pyxis support units

Add milestone/units to the existing rewrite plan with one atomic commit per unit and tests in that commit. Update VISION with exactly the user's accepted principle: 'Not a playlist manager: playlists exist as a simple primitive, a named list of tracks, but albums are the unit and placements organise the library. No surface promotes playlists or makes them the hub. Stations and source playlists are discovery tools.'

Keep existing POST /rpc, realtime, authentication/scopes, tagged-union/typed-outcome conventions. contract.rs is canonical; regenerate generated TypeScript and JSON Schema. This is a breaking wire change: choose a new contractId consistently (proposed pyxis-rpc-v3, a wire version, not another rewrite). Cached old clients must refuse mismatched contracts; never silently discard old queue.shuffle outbox intent. Define deployment/update handling and document any blocker rather than silently losing pending writes. Persisted current-v2 sessions must reopen with sensible default modes without losing queue/cursor; no v1 compatibility code.

- library.groups.list {} returns ordered records {id,label,kind,count}, core ordering album groups (all albums/placements, computed Hot, tags), Bookmarks, Stations, Playlists. IDs opaque to clients. Counts reflect true membership; never invent counts for failed external sources. Explicit typed availability/failure is allowed when a station provider cannot answer. Zero plugins remains valid.
- library.group.items.list {groupId,cursor?,limit} returns typed members plus opaque next cursor. IDs/cursors and data are account scoped. Cursors cannot be reused for another group/account. Counts and pages agree. No client-side parsing of labels to select a filter.
- Add persisted account-owned album tags, RpcLibraryAlbum.tags:string[], and a tags.set {tags:string[]} album command via existing library.album.command.run. Validate/normalize tags, update revisions, and preserve them across snapshots/offline sync and placement changes. Group membership must change when tags change, without client code edits. No enrichment providers or saved-filter language in this unit.
- transport.seek {positionMs} is a session command routed to the host. Validate finite/nonnegative/bounded position, known track/duration and ownership. The shared confirmed renderer and core-hosted Sonos adapter apply actual seek BEFORE publishing new session state. Stale directives/receipts, renderer refusals and failures preserve prior truth.
- repeat.set {mode:"off"|"all"|"one"}, shuffle.set {enabled:bool}; add session repeat and shuffle. Remove queue.shuffle throughout public contract, host/worker/SDK consumers and docs/tests. Preserve original queue order while shuffled, stable identity for repeated track occurrences, current playing occurrence and position on toggle, and correct add/remove/clear behavior. Repeat-one replays current at natural end; repeat-all wraps; off ends. Explicit next/previous is not trapped by repeat-one. Persist/replay/hand off the state correctly. Both web clients and core-hosted Sonos honor modes, with existing renderer-confirmed command/receipt safety. Do not issue physical Sonos commands during tests.
- library.playlist.tracks.add {playlistId,trackIds,commandId} appends through the authenticated library domain. Make it idempotent within account: identical replay returns prior outcome; same commandId/different payload rejects. Preserve ordered duplicate tracks when intentionally appended under a different commandId. Validate ownership and actual referenced tracks, bump revision atomically. Existing create/list stay. Do not add rename/delete/reorder feature scope.

Public reference-client affordances cover the new operations without visual redesign; product data plane/host capability handling consumes the new wire shapes. Tests cover real RPC/worker/public interfaces, not code-string assertions.

## Proof and check ownership

Workers: focused tests only, not repeated full test runs. Face: actual headless Quickshell screenshots and interaction checks at 1x and 2x using a configurable socket replay server; no screenshots from the HTML mock relabelled as QML. Daemon: protocol framing/schema and bad clients; isolated D-Bus real player fixture, controls/capabilities/owner changes; follow/yield with deterministic clock; known PCM/FFT; isolated PipeWire capture showing a second unrelated audio stream is not mixed in. State any missing environment proof honestly. Pyxis: targeted service/host/RPC/worker tests per unit, schema drift, and relevant static checks. New one-off automation goes in Nix-shebang scripts, never inline Python/Node/API scripts.

Parent runs the complete Pyxamp combined gate (cargo tests, formatter/clippy, protocol drift check, package build, QML screenshot/interaction gate) once after combining face and daemon, and Pyxis just verify plus separate product-client gates once after all support units. Review-stage full rerun is once after the final fixes, not per worker per fix. No nix flake check anywhere.

Default package should install the daemon, CLI control entrypoint and Quickshell face. Provide a daemon-only flake package attribute so the daemon worker can build its branch before face is merged; the parent's combined build tests the full default package. External sources in flakes use pinned flake inputs, no fetchurl/fetchFromGitHub.

## Explicit non-goals in this stage

No own mpv playback, actual Pyxis/Ossicle/Jellyfin adapters in Pyxamp, MilkDrop, production source credentials, offline downloads, GitHub repo creation, pushes, mountainous changes, deployment or landing. All implementation branches wait for Review and the Land stop. Document operational limits and future integration seams without implementing placeholder audio behavior.

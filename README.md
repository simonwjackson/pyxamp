# Pyxamp

Step 1 is a universal MPRIS remote. It has no own audio, sources, queue items, library items, mpv process, or production playback fixture. See [the approved contract](docs/contract.md), [brief](docs/brief.md), and [terms](CONTEXT.md).

## Build

Linux, a session D-Bus, PipeWire, and a private `$XDG_RUNTIME_DIR` are required. Nix inputs and Rust dependencies are pinned in `flake.lock` and `Cargo.lock`.

```sh
# Standalone daemon branch (does not need face/).
flock /tmp/pyxamp-wf8145-build.lock nix build .#daemon --max-jobs 1 --cores 2
# Combined package, after face/ is merged.
flock /tmp/pyxamp-wf8145-build.lock nix build .#default --max-jobs 1 --cores 2
```

The daemon package installs `pyxamp-daemon`, `pyxampctl`, and `pyxamp-schema`. The combined package also installs self-contained `face/` in `etc/xdg/quickshell/pyxamp`. The `pyxamp` launcher prepends the package's `etc/xdg` to `XDG_CONFIG_DIRS`, preserves existing entries (or `/etc/xdg` when unset), and runs `qs -n -c pyxamp` so a second launch does not create another surface. The combined build fails if the face is missing. Licenses and generated wire schema ship in `share/doc/pyxamp/`.

Run `pyxamp-daemon` in the graphical session, then `pyxamp`. `pyxampctl state` prints a fresh observed snapshot. `pyxampctl watch` prints server messages. For command-line controls, pass the **captured target from the gesture**, not just a reusable service name:

```sh
pyxampctl command '{"id":"gesture-1","target":{"busName":"org.mpris.MediaPlayer2.example","owner":":1.42"},"action":{"_tag":"transport.next","payload":{}}}'
```

The optional `homeManagerModules.default` module exposes `programs.pyxamp.enable`. It installs the combined package, registers `qs -c pyxamp`, and starts a user daemon with the graphical session. Configure the desktop shortcut separately:

```sh
qs -c pyxamp ipc call pyxamp toggle
```

Without the Home Manager module, expose the package's `etc/xdg` in the desktop session's `XDG_CONFIG_DIRS` before using bare `qs -c pyxamp` IPC commands. Fleet integration will do this separately. `PYXAMP_SOCKET` and `PYXAMP_FACE_STATE` are fixture-only face overrides, not production configuration.

No installation, deployment or changes to desktop media-key bindings happen at build time.

## Focused development checks

Run commands from this repository root. Set `CARGO_BUILD_JOBS=2`. All compilation and package builds share the workflow lock.

```sh
nix develop
export CARGO_BUILD_JOBS=2
cargo fmt --all -- --check
flock /tmp/pyxamp-wf8145-build.lock cargo check --workspace --all-targets
flock /tmp/pyxamp-wf8145-build.lock cargo clippy --workspace --all-targets -- -D warnings
flock /tmp/pyxamp-wf8145-build.lock cargo test -p pyxamp-daemon --lib protocol::tests
flock /tmp/pyxamp-wf8145-build.lock cargo test -p pyxamp-daemon --lib policy::tests
flock /tmp/pyxamp-wf8145-build.lock cargo test -p pyxamp-daemon --lib fft::tests
flock /tmp/pyxamp-wf8145-build.lock cargo test -p pyxamp-daemon --lib capture::tests
flock /tmp/pyxamp-wf8145-build.lock cargo test -p pyxamp-daemon --test socket
flock /tmp/pyxamp-wf8145-build.lock dbus-run-session -- cargo test -p pyxamp-daemon --test mpris --test capabilities -- --test-threads=1
flock /tmp/pyxamp-wf8145-build.lock cargo run --bin pyxamp-schema -- --check
```

`daemon/src/protocol.rs` is canonical. Regenerate `contracts/pyxamp.schema.json` with `cargo run --bin pyxamp-schema -- --write`; do not edit the schema. New files must be git-added before Nix evaluates the flake.

`daemon/tools/smoke` exercises the real daemon, CLI handshake/snapshot, own MPRIS Identity, and duplicate-instance protection on a private D-Bus/runtime directory. Set `PYXAMP_SMOKE_BIN_DIR="$PWD/result-daemon/bin"` to exercise the packaged build instead of local Cargo binaries.

`daemon/tools/browser-probe /absolute/path/to/chromium` investigates a stock browser in private Xvfb/D-Bus/PipeWire/PulseAudio services, using normal HTML media and MediaSession APIs. Build the daemon, then set `PYXAMP_PROBE_DAEMON="$PWD/target/debug/pyxamp-daemon" PYXAMP_BROWSER_EXPECT_CAPTURE=1` to assert native one-tab capture, two-tab refusal, recovery, and exclusion of an unrelated `pw-play` tone. The default probe daemon is the packaged `result-daemon/bin/pyxamp-daemon`. An explicit `PYXAMP_BROWSER_URL` HTTPS URL instead probes actual remote-site playback; this network check is not part of the hermetic combined gate. Artifacts remain in the printed private `/tmp/px-browser-*` directory. No user profile, desktop audio or production D-Bus is used.

`daemon/tools/test-pipewire` starts private PipeWire, WirePlumber and D-Bus services in a temporary runtime directory. It plays 750 Hz and 6000 Hz in separate configured processes and verifies that targeted capture excludes the other tone, clears samples on target change, and clears the display when a node disappears. It does not connect to production audio. `KEEP_AUDIO_LOGS=1` retains its logs. Its ignored fixture test must not be invoked against a user's audio server.

## Combined parent gate

After merging daemon and face, run **`daemon/tools/verify`** once. It runs formatting, Clippy, all Cargo tests under isolated D-Bus, schema drift, private PipeWire capture proof, the combined default package build, a private-session smoke test of the packaged daemon/CLI, and executable `face/tools/verify`. The gate supplies `QS` from the default package's `quickshell` passthru (currently Quickshell 0.3.2 in the pinned nixpkgs). It verifies the production runtime, not a different `qs` from PATH or a store search. A missing or incompatible packaged runtime fails the gate. Missing face verification is an error, not a skip. Do not run `nix flake check`.

## Runtime behavior and limits

- The socket is `$XDG_RUNTIME_DIR/pyxamp/control.sock`. Directory mode is 0700; socket mode is 0600. A lock prevents two daemons from unlinking a live socket. No `/tmp` fallback exists. Handshake, UTF-8 JSON framing, line size, clients, command queues and writes are bounded. Reconnect gets a fresh complete snapshot, never replays commands.
- `org.mpris.MediaPlayer2.pyxamp` has Identity `Pyxamp`. It mirrors observed foreign-player properties and forwards controls to the captured unique owner. All Pyxamp identities and owner aliases are excluded. Accepted means only native D-Bus completion. State is never optimistic. Position is read on a 250 ms clock, and native Seeked updates are handled.
- Actual transitions into Playing choose the followed player. Startup ties use ascending bus name. Paused/stopped/removed players fall back to a still-playing player, otherwise idle. A pure, supplied-clock yield policy is tested with a controllable session playback implementation; production own playback remains idle.
- Capture matches live PipeWire client/node process identity, including descendant audio processes, and rejects non-playback nodes. It never selects a default-sink monitor. Exactly one process-matched playback stream is required; multiple streams are unavailable, not combined. Missing/ambiguous identity, negotiation or server failures produce **unavailable**, with zero columns and centered waveform.
- One unambiguous native browser stream can be captured through its live D-Bus owner PID and audio-service descendant PID. No custom annotations or browser extension are required. Native browser streams generally do not identify a tab: two process-matched playback streams are ambiguous, even if one is corked/idle. This single-stream rule applies to **every** player, not a browser-name heuristic. Multiple MPRIS identities sharing a process or conflicting application identity are also unavailable. The cost is unavailable capture during native crossfades, cached old streams, or other multi-stream playback. See [native browser proof and limits](daemon/tests/browser-capture.md).
- Nullsoft FFT provenance and complete BSD-3/MIT notices are in `daemon/licenses/`. The 1024-sample analyzer sends 75 spectrum values and 75 waveform values. Frames are latest-only and emitted no faster than 60 Hz. No invented animation or Winamp bitmap assets ship.
- Unsupported properties stay absent; no codec, bit depth, channel count, bitrate or duration is guessed. Capability booleans gate native controls. Source integrations, EQ on own audio, and own playback are later work.

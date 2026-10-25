# Native browser capture investigation

## Correction

The initial annotation requirement was hypothetical, not a standard browser API. Stock Chromium 150.0.7871.128 and 155.0.8059.39 published neither `media.mpris.bus-name` nor `media.mpris.owner`. Requiring these fields rejected their usable single streams. The daemon no longer requires or invents these fields.

## Verified native facts

`daemon/tools/browser-probe` uses a fresh stock Chromium profile on a private Xvfb display and private D-Bus, PipeWire, WirePlumber and pipewire-pulse servers. Normal HTML audio and MediaSession APIs produce real browser-owned MPRIS properties and real native audio streams. The probe does not implement/intercept MPRIS for the browser, insert stream metadata, install an extension, or substitute audio buffers.

Both tested Chromium builds exposed:

- `org.mpris.MediaPlayer2.chromium.instance<PID>` owned by the browser process.
- `Stream/Output/Audio` nodes with `application.process.id` pointing to a **child** process whose native command line includes `--utility-sub-type=audio.mojom.AudioService`.
- Generic `media.name = Playback`, without a tab identifier or either custom annotation.
- One node for one audible tab, two nodes sharing the same audio-service PID for two audible tabs. The browser's MPRIS metadata selects the most recent tab but does not identify which generic PipeWire node represents it.

This permits one process-matched stream, not selection among multiple streams. The rule must apply to every application: a renamed browser must not become eligible for multi-stream mixing merely because its name is unfamiliar. Native crossfades and cached/corked streams can therefore make capture temporarily unavailable. Multiple MPRIS identities sharing a process are also ambiguous. No default-sink fallback exists.

## Actual capture proof

The local pages play modulated 750 Hz and 6000 Hz PCM through the browser. A separate native `pw-play` process plays 12000 Hz into the same private sink throughout. Thus a system-sink mix would leak an unrelated tone.

| Stock browser | One tab | Two audible tabs | Close second tab |
|---|---|---|---|
| Chromium 150.0.7871.128 | ready, column 2 varies 29–58; columns 18/37 stay 0 | unavailable, all columns 0 and waveform 128 | ready; 29–58, unrelated columns 0 |
| Chromium 155.0.8059.39 | ready, column 2 varies 29–58; columns 18/37 stay 0 | unavailable, all columns 0 and waveform 128 | ready; 29–58, unrelated columns 0 |

Initial artifacts: `/tmp/px-browser-3qfhS2` (150), `/tmp/px-browser-UT9j26` (155). Each directory holds `summary.json`, complete PipeWire/PulseAudio data, actual process ancestry, socket snapshots/frames and service logs. Assertions check native stream counts, absence of invented annotations, moving spectrum, unrelated-tone exclusion, ambiguity clearing and recovery.

An actual remote-site probe also played `https://www.youtube.com/watch?v=jNQXAC9IVRw` in stock Chromium 155. Native MPRIS reported **Me at the zoo**, artist **jawed**, duration **19021 ms**. Browser video diagnostics reported `paused=false`, `readyState=4`, advancing playback and duration 19.021 seconds. One unannotated native audio-service stream produced changing socket spectrum values: column 2 ranged **25–162**, column 18 **1–13**. Artifacts: `/tmp/px-browser-XtkcBV`. This was the real YouTube site, not a local page labelled as YouTube.

The final name-independent single-stream policy was rerun with Chromium 155: the local one/two/one-stream assertions passed (`/tmp/px-browser-jHCg21`), then actual YouTube capture passed (`/tmp/px-browser-0Veblv`, column 2 ranged 16–255). One intervening repeat finished the 19-second clip during the observation window and correctly became idle (`/tmp/px-browser-NzaZBN`); the site harness now enables the standard HTML video `loop` property before observing. It does not replace the video source or audio output.

Reproduce from the repository root (supply an installed stock browser executable):

```sh
nix develop
export CARGO_BUILD_JOBS=2
flock /tmp/pyxamp-wf8145-build.lock cargo build -p pyxamp-daemon --bin pyxamp-daemon
PYXAMP_PROBE_DAEMON="$PWD/target/debug/pyxamp-daemon" PYXAMP_BROWSER_EXPECT_CAPTURE=1 \
  daemon/tools/browser-probe /absolute/path/to/chromium
# Explicit non-hermetic real-site probe, still on private services and profile:
PYXAMP_PROBE_DAEMON="$PWD/target/debug/pyxamp-daemon" PYXAMP_BROWSER_EXPECT_CAPTURE=1 \
  PYXAMP_BROWSER_URL='https://www.youtube.com/watch?v=jNQXAC9IVRw&autoplay=1' \
  daemon/tools/browser-probe /absolute/path/to/chromium
```

## Firefox evidence and limits

Source-verified, not runtime-verified: Mozilla Firefox at `efd2d1e6f77570ae6bc5294731a68101e45c81a3` has neither annotation in its native [MPRIS implementation](https://github.com/mozilla-firefox/firefox/blob/efd2d1e6f77570ae6bc5294731a68101e45c81a3/widget/gtk/MPRISServiceHandler.cpp), [PulseAudio context](https://github.com/mozilla-firefox/firefox/blob/efd2d1e6f77570ae6bc5294731a68101e45c81a3/third_party/rust/cubeb-pulse/src/backend/context.rs), or [PulseAudio stream implementation](https://github.com/mozilla-firefox/firefox/blob/efd2d1e6f77570ae6bc5294731a68101e45c81a3/third_party/rust/cubeb-pulse/src/backend/stream.rs). The stream constructor at lines 938–957 uses a stream name and an optional `module-stream-restore.id`, not a D-Bus owner/tab binding. `daemon/tools/browser-source-probe` records and reads these exact source revisions; it does not build or copy them into production code.

A stock Firefox executable was not available in this test environment. Playwright-patched Firefox builds were not treated as stock-browser proof. Firefox runtime, sandbox/portal PID namespaces, native Wayland browser operation, and the fleet compositor/Quickshell presentation still need on-device verification. The browser probe proves native capture and socket output, not the face rendering or deployed fleet integration. If a browser internally mixes tabs before its sole native stream, or publishes an unmappable process identity, per-tab isolation cannot be established from this protocol; do not claim such configurations as verified.

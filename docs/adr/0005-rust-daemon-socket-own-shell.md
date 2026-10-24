# A Rust daemon, a Unix socket to the face, and Pyxamp's own Quickshell instance

The daemon is Rust. It drives mpv (libmpv), publishes MPRIS (zbus), captures the followed player's audio from PipeWire, runs the Winamp FFT, and talks to Pyxis, Jellyfin and Ossicle. It can reuse the Pyxis and Ossicle contract types from their Rust crates. The Quickshell face talks to it over one Unix socket, with JSON lines in both directions. MPRIS alone cannot carry the queue or the library, and QML has no general D-Bus client. The face runs in its own Quickshell instance (`qs -c pyxamp`), not in the mountainous one, so a bar reload or theme change never touches Pyxamp and a Pyxamp crash never takes the bar down.

## Considered Options

- TypeScript on Bun: it uses the generated contract types as they are, but its PipeWire and D-Bus bindings are weak for 60 fps capture.
- A custom D-Bus interface for the face: the standard Linux way, but it needs a C++ QML plugin.
- Sharing the mountainous Quickshell instance: one process fewer, but reloads and crashes are shared.

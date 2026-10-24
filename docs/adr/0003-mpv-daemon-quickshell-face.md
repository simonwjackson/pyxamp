# A daemon plays the audio through mpv; Quickshell only draws the face

Quickshell reloads its configuration often, and audio that plays inside the Quickshell process stops at each reload. So a separate Pyxamp daemon owns playback, the source connections and the MPRIS presence, and the Quickshell face only draws it and sends commands. The daemon plays through mpv, not a GStreamer pipeline or Qt Multimedia. mpv plays every stream format the three sources serve, takes HTTP auth headers, plays gapless, and keeps pitch at other speeds. The EQ is an FFmpeg filter in mpv. The visualizer reads audio from PipeWire, not from the decoder. PipeWire is needed anyway, because the visualizer also shows foreign players such as a YouTube tab.

## Considered Options

- GStreamer: its `equalizer-10bands` and `spectrum` elements are ready-made, but we would own all playback logic and package its plugins in Nix.
- Qt MediaPlayer inside Quickshell: the fewest parts, but music stops at every shell reload.

# Audio analysis provenance

`daemon/src/fft.rs` ports the fixed 1024-input/512-output FFT in Webamp's `packages/webamp/js/components/FFTNullsoft.ts` at commit `7734cd3c7cf144bab2d8614ce9f9ba2ff21e163e`. Webamp identifies its origin as WACUP's `FFTNullsoft` implementation. The corresponding WACUP source is pinned at `6435f433c60b9ec8abc4e714dd55660f203ef010`, `FFTNullsoft/fft.cpp` and `fft.h`.

Both source trees are non-flake inputs in `flake.nix`, pinned with hashes in `flake.lock`. `Nullsoft-BSD-3-Clause.txt` preserves the full FFT license from the source header. `Webamp-MIT.txt` preserves Webamp's MIT notice. `WACUP-MIT.txt` preserves the repository's MIT notice. The FFT's specific BSD-3-Clause notice remains applicable, not replaced by the repository MIT license.

The Rust port retains the bit reversal, sine envelope, bias-decay equalization and iterative radix-2 butterfly. Its 75-column reduction and PCM waveform sampling are local code. It does not use Winamp executables, the restricted 2024 source release, bitmap assets, or Webamp's renderer.

PipeWire buffer/format setup follows pipewire-rs 0.9.2's `examples/audio-capture.rs`, Copyright The pipewire-rs Contributors, MIT. Its MIT notice is included in `pipewire-rs-MIT.txt`.

//! The engine without a PipeWire server, as at login before the graphical
//! session starts audio. Its own test binary: it sets `PIPEWIRE_REMOTE`.
mod engine_support;
use engine_support::*;
use pyxamp_daemon::engine::mpv;

#[tokio::test]
async fn the_engine_starts_without_pipewire_and_a_load_then_fails_once() {
    // At login PipeWire may start after the daemon. The engine must still become
    // ready; a load before PipeWire is up fails once (the source retries) and
    // mpv never falls back to another audio output. The remote name below does
    // not exist, so the user's audio server is never touched.
    unsafe { std::env::set_var("PIPEWIRE_REMOTE", "pyxamp-test-no-such-server") };
    let dir = tempfile::tempdir().unwrap();
    let options = mpv::Options {
        mpv: mpv_binary(),
        extra_args: vec![],
        runtime_dir: dir.path().to_path_buf(),
    };
    let mut h = Harness::with_options(dir, std::path::PathBuf::new(), options);
    h.ready().await;
    let file = tone(&h.dir.path().join("t.wav"), 440.0, 1.0, 0.1, 2);
    h.load(1, file.to_str().unwrap(), false).await.unwrap();
    let events = h.until(1, failed).await;
    assert!(
        events.iter().all(|e| e.token == 1 && !ended(&e.kind)),
        "{events:?}"
    );
    assert!(h.quiet(1_000).await.is_empty());
    h.finish().await;
}

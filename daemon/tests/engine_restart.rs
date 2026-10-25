//! mpv dying under the engine. Its own test binary, because it reads the
//! process-wide engine pid.
mod engine_support;
use engine_support::*;
use pyxamp_daemon::{
    engine::{EngineCommand, EngineEventKind, mpv},
    protocol::EngineStatus,
};
use std::time::Duration;

#[tokio::test]
async fn a_crashed_mpv_fails_the_load_once_and_restarts_with_the_same_settings() {
    let mut h = Harness::start_realtime();
    h.ready().await;
    let file = tone(&h.dir.path().join("long.wav"), 440.0, 5.0, 0.1, 2);
    h.send(EngineCommand::Volume { value: 0.3 }).await.unwrap();
    h.load(1, file.to_str().unwrap(), false).await.unwrap();
    h.until(1, started).await;
    let pid = mpv::engine_pid().expect("engine pid");
    assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
    let events = h.until(1, failed).await;
    assert_eq!(events.len(), 1, "{events:?}");
    let EngineEventKind::Failed(failure) = &events[0].kind else {
        unreachable!()
    };
    assert_eq!(failure.code, "engine");
    // While mpv is down, requests are answered, not queued.
    let mut state = h.handle.state.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(
            state.borrow_and_update().status,
            EngineStatus::Unavailable(_)
        ) {
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    h.ready().await;
    assert_ne!(mpv::engine_pid(), Some(pid));
    // The new mpv gets the old volume back.
    tokio::time::timeout(Duration::from_secs(5), async {
        while state.borrow_and_update().volume != Some(0.3) {
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("volume was not restored");
    let short = tone(&h.dir.path().join("short.wav"), 440.0, 0.3, 0.1, 2);
    h.load(2, short.to_str().unwrap(), false).await.unwrap();
    let events = h.until(2, ended).await;
    assert!(events.iter().all(|e| e.token == 2), "{events:?}");
    h.finish().await;
}

//! Own-stream visualizer capture. Run only through daemon/tools/test-engine-pipewire:
//! it needs a private PipeWire + WirePlumber and a private D-Bus.
mod engine_support;
use engine_support::*;
use pyxamp_daemon::{
    capture,
    engine::{self, mpv},
    protocol::*,
};
use std::{process::Command, time::Duration};
use tokio::sync::watch;

struct Play(std::process::Child);
impl Drop for Play {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "requires isolated PipeWire; run daemon/tools/test-engine-pipewire"]
async fn visualizer_captures_own_mpv_stream_without_an_unrelated_pw_play_tone() {
    let dir = tempfile::tempdir().unwrap();
    let own_tone = tone(&dir.path().join("own.wav"), 750.0, 30.0, 0.3, 2);
    let other_tone = tone(&dir.path().join("other.wav"), 6_000.0, 30.0, 0.3, 2);
    // An unrelated desktop stream that must never reach own capture.
    let other = Play(
        Command::new("pw-play")
            .arg(&other_tone)
            .spawn()
            .expect("pw-play"),
    );
    let options = mpv::Options {
        mpv: mpv_binary(),
        extra_args: vec!["--ao=pipewire".into()],
        runtime_dir: dir.path().to_path_buf(),
    };
    let mut h = Harness::with_options(dir, std::path::PathBuf::new(), options);
    h.ready().await;
    h.load(1, own_tone.to_str().unwrap(), false).await.unwrap();
    h.until(1, started).await;

    // The production path: mpris.rs asks the engine for the own identity.
    let connection = zbus::Connection::session().await.expect("private D-Bus");
    let snapshot = Snapshot {
        own: Some(OwnPlayback {
            source: SourceId::Pyxis,
            output: Output {
                id: "local".into(),
                name: "This computer".into(),
                local: true,
                active: true,
            },
            track: Track::default(),
            transport: Transport::Playing,
            capabilities: Capabilities::default(),
            occurrence: None,
            position_ms: None,
            volume: None,
            repeat: None,
            shuffle: None,
            speed: None,
            skip: None,
            retry: None,
            yielded: None,
            note: None,
        }),
        ..Snapshot::default()
    };
    let identity = engine::own_capture(&snapshot, &connection).expect("own capture identity");
    let target = identity.target.clone();
    let (identities, input) = watch::channel(Some(identity));
    let (output, mut frames) =
        watch::channel(Visualizer::silent(None, 0, VisualizerStatus::Idle, None));
    let thread = capture::spawn(input, output);
    // Watch ready frames for 3 s: the own tone must show, the other tone never.
    let mut loudest = 0;
    let mut leaked = 0;
    let mut ready = 0;
    let watch_until = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < watch_until && ready < 180 {
        tokio::time::timeout_at(watch_until, frames.changed())
            .await
            .ok();
        let frame = frames.borrow_and_update().clone();
        if matches!(frame.status, VisualizerStatus::Ready) && frame.target.as_ref() == Some(&target)
        {
            ready += 1;
            loudest = loudest.max(frame.columns[2]);
            leaked = leaked.max(frame.columns[18]);
        } else if let Some(reason) = &frame.reason {
            eprintln!("capture: {reason}");
        }
    }
    eprintln!("ready frames {ready}; own column 2 max={loudest}; column 18 max={leaked}");
    assert!(ready > 30, "no own-stream capture samples");
    assert!(loudest > 10, "own 750 Hz tone missing");
    assert!(
        leaked < 3,
        "the unrelated 6000 Hz pw-play stream leaked into own capture"
    );
    // Control: the unrelated tone was really playing. Captured on its own, it shows.
    let other_target = Target {
        bus_name: "org.mpris.MediaPlayer2.other".into(),
        owner: ":1.4242".into(),
    };
    identities.send_replace(Some(capture::CaptureIdentity {
        target: other_target.clone(),
        pid: other.0.id(),
        desktop_entry: None,
        identity: "pw-play".into(),
        shared_process: false,
    }));
    let other_peak = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            frames.changed().await.unwrap();
            let frame = frames.borrow_and_update().clone();
            if matches!(frame.status, VisualizerStatus::Ready)
                && frame.target.as_ref() == Some(&other_target)
                && frame.columns[18] > 10
            {
                return frame.columns[2];
            }
        }
    })
    .await
    .expect("the control pw-play tone was not audible");
    assert!(other_peak < 3, "own audio leaked into the other capture");
    identities.send_replace(Some(engine::own_capture(&snapshot, &connection).unwrap()));
    // Own playback stops: its stream goes away and capture never falls back to desktop audio.
    h.send(pyxamp_daemon::engine::EngineCommand::Stop)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            frames.changed().await.unwrap();
            let frame = frames.borrow_and_update().clone();
            if matches!(frame.status, VisualizerStatus::Unavailable) {
                assert_eq!(frame.columns, vec![0; 75]);
                break;
            }
        }
    })
    .await
    .expect("capture did not become unavailable without own audio");
    drop(identities);
    drop(frames);
    h.finish().await;
    thread.join().unwrap();
}

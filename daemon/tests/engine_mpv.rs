//! The mpv engine on generated local audio. mpv writes untimed raw PCM, so no
//! audio server or network is used. Requires `mpv` on PATH (`nix develop`).
mod engine_support;
use engine_support::*;
use pyxamp_daemon::{
    engine::{EngineCommand, EngineEventKind, Media},
    protocol::{EqSettings, SourceId, Track, Transport},
};
use std::time::Duration;

fn eq(preamp: f64, band: Option<(usize, f64)>) -> EqSettings {
    let mut bands = vec![0.0; 10];
    if let Some((index, db)) = band {
        bands[index] = db;
    }
    EqSettings {
        on: true,
        preamp_db: preamp,
        bands_db: bands,
    }
}

/// Plays one file to its end with the given settings and returns the output.
async fn render(file: &std::path::Path, setup: Vec<EngineCommand>) -> (Vec<f64>, Vec<f64>) {
    let mut h = Harness::start();
    h.ready().await;
    for command in setup {
        h.handle.send(None, command).await.unwrap();
    }
    h.load(1, file.to_str().unwrap(), false).await.unwrap();
    h.until(1, ended).await;
    h.finish().await
}

#[tokio::test]
async fn plays_a_local_file_and_reports_state_events_and_measured_format() {
    let mut h = Harness::start_realtime();
    let file = h.dir.path().join("mono.wav");
    write_wav(&file, 44_100, 1, 44_100, |frame, _| {
        0.3 * (std::f64::consts::TAU * 440.0 * frame as f64 / 44_100.0).sin()
    });
    h.ready().await;
    h.load(7, file.to_str().unwrap(), true).await.unwrap();
    let events = h.until(7, started).await;
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].owner, SourceId::Pyxis);
    let mut state = h.handle.state.clone();
    let measured = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = state.borrow_and_update().clone();
            if current
                .track
                .as_ref()
                .is_some_and(|t| t.sample_rate_hz.is_some())
            {
                return current;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("no measured format");
    assert_eq!(measured.owner, Some(SourceId::Pyxis));
    assert_eq!(measured.token, Some(7));
    assert_eq!(measured.transport, Transport::Paused);
    let track = measured.track.unwrap();
    assert_eq!(track.title, "Test tone");
    assert_eq!(track.sample_rate_hz, Some(44_100.0));
    assert_eq!(track.channels, Some(1));
    assert_eq!(track.bit_depth, Some(16));
    assert_eq!(track.duration_ms, Some(1_000));
    // Paused really holds on a real-time output.
    assert!(h.quiet(500).await.is_empty());
    assert_eq!(h.handle.state.borrow().transport, Transport::Paused);
    h.send(EngineCommand::Volume { value: 0.5 }).await.unwrap();
    h.send(EngineCommand::Play).await.unwrap();
    let events = h.until(7, ended).await;
    assert!(events.iter().all(|e| !failed(&e.kind)), "{events:?}");
    let state = h.handle.state.borrow().clone();
    assert_eq!(state.transport, Transport::Stopped);
    assert_eq!(state.token, None);
    assert_eq!(state.volume, Some(0.5));
    h.finish().await;
}

#[tokio::test]
async fn preloaded_item_follows_without_a_gap() {
    let mut h = Harness::start_gated();
    // One continuous 1 kHz tone split in two files at a peak, so a gap would show as a jump.
    // A is longer than the FIFO plus mpv's buffers, so it cannot end before the gate opens.
    let split = 2 * RATE as usize + 12;
    let a = tone_from(
        &h.dir.path().join("a.wav"),
        1_000.0,
        split as f64 / RATE as f64,
        0.5,
        2,
        0,
    );
    let b = tone_from(&h.dir.path().join("b.wav"), 1_000.0, 0.5, 0.5, 2, split);
    h.ready().await;
    h.load(1, a.to_str().unwrap(), true).await.unwrap();
    h.send(EngineCommand::Preload {
        token: 2,
        media: Some(media(b.to_str().unwrap())),
    })
    .await
    .unwrap();
    h.send(EngineCommand::Play).await.unwrap();
    h.open_gate();
    let events = h.until(2, ended).await;
    let order: Vec<_> = events
        .iter()
        .map(|e| (e.token, format!("{:?}", e.kind)))
        .collect();
    assert_eq!(
        order,
        vec![
            (1, "Started".into()),
            (1, "Ended".into()),
            (2, "Started".into()),
            (2, "Ended".into())
        ]
    );
    let (left, _) = h.finish().await;
    let total = split + RATE as usize / 2;
    assert!(
        (left.len() as i64 - total as i64).abs() < 64,
        "{} vs {total}",
        left.len()
    );
    // A 1 kHz sine at 0.5 moves at most 0.066 per sample; a gap or click jumps further.
    let step = 0.5 * std::f64::consts::TAU * 1_000.0 / RATE as f64;
    let worst = left[100..left.len() - 100]
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f64::max);
    assert!(worst < step * 1.2, "discontinuity {worst} > {step}");
}

#[tokio::test]
async fn eq_and_preamp_change_a_known_tone_and_persist_across_loads() {
    let dir = tempfile::tempdir().unwrap();
    let file = tone(&dir.path().join("1k.wav"), 1_000.0, 1.0, 0.1, 2);
    let flat = rms(middle(&render(&file, vec![]).await.0));
    let boosted = rms(middle(
        &render(&file, vec![EngineCommand::Eq(eq(0.0, Some((4, 12.0))))])
            .await
            .0,
    ));
    let other_band = rms(middle(
        &render(&file, vec![EngineCommand::Eq(eq(0.0, Some((0, 12.0))))])
            .await
            .0,
    ));
    let preamp = rms(middle(
        &render(&file, vec![EngineCommand::Eq(eq(-6.0, None))])
            .await
            .0,
    ));
    let off = rms(middle(
        &render(
            &file,
            vec![EngineCommand::Eq(EqSettings {
                on: false,
                ..eq(0.0, Some((4, 12.0)))
            })],
        )
        .await
        .0,
    ));
    eprintln!("flat {flat} boosted {boosted} 60Hz {other_band} preamp {preamp} off {off}");
    // +12 dB is a factor of 3.98.
    assert!((3.4..4.6).contains(&(boosted / flat)), "{}", boosted / flat);
    assert!(
        (0.85..1.2).contains(&(other_band / flat)),
        "{}",
        other_band / flat
    );
    assert!((0.45..0.56).contains(&(preamp / flat)), "{}", preamp / flat);
    assert!((0.97..1.03).contains(&(off / flat)), "{}", off / flat);

    // Settings live in the engine, not in one load: two loads, both boosted.
    let mut h = Harness::start();
    h.ready().await;
    h.handle
        .send(None, EngineCommand::Eq(eq(0.0, Some((4, 12.0)))))
        .await
        .unwrap();
    for token in [1, 2] {
        h.load(token, file.to_str().unwrap(), false).await.unwrap();
        h.until(token, ended).await;
    }
    let (left, _) = h.finish().await;
    let half = left.len() / 2;
    for part in [&left[..half], &left[half..]] {
        let ratio = rms(middle(part)) / flat;
        assert!((3.4..4.6).contains(&ratio), "{ratio}");
    }
}

#[tokio::test]
async fn balance_attenuates_one_side_of_a_known_tone() {
    let dir = tempfile::tempdir().unwrap();
    let file = tone(&dir.path().join("1k.wav"), 1_000.0, 1.0, 0.2, 2);
    let (left, right) = render(&file, vec![]).await;
    let (l, r) = (rms(middle(&left)), rms(middle(&right)));
    let (hard_left, hard_left_r) =
        render(&file, vec![EngineCommand::Balance { value: -1.0 }]).await;
    let (half_right_l, half_right) =
        render(&file, vec![EngineCommand::Balance { value: 0.5 }]).await;
    assert!((rms(middle(&hard_left)) / l - 1.0).abs() < 0.05);
    assert!(rms(middle(&hard_left_r)) / r < 0.01);
    assert!((rms(middle(&half_right_l)) / l - 0.5).abs() < 0.05);
    assert!((rms(middle(&half_right)) / r - 1.0).abs() < 0.05);
}

#[tokio::test]
async fn speed_changes_duration_but_keeps_pitch() {
    let dir = tempfile::tempdir().unwrap();
    let file = tone(&dir.path().join("1k.wav"), 1_000.0, 2.0, 0.3, 2);
    let (normal, _) = render(&file, vec![]).await;
    let mut h = Harness::start();
    h.ready().await;
    // The session (`None`) may set speed as well as the active source.
    h.handle
        .send(None, EngineCommand::Speed { rate: 2.0 })
        .await
        .unwrap();
    assert_eq!(h.handle.state.borrow().speed, Some(2.0));
    h.load(1, file.to_str().unwrap(), false).await.unwrap();
    h.until(1, ended).await;
    let (fast, _) = h.finish().await;
    let ratio = fast.len() as f64 / normal.len() as f64;
    assert!((0.45..0.56).contains(&ratio), "{ratio}");
    let pitch = frequency(middle(&fast));
    assert!((960.0..1040.0).contains(&pitch), "pitch {pitch}");
    assert!(
        (960.0..1040.0).contains(&frequency(middle(&normal))),
        "reference pitch"
    );
}

#[tokio::test]
async fn refuses_sources_that_are_not_active_and_bad_values() {
    let h = Harness::start();
    h.ready().await;
    let file = tone(&h.dir.path().join("t.wav"), 440.0, 0.2, 0.1, 2);
    h.active.send_replace(Some(SourceId::Ossicle));
    let refused = h.load(1, file.to_str().unwrap(), true).await.unwrap_err();
    assert_eq!(refused.code, "inactive");
    h.active.send_replace(None);
    assert_eq!(
        h.send(EngineCommand::Pause).await.unwrap_err().code,
        "inactive"
    );
    // The session (`None`) is always accepted.
    h.handle
        .send(None, EngineCommand::Balance { value: 0.25 })
        .await
        .unwrap();
    h.active.send_replace(Some(SourceId::Pyxis));
    for bad in [
        EngineCommand::Speed { rate: 3.5 },
        EngineCommand::Volume { value: 1.5 },
        EngineCommand::Balance { value: -2.0 },
        EngineCommand::Eq(EqSettings {
            on: true,
            preamp_db: 0.0,
            bands_db: vec![0.0; 9],
        }),
    ] {
        assert_eq!(h.send(bad).await.unwrap_err().code, "invalid");
    }
    let header_injection = Media {
        url: file.to_str().unwrap().into(),
        headers: vec![("X".into(), "a\r\nB: c".into())],
        track: Track::default(),
    };
    assert_eq!(
        h.send(EngineCommand::Load {
            token: 2,
            media: header_injection,
            start_ms: 0,
            paused: true
        })
        .await
        .unwrap_err()
        .code,
        "invalid"
    );
    h.finish().await;
}

/// Exactly one `Failed` for the failed token, nothing else afterwards.
async fn fails_once(h: &mut Harness, token: u64) -> String {
    let events = h.until(token, failed).await;
    assert!(
        events
            .iter()
            .all(|e| e.token == token && !matches!(e.kind, EngineEventKind::Ended)),
        "{events:?}"
    );
    let later = h.quiet(1_500).await;
    assert!(later.is_empty(), "events after the failure: {later:?}");
    let state = h.handle.state.borrow().clone();
    assert_eq!(state.token, None);
    assert_eq!(state.transport, Transport::Stopped);
    match &events.last().unwrap().kind {
        EngineEventKind::Failed(failure) => failure.message.clone(),
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn local_failures_report_once_and_a_reload_with_a_new_token_plays() {
    let mut h = Harness::start();
    h.ready().await;
    let good = tone(&h.dir.path().join("good.wav"), 440.0, 0.3, 0.1, 2);
    let garbage = h.dir.path().join("garbage.flac");
    std::fs::write(&garbage, vec![0x5a; 64 * 1024]).unwrap();
    let started_at = std::time::Instant::now();
    h.load(1, h.dir.path().join("missing.wav").to_str().unwrap(), false)
        .await
        .unwrap();
    fails_once(&mut h, 1).await;
    assert!(
        started_at.elapsed() < Duration::from_secs(3),
        "slow failure"
    );
    h.load(2, garbage.to_str().unwrap(), false).await.unwrap();
    fails_once(&mut h, 2).await;
    // The source's retry: a new load of a playable item with a new token.
    h.load(3, good.to_str().unwrap(), false).await.unwrap();
    let events = h.until(3, ended).await;
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(started(&events[0].kind));
    h.finish().await;
}

#[tokio::test]
async fn http_headers_404_and_a_dropped_stream() {
    let dir = tempfile::tempdir().unwrap();
    let body = std::fs::read(tone(&dir.path().join("t.wav"), 440.0, 1.0, 0.1, 2)).unwrap();
    let long = std::fs::read(tone(&dir.path().join("long.wav"), 440.0, 8.0, 0.1, 2)).unwrap();
    let server = Server::start(vec![
        (
            "/ok.wav",
            Reply::File {
                body: body.clone(),
                delay_ms: 0,
            },
        ),
        ("/slow-missing.wav", Reply::Missing { delay_ms: 600 }),
        (
            "/drop.wav",
            Reply::Truncated {
                body: long.clone(),
                sent: long.len() / 4,
            },
        ),
    ]);
    // Real-time output: an early end is judged against the played position.
    let mut h = Harness::start_realtime();
    h.ready().await;

    let mut ok = media(&server.url("/ok.wav"));
    ok.headers = vec![
        (
            "Authorization".into(),
            "MediaBrowser Client=\"Pyxamp\", Token=\"t\"".into(),
        ),
        ("X-Pyxis-Contract".into(), "pyxis-rpc-v3".into()),
    ];
    h.send(EngineCommand::Load {
        token: 1,
        media: ok,
        start_ms: 0,
        paused: false,
    })
    .await
    .unwrap();
    h.until(1, ended).await;
    let headers = server.requests.lock().unwrap()[0].1.clone();
    assert!(
        headers.contains(&"Authorization: MediaBrowser Client=\"Pyxamp\", Token=\"t\"".to_owned()),
        "{headers:?}"
    );
    assert!(headers.contains(&"X-Pyxis-Contract: pyxis-rpc-v3".to_owned()));

    // A 404 fails once; the preloaded item must not start on its own.
    h.load(2, &server.url("/slow-missing.wav"), false)
        .await
        .unwrap();
    h.send(EngineCommand::Preload {
        token: 3,
        media: Some(media(&server.url("/ok.wav"))),
    })
    .await
    .unwrap();
    let before = server.hits("/ok.wav");
    fails_once(&mut h, 2).await;
    assert_eq!(
        server.hits("/ok.wav"),
        before,
        "the engine advanced after a failure"
    );

    // The stream drops a quarter of the way in: one failure, not a normal end.
    h.load(4, &server.url("/drop.wav"), false).await.unwrap();
    let reason = fails_once(&mut h, 4).await;
    eprintln!("dropped stream: {reason}");
    h.finish().await;
}

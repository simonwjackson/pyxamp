//! Session yield and source switching, with a recording engine, recording sources and a
//! controlled wall clock. No mpv, D-Bus or audio.
use pyxamp_daemon::{
    engine::{self, EngineCommand, EngineState},
    protocol::*,
    session::{self, RESUME_LIMIT_MS, RequestEnvelope},
    sources::{self, SourceSession, SourceState},
    system_volume,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};

type Log<T> = Arc<Mutex<Vec<T>>>;

struct World {
    clock: Arc<AtomicU64>,
    state: watch::Sender<Snapshot>,
    requests: mpsc::Sender<RequestEnvelope>,
    engine_log: Log<(Option<SourceId>, EngineCommand)>,
    engine_state: watch::Sender<EngineState>,
    source_log: Log<(SourceId, String, SourceAction)>,
    source_states: Vec<watch::Sender<SourceState>>,
    /// What the recording wpctl was asked, one line per run.
    wpctl_log: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

fn own(source: SourceId) -> OwnPlayback {
    OwnPlayback {
        source,
        output: Output {
            id: "local".into(),
            name: "This computer".into(),
            local: true,
            active: true,
        },
        track: Track::default(),
        transport: Transport::Playing,
        capabilities: Capabilities::default(),
        occurrence: Some("o1".into()),
        position_ms: None,
        volume: None,
        repeat: None,
        shuffle: None,
        speed: None,
        skip: None,
        retry: None,
        yielded: None,
        note: None,
    }
}

const T0: u64 = 1_800_000_000_000;

/// A real program in wpctl's place that records its arguments and exits with `status`.
fn recording_wpctl(dir: &std::path::Path, status: u8) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let log = dir.join("wpctl.log");
    let program = dir.join("wpctl");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nexit {status}\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    (program, log)
}

fn world() -> World {
    world_with_wpctl(0)
}

fn world_with_wpctl(status: u8) -> World {
    let dir = tempfile::tempdir().unwrap();
    let (wpctl, wpctl_log) = recording_wpctl(dir.path(), status);
    let (handle, mut ctx) = engine::channels();
    let engine_state = ctx.state.clone();
    let engine_log: Log<_> = Arc::default();
    let log = engine_log.clone();
    // Recording engine: records every request and reflects play/pause in its state.
    tokio::spawn(async move {
        while let Some(request) = ctx.requests.recv().await {
            log.lock()
                .unwrap()
                .push((request.owner, request.command.clone()));
            match request.command {
                EngineCommand::Pause => ctx.state.send_modify(|s| s.transport = Transport::Paused),
                EngineCommand::Play => ctx.state.send_modify(|s| s.transport = Transport::Playing),
                _ => {}
            }
            let _ = request.reply.send(Ok(()));
        }
    });
    let (active_tx, active) = watch::channel(None);
    let source_log: Log<_> = Arc::default();
    let mut handles = vec![];
    let mut source_states = vec![];
    for id in [SourceId::Pyxis, SourceId::Ossicle] {
        let (handle, mut ctx) = sources::channels(id, (), handle.clone(), active.clone());
        handles.push(handle);
        let log = source_log.clone();
        source_states.push(ctx.state.clone());
        tokio::spawn(async move {
            let mut state = SourceState::new(id, SourceStatus::Ready(Empty {}));
            state.session = Some(SourceSession {
                own: own(id),
                queue: vec![],
                outputs: vec![],
                conflicts: vec![],
            });
            ctx.state.send_replace(state);
            while let Some(request) = ctx.requests.recv().await {
                log.lock()
                    .unwrap()
                    .push((id, request.id.clone(), request.action.clone()));
                let _ = request.reply.send(Ok(None));
            }
        });
    }
    let clock = Arc::new(AtomicU64::new(T0));
    let now = clock.clone();
    let (state, _) = watch::channel(Snapshot::default());
    let (requests, receiver) = mpsc::channel(8);
    tokio::spawn(session::run_with_clock(
        handles,
        handle,
        system_volume::Control::with_program(wpctl),
        active_tx,
        state.clone(),
        receiver,
        Arc::new(move || now.load(Ordering::SeqCst)),
    ));
    World {
        clock,
        state,
        requests,
        engine_log,
        engine_state,
        source_log,
        source_states,
        wpctl_log,
        _dir: dir,
    }
}

impl World {
    async fn request(&self, id: &str, action: SourceAction) -> Result<Option<Page>, Failure> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(RequestEnvelope {
                request: Request {
                    id: id.into(),
                    action,
                },
                reply,
            })
            .await
            .unwrap();
        response.await.unwrap()
    }

    async fn play(&self, source: SourceId, token: u64) {
        self.request(
            &format!("face-play-{token}"),
            SourceAction::Play {
                source,
                item: "track".into(),
                mode: PlayMode::Now,
            },
        )
        .await
        .unwrap();
        // The source loads once it is active; the engine now plays its token.
        self.engine_state.send_modify(|s| {
            s.status = EngineStatus::Ready(Empty {});
            s.owner = Some(source);
            s.token = Some(token);
            s.transport = Transport::Playing;
        });
        settle().await;
    }

    fn foreign(&self, playing: bool) {
        let player = if playing {
            PlayerState::Following(Box::new(Following {
                target: Target {
                    bus_name: "org.mpris.MediaPlayer2.firefox".into(),
                    owner: ":1.9".into(),
                },
                identity: "Firefox".into(),
                track: Track::default(),
                transport: Transport::Playing,
                capabilities: Capabilities::default(),
                position_ms: None,
                volume: None,
                repeat: None,
                shuffle: None,
                rate: None,
            }))
        } else {
            PlayerState::Idle(Empty {})
        };
        self.state.send_modify(|s| s.player = player);
    }

    fn advance(&self, ms: u64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
        // Any state change wakes the session, as a position update would.
        self.engine_state.send_modify(|s| {
            s.position_ms = Some(s.position_ms.unwrap_or(0) + 1);
        });
    }

    /// The yield commands the session sent the engine (owner `None`).
    fn yields(&self) -> Vec<EngineCommand> {
        self.engine_log
            .lock()
            .unwrap()
            .iter()
            .filter(|(owner, _)| owner.is_none())
            .map(|(_, command)| command.clone())
            .collect()
    }

    fn yielded(&self) -> Option<Yielded> {
        self.state
            .borrow()
            .own
            .as_ref()
            .and_then(|o| o.yielded.clone())
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn foreign_start_pauses_once_and_a_short_interruption_resumes() {
    let w = world();
    w.play(SourceId::Pyxis, 1).await;
    w.foreign(true);
    settle().await;
    assert_eq!(w.yields(), vec![EngineCommand::Pause]);
    assert_eq!(
        w.yielded(),
        Some(Yielded {
            resume_by_ms: Some(T0 + RESUME_LIMIT_MS)
        })
    );
    w.advance(60_000);
    settle().await;
    assert_eq!(w.yields(), vec![EngineCommand::Pause], "paused only once");
    w.foreign(false);
    settle().await;
    assert_eq!(w.yields(), vec![EngineCommand::Pause, EngineCommand::Play]);
    assert_eq!(w.yielded(), None);
}

#[tokio::test]
async fn a_long_interruption_stays_paused() {
    let w = world();
    w.play(SourceId::Pyxis, 1).await;
    w.foreign(true);
    settle().await;
    w.advance(RESUME_LIMIT_MS + 1);
    settle().await;
    assert_eq!(w.yielded(), Some(Yielded { resume_by_ms: None }));
    w.foreign(false);
    settle().await;
    assert_eq!(w.yields(), vec![EngineCommand::Pause]);
    assert_eq!(w.yielded(), None);
}

#[tokio::test]
async fn a_user_action_cancels_resume_and_paused_own_is_never_restarted() {
    let w = world();
    w.play(SourceId::Pyxis, 1).await;
    w.foreign(true);
    settle().await;
    w.request(
        "face-seek",
        SourceAction::Seek {
            position_ms: 1_000,
            occurrence: "o1".into(),
        },
    )
    .await
    .unwrap();
    settle().await;
    assert_eq!(w.yielded(), Some(Yielded { resume_by_ms: None }));
    w.foreign(false);
    settle().await;
    assert_eq!(w.yields(), vec![EngineCommand::Pause]);

    // Own already paused when a foreign player starts: no pause, no resume.
    let w = world();
    w.play(SourceId::Pyxis, 1).await;
    w.engine_state
        .send_modify(|s| s.transport = Transport::Paused);
    settle().await;
    w.foreign(true);
    settle().await;
    w.foreign(false);
    settle().await;
    assert!(w.yields().is_empty(), "{:?}", w.yields());
}

#[tokio::test]
async fn switching_sources_pauses_the_old_one_internally() {
    let w = world();
    w.play(SourceId::Pyxis, 1).await;
    w.play(SourceId::Ossicle, 2).await;
    let log = w.source_log.lock().unwrap().clone();
    let pauses: Vec<_> = log
        .iter()
        .filter(|(_, _, action)| {
            matches!(
                action,
                SourceAction::Transport {
                    action: OwnTransport::Pause
                }
            )
        })
        .collect();
    assert_eq!(pauses.len(), 1, "{log:?}");
    assert_eq!(pauses[0].0, SourceId::Pyxis);
    assert!(pauses[0].1.starts_with("internal:"));
    assert_eq!(
        w.state.borrow().own.as_ref().unwrap().source,
        SourceId::Ossicle
    );
    // EQ and balance go to the engine as the session (`None`), not to a source.
    let eq = EqSettings {
        on: true,
        preamp_db: 1.0,
        bands_db: vec![0.0; 10],
    };
    w.request("face-eq", SourceAction::SetEq(eq.clone()))
        .await
        .unwrap();
    assert!(w.yields().contains(&EngineCommand::Eq(eq)));
}

#[tokio::test]
async fn a_source_asking_to_play_becomes_active_and_pauses_the_old_one() {
    let w = world();
    w.play(SourceId::Ossicle, 1).await;
    // A remote console tells this computer to play Pyxis: the source asks to be active.
    w.source_states[0].send_modify(|s| s.activation += 1);
    settle().await;
    assert_eq!(
        w.state.borrow().own.as_ref().unwrap().source,
        SourceId::Pyxis
    );
    let log = w.source_log.lock().unwrap().clone();
    assert!(
        log.iter()
            .any(|(source, id, action)| *source == SourceId::Ossicle
                && id.starts_with("internal:")
                && matches!(
                    action,
                    SourceAction::Transport {
                        action: OwnTransport::Pause
                    }
                )),
        "{log:?}"
    );
}

#[tokio::test]
async fn play_on_outputs_show_before_the_first_play() {
    let w = world();
    let speaker = Output {
        id: "sonos".into(),
        name: "Living Room".into(),
        local: false,
        active: false,
    };
    settle().await; // the recording sources publish their first state
    w.source_states[0].send_modify(|s| s.outputs = vec![speaker.clone()]);
    settle().await;
    assert!(w.state.borrow().own.is_none());
    assert_eq!(w.state.borrow().outputs, vec![speaker]);
}

#[tokio::test]
async fn system_volume_goes_to_the_default_sink_not_to_a_source_or_the_engine() {
    let w = world();
    w.play(SourceId::Pyxis, 1).await;
    let (sources_before, engine_before) = (
        w.source_log.lock().unwrap().len(),
        w.engine_log.lock().unwrap().len(),
    );
    w.request("face-vol", SourceAction::SystemVolume { value: 0.42 })
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&w.wpctl_log).unwrap(),
        "set-volume @DEFAULT_AUDIO_SINK@ 0.4200\n"
    );
    assert_eq!(w.source_log.lock().unwrap().len(), sources_before);
    assert_eq!(w.engine_log.lock().unwrap().len(), engine_before);
    // Following a foreign player changes nothing about where it goes.
    w.foreign(true);
    settle().await;
    w.request("face-vol-2", SourceAction::SystemVolume { value: 0.0 })
        .await
        .unwrap();
    assert!(
        std::fs::read_to_string(&w.wpctl_log)
            .unwrap()
            .ends_with("set-volume @DEFAULT_AUDIO_SINK@ 0.0000\n")
    );
    assert_eq!(w.source_log.lock().unwrap().len(), sources_before);
    // Not a user action on own playback: the yield is not cancelled.
    assert!(
        w.state
            .borrow()
            .own
            .as_ref()
            .unwrap()
            .yielded
            .as_ref()
            .is_some_and(|y| y.resume_by_ms.is_some())
    );
}

#[tokio::test]
async fn a_failed_system_volume_set_is_rejected_with_the_reason() {
    let w = world_with_wpctl(1);
    let failure = w
        .request("face-vol", SourceAction::SystemVolume { value: 0.5 })
        .await
        .unwrap_err();
    assert_eq!(failure.code, "systemVolume");
}

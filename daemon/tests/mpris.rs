use pyxamp_daemon::{mpris, presence, protocol::*};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, watch};
use zbus::{
    Connection, fdo,
    zvariant::{OwnedObjectPath, Value},
};

#[derive(Clone)]
struct PlayerConfig {
    status: String,
    track: String,
    duration: Option<i64>,
    can_control: bool,
    can_play: bool,
    can_pause: bool,
    can_seek: bool,
    can_next: bool,
    failure: bool,
    volume: f64,
    repeat: String,
    shuffle: bool,
    calls: Vec<String>,
    metadata_delay_ms: u64,
    metadata_reads: Arc<std::sync::atomic::AtomicUsize>,
}
impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            status: "Playing".into(),
            track: "/track/one".into(),
            duration: Some(60_000_000),
            can_control: true,
            can_play: true,
            can_pause: false,
            can_seek: true,
            can_next: true,
            failure: false,
            volume: 0.5,
            repeat: "None".into(),
            shuffle: false,
            calls: vec![],
            metadata_delay_ms: 0,
            metadata_reads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}
struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Session audio player"
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "session-audio"
    }
}
struct SessionPlayer {
    config: Arc<Mutex<PlayerConfig>>,
}
impl SessionPlayer {
    fn call(&self, name: &str) -> fdo::Result<()> {
        let mut c = self.config.lock().unwrap();
        if c.failure {
            return Err(fdo::Error::Failed("Configured native refusal".into()));
        }
        c.calls.push(name.into());
        Ok(())
    }
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl SessionPlayer {
    fn play(&self) -> fdo::Result<()> {
        self.call("Play")
    }
    fn pause(&self) -> fdo::Result<()> {
        self.call("Pause")
    }
    fn play_pause(&self) -> fdo::Result<()> {
        self.call("PlayPause")
    }
    fn stop(&self) -> fdo::Result<()> {
        self.call("Stop")
    }
    fn previous(&self) -> fdo::Result<()> {
        self.call("Previous")
    }
    fn next(&self) -> fdo::Result<()> {
        self.call("Next")
    }
    fn set_position(&self, track_id: OwnedObjectPath, position: i64) -> fdo::Result<()> {
        let c = self.config.lock().unwrap();
        if track_id.as_str() != c.track {
            return Err(fdo::Error::Failed("Track changed".into()));
        }
        drop(c);
        self.call(&format!("SetPosition:{track_id}:{position}"))
    }
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.config.lock().unwrap().status.clone()
    }
    #[zbus(property)]
    async fn metadata(&self) -> HashMap<&'static str, Value<'static>> {
        let delay = {
            let config = self.config.lock().unwrap();
            config
                .metadata_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            config.metadata_delay_ms
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let c = self.config.lock().unwrap();
        let mut m = HashMap::from([
            (
                "mpris:trackid",
                Value::from(OwnedObjectPath::try_from(c.track.clone()).unwrap()),
            ),
            ("xesam:title", Value::from("Observed title")),
            ("xesam:artist", Value::from(vec!["Artist"])),
        ]);
        if let Some(d) = c.duration {
            m.insert("mpris:length", Value::from(d));
        }
        m
    }
    #[zbus(property)]
    fn position(&self) -> i64 {
        1_000_000
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        self.config.lock().unwrap().can_control
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.config.lock().unwrap().can_play
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.config.lock().unwrap().can_pause
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.config.lock().unwrap().can_seek
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.config.lock().unwrap().can_next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.config.lock().unwrap().volume
    }
    #[zbus(property)]
    fn set_volume(&self, value: f64) {
        self.config.lock().unwrap().volume = value;
    }
    #[zbus(property)]
    fn loop_status(&self) -> String {
        self.config.lock().unwrap().repeat.clone()
    }
    #[zbus(property)]
    fn set_loop_status(&self, value: &str) {
        self.config.lock().unwrap().repeat = value.into();
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.config.lock().unwrap().shuffle
    }
    #[zbus(property)]
    fn set_shuffle(&self, value: bool) {
        self.config.lock().unwrap().shuffle = value;
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }
}
struct BusPlayer {
    connection: Connection,
    config: Arc<Mutex<PlayerConfig>>,
    target: Target,
}
impl BusPlayer {
    async fn start(name: &str, status: &str) -> Self {
        let bus_name = format!("org.mpris.MediaPlayer2.{name}");
        let config = Arc::new(Mutex::new(PlayerConfig {
            status: status.into(),
            ..PlayerConfig::default()
        }));
        let connection = zbus::connection::Builder::session()
            .unwrap()
            .name(bus_name.as_str())
            .unwrap()
            .serve_at(mpris::PATH, Root)
            .unwrap()
            .serve_at(
                mpris::PATH,
                SessionPlayer {
                    config: config.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let target = Target {
            bus_name,
            owner: connection.unique_name().unwrap().to_string(),
        };
        Self {
            connection,
            config,
            target,
        }
    }
    async fn status(&self, status: &str) {
        self.config.lock().unwrap().status = status.into();
        let properties = HashMap::from([("PlaybackStatus", Value::from(status))]);
        self.connection
            .emit_signal(
                None::<&str>,
                mpris::PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(mpris::PLAYER, properties, Vec::<String>::new()),
            )
            .await
            .unwrap();
    }
}
fn command(player: &BusPlayer, action: Action) -> Command {
    Command {
        id: "gesture".into(),
        target: player.target.clone(),
        action,
    }
}

#[tokio::test]
async fn isolated_bus_owner_capability_track_and_native_command_safety() {
    let p = BusPlayer::start("fixtureCommands", "Playing").await;
    let client = Connection::session()
        .await
        .expect("Run with dbus-run-session");
    let o = mpris::observe(&client, p.target.clone()).await.unwrap();
    assert_eq!(o.player.track.id.as_deref(), Some("/track/one"));
    assert_eq!(o.player.track.duration_ms, Some(60_000));
    assert!(o.player.track.channels.is_none());
    assert!(!o.player.capabilities.pause);
    assert!(o.player.capabilities.volume);
    let execute = |action| {
        let command = command(&p, action);
        let client = client.clone();
        let target = p.target.clone();
        async move { mpris::execute(&client, Some(&target), &command).await }
    };
    assert_eq!(
        execute(Action::Pause(Empty {})).await.unwrap_err().code,
        "unsupported"
    );
    assert_eq!(
        execute(Action::Previous(Empty {})).await.unwrap_err().code,
        "unsupported"
    );
    execute(Action::Next(Empty {})).await.unwrap();
    assert_eq!(p.config.lock().unwrap().calls, vec!["Next"]);
    // Accepted is native completion, not a synthetic track/transport update.
    assert_eq!(
        mpris::observe(&client, p.target.clone())
            .await
            .unwrap()
            .player
            .track
            .id
            .as_deref(),
        Some("/track/one")
    );
    execute(Action::Volume { value: 0.25 }).await.unwrap();
    assert_eq!(p.config.lock().unwrap().volume, 0.25);
    execute(Action::Repeat { mode: Repeat::One }).await.unwrap();
    assert_eq!(p.config.lock().unwrap().repeat, "Track");
    execute(Action::Shuffle { enabled: true }).await.unwrap();
    assert!(p.config.lock().unwrap().shuffle);
    execute(Action::Seek(Seek {
        position_ms: 2000,
        track_id: "/track/one".into(),
    }))
    .await
    .unwrap();
    p.config.lock().unwrap().track = "/track/two".into();
    assert_eq!(
        execute(Action::Seek(Seek {
            position_ms: 4000,
            track_id: "/track/one".into()
        }))
        .await
        .unwrap_err()
        .code,
        "staleTrack"
    );
    assert_eq!(
        execute(Action::Seek(Seek {
            position_ms: 60_001,
            track_id: "/track/two".into()
        }))
        .await
        .unwrap_err()
        .code,
        "invalid"
    );
    p.config.lock().unwrap().duration = None;
    assert!(
        !mpris::observe(&client, p.target.clone())
            .await
            .unwrap()
            .player
            .capabilities
            .seek
    );
    p.config.lock().unwrap().failure = true;
    assert_eq!(
        execute(Action::Play(Empty {})).await.unwrap_err().code,
        "native"
    );
    let old = command(&p, Action::Next(Empty {}));
    p.config.lock().unwrap().metadata_delay_ms = 200;
    let (result, replacement) =
        tokio::join!(mpris::execute(&client, Some(&old.target), &old), async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            p.connection
                .release_name(p.target.bus_name.as_str())
                .await
                .unwrap();
            BusPlayer::start("fixtureCommands", "Playing").await
        });
    assert_eq!(result.unwrap_err().code, "staleTarget");
    assert!(replacement.config.lock().unwrap().calls.is_empty());
    assert_eq!(
        mpris::execute(&client, None, &old).await.unwrap_err().code,
        "noTarget"
    );
}

async fn wait_target(state: &mut watch::Receiver<Snapshot>, expected: Option<&str>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let actual = match &state.borrow().player {
                PlayerState::Following(p) => Some(p.target.bus_name.clone()),
                _ => None,
            };
            if actual.as_deref() == expected {
                break;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn real_signals_drive_follow_and_self_mpris_forwards_to_captured_owner() {
    let a = BusPlayer::start("fixtureFollowA", "Playing").await;
    let b = BusPlayer::start("fixtureFollowB", "Playing").await;
    let (state_tx, mut state) = watch::channel(Snapshot::default());
    let (capture_tx, _capture) = watch::channel(None);
    let (commands, receiver) = mpsc::channel(8);
    let own = zbus::connection::Builder::session()
        .unwrap()
        .name("org.mpris.MediaPlayer2.pyxamp")
        .unwrap()
        .serve_at(mpris::PATH, presence::Root)
        .unwrap()
        .serve_at(
            mpris::PATH,
            presence::Player {
                state: state.clone(),
                commands,
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let runner = tokio::spawn(mpris::run(own.clone(), state_tx, capture_tx, receiver));
    wait_target(&mut state, Some(&a.target.bus_name)).await;
    // Metadata/status repetition does not promote B. A real paused->playing transition does.
    b.status("Playing").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(matches!(&state.borrow().player, PlayerState::Following(p) if p.target == a.target));
    b.status("Paused").await;
    b.status("Playing").await;
    wait_target(&mut state, Some(&b.target.bus_name)).await;
    let client = Connection::session().await.unwrap();
    let own_proxy = zbus::Proxy::new(
        &client,
        "org.mpris.MediaPlayer2.pyxamp",
        mpris::PATH,
        mpris::PLAYER,
    )
    .await
    .unwrap();
    let _: () = own_proxy.call("Next", &()).await.unwrap();
    assert_eq!(b.config.lock().unwrap().calls, vec!["Next"]);
    assert!(a.config.lock().unwrap().calls.is_empty());
    b.status("Paused").await;
    wait_target(&mut state, Some(&a.target.bus_name)).await;
    a.status("Stopped").await;
    wait_target(&mut state, None).await;
    assert_eq!(
        own_proxy
            .get_property::<String>("PlaybackStatus")
            .await
            .unwrap(),
        "Stopped"
    );
    runner.abort();
}

#[tokio::test]
async fn queued_native_transitions_keep_actual_order_across_slow_property_poll() {
    use std::sync::atomic::Ordering;
    let a = BusPlayer::start("fixtureOrderA", "Paused").await;
    let b = BusPlayer::start("fixtureOrderB", "Paused").await;
    let (state_tx, mut state) = watch::channel(Snapshot::default());
    let (capture_tx, _capture) = watch::channel(None);
    let (_commands, receiver) = mpsc::channel(8);
    let connection = Connection::session().await.unwrap();
    let runner = tokio::spawn(mpris::run(connection, state_tx, capture_tx, receiver));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (reads, before) = {
        let mut c = a.config.lock().unwrap();
        c.metadata_delay_ms = 300;
        (
            c.metadata_reads.clone(),
            c.metadata_reads.load(Ordering::SeqCst),
        )
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        while reads.load(Ordering::SeqCst) == before {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    // A's GetAll is now in flight. B starts first, A starts last.
    b.status("Playing").await;
    a.status("Playing").await;
    wait_target(&mut state, Some(&a.target.bus_name)).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(matches!(&state.borrow().player, PlayerState::Following(p) if p.target == a.target));
    runner.abort();
}

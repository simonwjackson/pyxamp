//! The Jellyfin connector against a local Jellyfin 10.11.6 instance.
//!
//! Run with `daemon/tools/jellyfin-test` (starts the instance, a private
//! D-Bus session and a headless mpv). These tests are ignored in the plain
//! suite because they need that instance; they never touch the live server.
use pyxamp_daemon::{
    config::JellyfinConfig,
    engine::{self, EngineCommand, EngineEvent, EngineEventKind, EngineState},
    protocol::*,
    sources::{self, SourceHandle, SourceState, jellyfin},
};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

const WAIT: Duration = Duration::from_secs(20);

struct Env {
    url: String,
    key_file: String,
    user: String,
}

fn env() -> Env {
    let var = |name: &str| {
        std::env::var(name)
            .unwrap_or_else(|_| panic!("{name} is unset; run daemon/tools/jellyfin-test"))
    };
    Env {
        url: var("JELLYFIN_TEST_URL"),
        key_file: var("JELLYFIN_TEST_KEY_FILE"),
        user: var("JELLYFIN_TEST_USER"),
    }
}

/// A real engine actor with recording configuration: it accepts every
/// command, keeps `EngineState` like the mpv engine does, and lets the test
/// emit the engine's events.
struct RecordingEngine {
    commands: mpsc::UnboundedReceiver<(Option<SourceId>, EngineCommand)>,
    state: watch::Sender<EngineState>,
    events: tokio::sync::broadcast::Sender<EngineEvent>,
    /// The preloaded token, which takes over at the end of the current item.
    preloaded: std::sync::Arc<std::sync::Mutex<Option<u64>>>,
}

fn recording_engine() -> (engine::EngineHandle, RecordingEngine) {
    let (handle, mut ctx) = engine::channels();
    let (log, commands) = mpsc::unbounded_channel();
    let state = ctx.state.clone();
    let preloaded = std::sync::Arc::new(std::sync::Mutex::new(None));
    let next = preloaded.clone();
    tokio::spawn(async move {
        while let Some(request) = ctx.requests.recv().await {
            ctx.state.send_modify(|s| {
                s.status = EngineStatus::Ready(Empty {});
                match &request.command {
                    EngineCommand::Load {
                        token,
                        media,
                        start_ms,
                        paused,
                    } => {
                        s.owner = request.owner;
                        s.token = Some(*token);
                        s.track = Some(media.track.clone());
                        s.position_ms = Some(*start_ms);
                        s.transport = if *paused {
                            Transport::Paused
                        } else {
                            Transport::Playing
                        };
                    }
                    EngineCommand::Pause => s.transport = Transport::Paused,
                    EngineCommand::Play => s.transport = Transport::Playing,
                    EngineCommand::Stop => s.transport = Transport::Stopped,
                    EngineCommand::Seek { position_ms } => s.position_ms = Some(*position_ms),
                    EngineCommand::Volume { value } => s.volume = Some(*value),
                    EngineCommand::Preload { token, media } => {
                        *next.lock().unwrap() = media.as_ref().map(|_| *token);
                    }
                    _ => {}
                }
            });
            let _ = log.send((request.owner, request.command.clone()));
            let _ = request.reply.send(Ok(()));
        }
    });
    (
        handle.clone(),
        RecordingEngine {
            commands,
            state,
            events: handle.events,
            preloaded,
        },
    )
}

impl RecordingEngine {
    async fn expect(&mut self, what: &str, pick: impl Fn(&EngineCommand) -> bool) -> EngineCommand {
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let (owner, command) = tokio::time::timeout_at(deadline, self.commands.recv())
                .await
                .unwrap_or_else(|_| panic!("engine never got {what}"))
                .expect("engine log");
            assert_eq!(
                owner,
                Some(SourceId::Jellyfin),
                "engine commands name their owner"
            );
            if pick(&command) {
                return command;
            }
        }
    }

    fn quiet(&mut self) -> Vec<EngineCommand> {
        std::iter::from_fn(|| self.commands.try_recv().ok().map(|(_, c)| c)).collect()
    }

    fn emit(&self, token: u64, kind: EngineEventKind) {
        if kind == EngineEventKind::Ended {
            // The engine moves to the preloaded item, as mpv's gapless playlist does.
            let next = self.preloaded.lock().unwrap().take();
            self.state.send_modify(|s| {
                s.position_ms = Some(0);
                if next.is_some() {
                    s.token = next;
                }
            });
        }
        let _ = self.events.send(EngineEvent {
            owner: SourceId::Jellyfin,
            token,
            kind,
        });
    }
}

struct Harness {
    handle: SourceHandle,
    active: watch::Sender<Option<SourceId>>,
    engine: RecordingEngine,
    http: Http,
}

async fn start(user: Option<&str>) -> Harness {
    let env = env();
    let (engine_handle, engine) = recording_engine();
    let (active, active_rx) = watch::channel(None);
    let config = JellyfinConfig {
        url: env.url.clone(),
        api_key_file: env.key_file.clone().into(),
        user: user.unwrap_or(&env.user).into(),
    };
    let (handle, ctx) = sources::channels(SourceId::Jellyfin, config, engine_handle, active_rx);
    tokio::spawn(jellyfin::run(ctx));
    Harness {
        handle,
        active,
        engine,
        http: Http::new(&env).await,
    }
}

impl Harness {
    async fn state(&mut self, what: &str, pick: impl Fn(&SourceState) -> bool) -> SourceState {
        let mut rx = self.handle.state.clone();
        tokio::time::timeout(WAIT, async {
            loop {
                {
                    let state = rx.borrow_and_update();
                    if pick(&state) {
                        return state.clone();
                    }
                }
                rx.changed().await.expect("source stopped");
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "source never reached: {what}; last {:?}",
                self.handle.state.borrow()
            )
        })
    }

    async fn ready(&mut self) -> SourceState {
        self.state("ready", |s| {
            matches!(s.summary.status, SourceStatus::Ready(_))
        })
        .await
    }

    async fn request(&self, id: &str, action: SourceAction) -> Result<Option<Page>, Failure> {
        self.handle.request(id.into(), action).await
    }

    async fn open(&self, node: Option<&str>, cursor: Option<&str>) -> Page {
        self.request(
            "open",
            SourceAction::Open {
                source: SourceId::Jellyfin,
                node: node.map(Into::into),
                cursor: cursor.map(Into::into),
            },
        )
        .await
        .expect("open")
        .expect("a page")
    }

    async fn play(&self, item: &str, mode: PlayMode) -> Result<Option<Page>, Failure> {
        self.request(
            "play",
            SourceAction::Play {
                source: SourceId::Jellyfin,
                item: item.into(),
                mode,
            },
        )
        .await
    }
}

/// The test's own view of the server, through the same API key.
struct Http {
    client: reqwest::Client,
    url: String,
    key: String,
    user_id: String,
}

impl Http {
    async fn new(env: &Env) -> Self {
        let key = std::fs::read_to_string(&env.key_file)
            .unwrap()
            .trim()
            .to_owned();
        let mut http = Self {
            client: reqwest::Client::new(),
            url: env.url.clone(),
            key,
            user_id: String::new(),
        };
        let users = http.get("Users").await;
        http.user_id = users
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["Name"] == env.user.as_str())
            .unwrap()["Id"]
            .as_str()
            .unwrap()
            .into();
        http
    }

    async fn get(&self, path: &str) -> Value {
        self.client
            .get(format!("{}/{path}", self.url))
            .header(
                "Authorization",
                format!("MediaBrowser Token=\"{}\"", self.key),
            )
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn post(&self, path: &str, body: Value) {
        self.client
            .post(format!("{}/{path}", self.url))
            .header(
                "Authorization",
                format!("MediaBrowser Token=\"{}\"", self.key),
            )
            .json(&body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    async fn user_data(&self, id: &str) -> Value {
        self.get(&format!("UserItems/{id}/UserData?userId={}", self.user_id))
            .await
    }

    async fn reset(&self, id: &str) {
        self.post(
            &format!("UserItems/{id}/UserData?userId={}", self.user_id),
            serde_json::json!({"PlayCount": 0, "Played": false, "PlaybackPositionTicks": 0}),
        )
        .await;
    }

    /// The Pyxamp session's now-playing item name, if any.
    async fn now_playing(&self) -> Option<String> {
        let sessions = self.get("Sessions").await;
        sessions
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| {
                s["DeviceId"]
                    .as_str()
                    .is_some_and(|d| d.starts_with("pyxamp-"))
            })
            .find_map(|s| s["NowPlayingItem"]["Name"].as_str().map(str::to_owned))
    }

    async fn wait<T>(&self, what: &str, mut probe: impl AsyncFnMut(&Self) -> Option<T>) -> T {
        let deadline = tokio::time::Instant::now() + WAIT * 2;
        loop {
            if let Some(value) = probe(self).await {
                return value;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "server never showed {what}"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

fn jellyfin_id(item: &str) -> &str {
    item.split_once(':').unwrap().1
}

#[tokio::test]
#[ignore = "needs a local Jellyfin: daemon/tools/jellyfin-test"]
async fn groups_paging_and_search_come_from_the_server() {
    let mut h = start(None).await;
    let state = h.ready().await;
    let groups: Vec<(&str, GroupKind, Option<u64>)> = state
        .summary
        .groups
        .iter()
        .map(|g| (g.label.as_str(), g.kind, g.count))
        .collect();
    assert_eq!(
        groups,
        [
            ("Artists", GroupKind::Artists, Some(2)),
            ("Albums", GroupKind::Albums, Some(2)),
            ("Playlists", GroupKind::Playlists, Some(1)),
            ("Videos", GroupKind::Videos, Some(1)),
        ]
    );
    assert!(state.session.is_none(), "no queue before a play");

    let root = h.open(None, None).await;
    assert_eq!(root.items.len(), 4);
    assert!(root.items.iter().all(|i| i.openable && !i.playable));

    let artists = h.open(Some("artists"), None).await;
    let titles: Vec<&str> = artists.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Other Tones", "Test Tones"]);
    let albums = h.open(Some(&artists.items[1].id), None).await;
    assert_eq!(albums.items.len(), 1);
    assert_eq!(albums.items[0].title, "Sines");
    assert_eq!(albums.items[0].year, Some(2001));
    assert_eq!(albums.items[0].count, Some(3));

    let tracks = h.open(Some(&albums.items[0].id), None).await;
    let titles: Vec<&str> = tracks.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Low", "Middle", "High"]);
    assert_eq!(tracks.total, Some(3));
    assert_eq!(tracks.next_cursor, None);
    assert_eq!(tracks.items[0].duration_ms, Some(30_000));
    assert_eq!(tracks.items[0].subtitle.as_deref(), Some("Test Tones"));
    let rest = h.open(Some(&albums.items[0].id), Some("1")).await;
    let titles: Vec<&str> = rest.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Middle", "High"], "the cursor is an offset");

    let playlists = h.open(Some("playlists"), None).await;
    assert_eq!(playlists.items[0].title, "Mix");
    let mix = h.open(Some(&playlists.items[0].id), None).await;
    assert_eq!(mix.items.len(), 2);

    let videos = h.open(Some("videos"), None).await;
    assert_eq!(videos.items.len(), 1, "only video libraries");
    let clips = h.open(Some(&videos.items[0].id), None).await;
    assert_eq!(clips.items[0].kind, ItemKind::Video);
    assert!(clips.items[0].playable && !clips.items[0].openable);

    let found = h
        .request(
            "s",
            SourceAction::Search {
                query: "tones".into(),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(found.items.iter().any(|i| i.kind == ItemKind::Artist));
    let found = h
        .request(
            "s",
            SourceAction::Search {
                query: "pattern".into(),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.items.len(), 1);
    assert_eq!(found.items[0].kind, ItemKind::Video);

    let error = h
        .request(
            "bad",
            SourceAction::Open {
                source: SourceId::Jellyfin,
                node: Some("album:../../Users".into()),
                cursor: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "invalid");
}

#[tokio::test]
#[ignore = "needs a local Jellyfin: daemon/tools/jellyfin-test"]
async fn an_unknown_user_is_a_clear_unavailable_reason() {
    let mut h = start(Some("nobody")).await;
    let state = h
        .state("unavailable", |s| {
            matches!(s.summary.status, SourceStatus::Unavailable(_))
        })
        .await;
    let SourceStatus::Unavailable(failure) = state.summary.status else {
        unreachable!()
    };
    assert_eq!(failure.code, "noUser");
    assert!(failure.message.contains("nobody"));
}

#[tokio::test]
#[ignore = "needs a local Jellyfin: daemon/tools/jellyfin-test"]
async fn audio_plays_through_the_engine_and_reports_to_jellyfin() {
    let mut h = start(None).await;
    h.ready().await;
    let artists = h.open(Some("artists"), None).await;
    let albums = h.open(Some(&artists.items[1].id), None).await;
    let album = albums.items[0].id.clone();
    let tracks = h.open(Some(&album), None).await;
    let first = jellyfin_id(&tracks.items[0].id).to_owned();
    h.http.reset(&first).await;

    // Accepted while inactive: nothing reaches the engine.
    h.play(&album, PlayMode::Now).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        h.engine.quiet().is_empty(),
        "no engine command while inactive"
    );
    let state = h.state("queue", |s| s.session.is_some()).await;
    let session = state.session.unwrap();
    assert_eq!(session.queue.len(), 3);
    assert!(
        session
            .queue
            .iter()
            .all(|q| q.album_key == session.queue[0].album_key)
    );
    assert!(session.queue[0].album_key.is_some());
    assert_eq!(session.queue[0].status, QueueStatus::Current);

    // Active: load the first track with the stream URL and a comma-free header.
    h.active.send_replace(Some(SourceId::Jellyfin));
    let EngineCommand::Load {
        token,
        media,
        start_ms,
        paused,
    } = h
        .engine
        .expect("load", |c| matches!(c, EngineCommand::Load { .. }))
        .await
    else {
        unreachable!()
    };
    assert_eq!((start_ms, paused), (0, false));
    assert!(
        media
            .url
            .ends_with(&format!("/Audio/{first}/stream?static=true")),
        "{}",
        media.url
    );
    assert!(!media.url.contains(&h.http.key), "no key in the URL");
    assert_eq!(media.headers.len(), 1);
    assert!(!media.headers[0].1.contains(','));
    assert_eq!(media.track.title, "Low");
    let EngineCommand::Preload {
        token: next,
        media: Some(next_media),
    } = h
        .engine
        .expect("preload", |c| {
            matches!(c, EngineCommand::Preload { media: Some(_), .. })
        })
        .await
    else {
        unreachable!()
    };
    assert_eq!(next_media.track.title, "Middle");

    h.engine.emit(token, EngineEventKind::Started);
    let name = h
        .http
        .wait("now playing", async |http| {
            http.now_playing().await.filter(|n| n == "Low")
        })
        .await;
    assert_eq!(name, "Low");

    // Seek is fenced by occurrence.
    let current = h
        .state("current", |s| s.session.is_some())
        .await
        .session
        .unwrap();
    let occurrence = current.own.occurrence.clone().unwrap();
    assert!(current.own.capabilities.seek && current.own.capabilities.next);
    let stale = h
        .request(
            "seek",
            SourceAction::Seek {
                position_ms: 1000,
                occurrence: "jf999".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(stale.code, "stale");
    h.request(
        "seek",
        SourceAction::Seek {
            position_ms: 1000,
            occurrence,
        },
    )
    .await
    .unwrap();
    h.engine
        .expect("seek", |c| {
            matches!(c, EngineCommand::Seek { position_ms: 1000 })
        })
        .await;

    // Natural end: the preloaded track becomes current and the play counts.
    h.engine.emit(token, EngineEventKind::Ended);
    let state = h
        .state("second current", |s| {
            s.session
                .as_ref()
                .is_some_and(|x| x.own.track.title == "Middle")
        })
        .await;
    let queue = state.session.unwrap().queue;
    assert_eq!(queue[0].status, QueueStatus::Played);
    assert_eq!(queue[1].status, QueueStatus::Current);
    let data = h
        .http
        .wait("a recorded play", async |http| {
            let data = http.user_data(&first).await;
            (data["PlayCount"] == 1).then_some(data)
        })
        .await;
    assert_eq!(data["Played"], true);
    h.engine.emit(next, EngineEventKind::Started);
    let name = h
        .http
        .wait("the second track", async |http| {
            http.now_playing().await.filter(|n| n == "Middle")
        })
        .await;
    assert_eq!(name, "Middle");

    // The engine reports 12 s into the second track.
    h.engine.state.send_modify(|s| s.position_ms = Some(12_000));
    h.state("12 s", |s| {
        s.session
            .as_ref()
            .is_some_and(|x| x.own.position_ms == Some(12_000))
    })
    .await;
    // Leaving: the session pauses at the source's end and keeps the place.
    h.request(
        "internal:deactivate:x",
        SourceAction::Transport {
            action: OwnTransport::Pause,
        },
    )
    .await
    .unwrap();
    h.engine
        .expect("pause", |c| *c == EngineCommand::Pause)
        .await;
    h.active.send_replace(Some(SourceId::Pyxis));
    let state = h
        .state("paused", |s| {
            s.session
                .as_ref()
                .is_some_and(|x| x.own.transport == Transport::Paused)
        })
        .await;
    assert_eq!(state.session.unwrap().own.track.title, "Middle");
    let refused = h
        .request("v", SourceAction::Volume { value: 0.5 })
        .await
        .unwrap_err();
    assert_eq!(refused.code, "inactive", "no engine command while inactive");

    // Repeat and shuffle are real modes.
    h.request("r", SourceAction::Repeat { mode: Repeat::All })
        .await
        .unwrap();
    h.request("s", SourceAction::Shuffle { enabled: true })
        .await
        .unwrap();
    let own = h
        .state("modes", |s| {
            s.session
                .as_ref()
                .is_some_and(|x| x.own.shuffle == Some(true))
        })
        .await
        .session
        .unwrap()
        .own;
    assert_eq!(own.repeat, Some(Repeat::All));

    // Queueing next while inactive resumes the current one in place when active again.
    // Meanwhile another source took the engine.
    h.engine.state.send_modify(|s| {
        s.owner = Some(SourceId::Pyxis);
        s.token = Some(999);
    });
    h.play(&tracks.items[2].id, PlayMode::Next).await.unwrap();
    h.active.send_replace(Some(SourceId::Jellyfin));
    let EngineCommand::Load {
        media, start_ms, ..
    } = h
        .engine
        .expect("resume", |c| matches!(c, EngineCommand::Load { .. }))
        .await
    else {
        unreachable!()
    };
    assert_eq!(start_ms, 12_000, "resumes in place");
    assert_eq!(media.track.title, "Middle");
    let queue = h
        .state("4 entries", |s| {
            s.session.as_ref().is_some_and(|x| x.queue.len() == 4)
        })
        .await;
    let queue = queue.session.unwrap().queue;
    assert_eq!(
        queue[2].item.title, "High",
        "next goes after the current one"
    );
    let mut seen = std::collections::HashSet::new();
    assert!(
        queue.iter().all(|q| seen.insert(q.occurrence.clone())),
        "unique occurrences"
    );
}

#[tokio::test]
#[ignore = "needs a local Jellyfin: daemon/tools/jellyfin-test"]
async fn a_failed_track_retries_after_2_s_and_10_s_then_is_skipped() {
    let mut h = start(None).await;
    h.ready().await;
    let albums = h.open(Some("albums"), None).await;
    let squares = albums
        .items
        .iter()
        .find(|i| i.title == "Squares")
        .unwrap()
        .id
        .clone();
    h.active.send_replace(Some(SourceId::Jellyfin));
    h.play(&squares, PlayMode::Now).await.unwrap();
    let mut token = load_token(&mut h, "One").await;
    for (attempt, delay) in [(1u32, 2u64), (2, 10)] {
        let failed_at = std::time::Instant::now();
        h.engine.emit(
            token,
            EngineEventKind::Failed(Failure::new("stream", "test")),
        );
        let session = h
            .state("retrying", |s| {
                s.session
                    .as_ref()
                    .is_some_and(|x| x.own.retry.as_ref().is_some_and(|r| r.attempt == attempt))
            })
            .await
            .session
            .unwrap();
        assert_eq!(session.queue[0].status, QueueStatus::Retrying);
        token = load_token(&mut h, "One").await;
        let waited = failed_at.elapsed();
        assert!(
            waited >= Duration::from_secs(delay) && waited < Duration::from_secs(delay + 2),
            "attempt {attempt} waited {waited:?}"
        );
    }
    // Third failure: marked failed, and the next track loads.
    h.engine.emit(
        token,
        EngineEventKind::Failed(Failure::new("stream", "test")),
    );
    load_token(&mut h, "Two").await;
    let session = h
        .state("failed", |s| {
            s.session
                .as_ref()
                .is_some_and(|x| x.queue[0].status == QueueStatus::Failed)
        })
        .await
        .session
        .unwrap();
    assert!(session.own.retry.is_none());
    assert_eq!(session.queue[1].status, QueueStatus::Current);
}

async fn load_token(h: &mut Harness, title: &str) -> u64 {
    let EngineCommand::Load { token, media, .. } = h
        .engine
        .expect("load", |c| matches!(c, EngineCommand::Load { .. }))
        .await
    else {
        unreachable!()
    };
    assert_eq!(media.track.title, title);
    token
}

const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";

async fn mpris_proxy(bus: &zbus::Connection, name: &str, interface: &str) -> zbus::Proxy<'static> {
    zbus::proxy::Builder::new(bus)
        .destination(name.to_owned())
        .unwrap()
        .path(MPRIS_PATH)
        .unwrap()
        .interface(interface.to_owned())
        .unwrap()
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .unwrap()
}

/// The handed-off window's MPRIS name, once it is on the bus.
async fn mpv_player(bus: &zbus::Connection) -> String {
    let dbus = zbus::fdo::DBusProxy::new(bus).await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let names = dbus.list_names().await.unwrap();
        if let Some(name) = names
            .iter()
            .map(|n| n.to_string())
            .find(|n| n.starts_with("org.mpris.MediaPlayer2.mpv"))
        {
            return name;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no mpv MPRIS player appeared"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn status_becomes(player: &zbus::Proxy<'_>, want: &str) {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let status: String = player
            .get_property("PlaybackStatus")
            .await
            .unwrap_or_default();
        if status == want {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "PlaybackStatus stayed {status}, wanted {want}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn gone(bus: &zbus::Connection, name: &str) {
    let dbus = zbus::fdo::DBusProxy::new(bus).await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    while dbus.name_has_owner(name.try_into().unwrap()).await.unwrap() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the window did not close"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
#[ignore = "needs a local Jellyfin: daemon/tools/jellyfin-test"]
async fn a_video_opens_in_its_own_mpris_window_and_reports_to_jellyfin() {
    assert_eq!(
        std::env::var("JELLYFIN_TEST_PRIVATE_BUS").as_deref(),
        Ok("1"),
        "run on a private bus: daemon/tools/jellyfin-test"
    );
    let bus = zbus::Connection::session().await.unwrap();
    let mut h = start(None).await;
    h.ready().await;
    let views = h.open(Some("videos"), None).await;
    let clips = h.open(Some(&views.items[0].id), None).await;
    let video = clips.items[0].id.clone();
    let id = jellyfin_id(&video).to_owned();
    h.http.reset(&id).await;

    let refused = h.play(&video, PlayMode::Last).await.unwrap_err();
    assert_eq!(refused.code, "videoHandOff");
    h.active.send_replace(Some(SourceId::Jellyfin));
    h.play(&video, PlayMode::Now).await.unwrap();

    // A separate mpv window that is an MPRIS player owned by mpv itself.
    let name = mpv_player(&bus).await;
    let dbus = zbus::fdo::DBusProxy::new(&bus).await.unwrap();
    let pid = dbus
        .get_connection_unix_process_id(name.as_str().try_into().unwrap())
        .await
        .unwrap();
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap();
    assert!(
        comm.trim().starts_with("mpv"),
        "the MPRIS owner is mpv, not the daemon: {comm}"
    );
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap();
    assert!(
        !String::from_utf8_lossy(&cmdline).contains(&h.http.key),
        "the API key is not on mpv's command line"
    );
    let player = mpris_proxy(&bus, &name, "org.mpris.MediaPlayer2.Player").await;
    status_becomes(&player, "Playing").await;
    let metadata: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
        player.get_property("Metadata").await.unwrap();
    let title = String::try_from(metadata["xesam:title"].clone()).unwrap();
    assert_eq!(title, "Pattern");
    assert!(
        h.engine.quiet().is_empty(),
        "the engine is not used for video"
    );
    let playing = h
        .http
        .wait("video now playing", async |http| http.now_playing().await)
        .await;
    assert_eq!(playing, "Pattern");

    // Switching sources pauses the window and keeps its place.
    tokio::time::sleep(Duration::from_secs(3)).await;
    h.request(
        "internal:deactivate:v",
        SourceAction::Transport {
            action: OwnTransport::Pause,
        },
    )
    .await
    .unwrap();
    status_becomes(&player, "Paused").await;

    // Closing the window saves a resume position for the user.
    let root = mpris_proxy(&bus, &name, "org.mpris.MediaPlayer2").await;
    root.call_method("Quit", &()).await.unwrap();
    gone(&bus, &name).await;
    let ticks = h
        .http
        .wait("a resume position", async |http| {
            let ticks = http.user_data(&id).await["PlaybackPositionTicks"]
                .as_u64()
                .unwrap_or(0);
            (ticks > 0).then_some(ticks)
        })
        .await;
    assert!(ticks >= 20_000_000, "about 3 s played: {ticks}");
    h.http
        .wait("the play stopped", async |http| {
            http.now_playing().await.is_none().then_some(())
        })
        .await;

    // Playing it again resumes there; watching to the end counts as played.
    h.play(&video, PlayMode::Now).await.unwrap();
    let name = mpv_player(&bus).await;
    let player = mpris_proxy(&bus, &name, "org.mpris.MediaPlayer2.Player").await;
    status_becomes(&player, "Playing").await;
    // mpv-mpris can answer Position before the resume seek lands; give it up to 1 s, less than the 3 s
    // that would pass from playing at the start.
    let mut position: i64 = 0;
    for _ in 0..10 {
        position = player.get_property("Position").await.unwrap();
        if position >= (ticks / 10) as i64 - 500_000 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        position >= (ticks / 10) as i64 - 500_000,
        "resumed at {position} µs"
    );
    player.call_method("Seek", &(15_000_000i64,)).await.unwrap();
    gone(&bus, &name).await;
    let data = h
        .http
        .wait("played", async |http| {
            let data = http.user_data(&id).await;
            (data["Played"] == true).then_some(data)
        })
        .await;
    assert_eq!(data["PlayCount"], 1);
    assert_eq!(data["PlaybackPositionTicks"], 0);
}

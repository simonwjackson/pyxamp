//! The Pyxis source against a real, locally run Pyxis service (Pyxis `main`, wire v3).
//!
//! Needs `PYXIS_BIN` and `PYXIS_LAB_BIN` (the service and its laboratory plugin built
//! from the Pyxis repository). `daemon/tools/pyxis-test` builds both from the pinned
//! Pyxis revision and runs these tests. Each test starts its own service on loopback
//! with a temporary state directory and a `PATH` with no installed plugins, so nothing
//! reaches the live server or a physical speaker. Sources are the laboratory plugin
//! (search, albums, stations) and a laboratory output ("Test Room"), which records calls.
use pyxamp_daemon::{
    config::PyxisConfig,
    engine::{
        self, EngineCommand, EngineContext, EngineEvent, EngineEventKind, EngineHandle, EngineState,
    },
    protocol::*,
    session::{self, RequestEnvelope},
    sources::{self, SourceState},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{mpsc, oneshot, watch},
};

const AUDIO: &[u8] = b"pyxamp-test-audio-bytes";
const WAIT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------- the service

fn binary(var: &str) -> PathBuf {
    let path = std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{var} is not set; run daemon/tools/pyxis-test"));
    assert!(
        path.is_absolute() && path.exists(),
        "{var} must be an existing absolute path"
    );
    path
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A tiny HTTP server for the laboratory's remote stream URL.
async fn media_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: audio/mpeg\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    AUDIO.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(AUDIO).await;
            });
        }
    });
    port
}

struct Service {
    child: tokio::process::Child,
    url: String,
    dir: tempfile::TempDir,
    http: reqwest::Client,
    token: String,
    output_calls: PathBuf,
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn write_plugin(dir: &Path, name: &str, lab: &Path, env: &[(&str, String)]) {
    use std::os::unix::fs::PermissionsExt;
    let mut script = String::from("#!/bin/sh\n");
    for (key, value) in env {
        script.push_str(&format!("export {key}='{value}'\n"));
    }
    script.push_str(&format!("exec '{}'\n", lab.display()));
    let path = dir.join(name);
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Service {
    async fn start() -> Self {
        let pyxis = binary("PYXIS_BIN");
        let lab = binary("PYXIS_LAB_BIN");
        let media = media_server().await;
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let plugins = state.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let empty_path = dir.path().join("empty-path");
        std::fs::create_dir_all(&empty_path).unwrap();
        let output_calls = dir.path().join("output-calls.log");
        write_plugin(
            &plugins,
            "pyxis-plugin-lab-source",
            &lab,
            &[
                ("PYXIS_LAB_ID", "lab".into()),
                (
                    "PYXIS_LAB_SEARCH",
                    "Song|Lab Artist|Lab Album|180000".into(),
                ),
                ("PYXIS_LAB_RESULT_COPIES", "3".into()),
                ("PYXIS_LAB_ARTIST", "artist-1|Lab Artist".into()),
                (
                    "PYXIS_LAB_ALBUM",
                    "src-album|Source Album|Lab Artist".into(),
                ),
                ("PYXIS_LAB_STATION", "radio-1|Lab Radio".into()),
                ("PYXIS_LAB_STATION_LIST", "listed-1|Listed Radio".into()),
                ("PYXIS_LAB_STATION_SEEDS", "track,artist,album".into()),
                (
                    "PYXIS_LAB_STREAM_URL",
                    format!("http://127.0.0.1:{media}/audio.mp3"),
                ),
            ],
        );
        write_plugin(
            &plugins,
            "pyxis-plugin-lab-output",
            &lab,
            &[
                ("PYXIS_LAB_ID", "labout".into()),
                ("PYXIS_LAB_CAPABILITY", "output".into()),
                ("PYXIS_LAB_CALL_LOG", output_calls.display().to_string()),
            ],
        );
        let port = free_port();
        let child = tokio::process::Command::new(pyxis)
            .env_clear()
            .env("PATH", &empty_path)
            .env("HOME", dir.path())
            .env("PYXIS_HOST", "127.0.0.1")
            .env("PYXIS_PORT", port.to_string())
            .env("PYXIS_STATE_DIR", &state)
            .env("PYXIS_LOG", "warn")
            .kill_on_drop(true)
            .spawn()
            .expect("start pyxis");
        let url = format!("http://127.0.0.1:{port}");
        let http = reqwest::Client::new();
        for _ in 0..200 {
            if http
                .get(format!("{url}/healthz"))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let mut service = Self {
            child,
            url,
            dir,
            http,
            token: String::new(),
            output_calls,
        };
        let claim = service
            .rpc_as(
                None,
                json!({"_tag":"auth.device.claim","payload":{"name":"Desk"}}),
            )
            .await;
        assert_eq!(claim["outcome"]["status"], "ready", "{claim}");
        service.token = claim["outcome"]["value"]["bearerToken"]
            .as_str()
            .unwrap()
            .into();
        service
    }

    async fn rpc_as(&self, token: Option<&str>, body: Value) -> Value {
        let mut request = self
            .http
            .post(format!("{}/rpc", self.url))
            .header("x-pyxis-contract", "pyxis-rpc-v3")
            .json(&body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send().await.unwrap().json().await.unwrap()
    }

    async fn rpc(&self, body: Value) -> Value {
        self.rpc_as(Some(&self.token.clone()), body).await
    }

    fn token_file(&self, token: &str) -> PathBuf {
        let path = self
            .dir
            .path()
            .join(format!("token-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, format!("{token}\n")).unwrap();
        path
    }

    /// A second device, paired through the first: plays the part of a phone or a host.
    async fn pair(&self, name: &str) -> String {
        let code = self
            .rpc(json!({"_tag":"auth.pairing.create","payload":{}}))
            .await;
        let code = code["outcome"]["value"]["code"]
            .as_str()
            .unwrap()
            .to_owned();
        let paired = self
            .rpc_as(
                None,
                json!({"_tag":"auth.device.pair","payload":{"name":name,"code":code}}),
            )
            .await;
        paired["outcome"]["value"]["bearerToken"]
            .as_str()
            .unwrap()
            .into()
    }

    /// Search registers playable source tracks; save them as one library album.
    async fn seed_album(&self, title: &str) -> Vec<String> {
        let search = self
            .rpc(json!({"_tag":"source.search.run","payload":{"query":"song","limit":10}}))
            .await;
        let ids: Vec<String> = search["outcome"]["value"]["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(ids.len(), 3, "{search}");
        let tracks: Vec<Value> = ids
            .iter()
            .enumerate()
            .map(|(n, id)| {
                json!({"id":id,"title":format!("Track {}", n + 1),"artist":"Lab Artist",
                       "durationMs":180000,"trackNumber":n + 1})
            })
            .collect();
        let added = self
            .rpc(json!({"_tag":"library.album.add","payload":{
                "title":title,"artist":"Lab Artist","year":1997,"tracks":tracks}}))
            .await;
        assert_eq!(added["outcome"]["status"], "ready", "{added}");
        ids
    }

    async fn session(&self, id: &str) -> Value {
        let state = self
            .rpc(json!({"_tag":"session.state.get","payload":{"sessionId":id}}))
            .await;
        state["outcome"]["value"].clone()
    }
}

// ---------------------------------------------------------------- the engine

/// A real engine actor with configurable renderer behaviour. It enforces the engine's
/// ownership rule (`inactive`), fetches every loaded URL with its headers, and reports
/// what it did. `failures` makes loads of a track fail that many times.
#[derive(Clone, Default)]
struct Knobs {
    commands: Arc<Mutex<Vec<EngineCommand>>>,
    failures: Arc<Mutex<HashMap<String, u32>>>,
}
impl Knobs {
    fn loads(&self) -> Vec<(u64, String, u64)> {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                EngineCommand::Load {
                    token,
                    media,
                    start_ms,
                    ..
                } => Some((
                    *token,
                    media.track.id.clone().unwrap_or_default(),
                    *start_ms,
                )),
                _ => None,
            })
            .collect()
    }
    fn count(&self, matches: impl Fn(&EngineCommand) -> bool) -> usize {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| matches(c))
            .count()
    }
}

async fn run_engine(
    mut ctx: EngineContext,
    active: watch::Receiver<Option<SourceId>>,
    knobs: Knobs,
) {
    ctx.state
        .send_modify(|s| s.status = EngineStatus::Ready(Empty {}));
    let http = reqwest::Client::new();
    while let Some(request) = ctx.requests.recv().await {
        if request.owner.is_some() && request.owner != *active.borrow() {
            let _ = request
                .reply
                .send(Err(Failure::new("inactive", "not the active source")));
            continue;
        }
        knobs.commands.lock().unwrap().push(request.command.clone());
        let owner = request.owner;
        let mut event = None;
        match request.command {
            EngineCommand::Load {
                token,
                media,
                start_ms,
                ..
            } => {
                let id = media.track.id.clone().unwrap_or_default();
                let fail = {
                    let mut failures = knobs.failures.lock().unwrap();
                    match failures.get_mut(&id) {
                        Some(left) if *left > 0 => {
                            *left -= 1;
                            true
                        }
                        _ => false,
                    }
                };
                let mut fetch = http.get(&media.url);
                for (key, value) in &media.headers {
                    fetch = fetch.header(key, value);
                }
                let bytes = match fetch.send().await {
                    Ok(r) if r.status().is_success() => r.bytes().await.ok(),
                    _ => None,
                };
                let ok = !fail && bytes.as_deref() == Some(AUDIO);
                let mut track = media.track.clone();
                track.bitrate_kbps = Some(320.0);
                track.sample_rate_hz = Some(44100.0);
                track.duration_ms = track.duration_ms.or(Some(180_000));
                ctx.state.send_modify(|s| {
                    s.owner = owner;
                    s.token = Some(token);
                    s.transport = if ok {
                        Transport::Playing
                    } else {
                        Transport::Stopped
                    };
                    s.position_ms = Some(start_ms);
                    s.track = Some(track);
                });
                event = Some(EngineEvent {
                    owner: owner.unwrap(),
                    token,
                    kind: if ok {
                        EngineEventKind::Started
                    } else {
                        EngineEventKind::Failed(Failure::new("load", "renderer refused"))
                    },
                });
            }
            EngineCommand::Play => ctx.state.send_modify(|s| s.transport = Transport::Playing),
            EngineCommand::Pause => ctx.state.send_modify(|s| s.transport = Transport::Paused),
            EngineCommand::Stop => ctx.state.send_modify(|s| s.transport = Transport::Stopped),
            EngineCommand::Seek { position_ms } => {
                ctx.state.send_modify(|s| s.position_ms = Some(position_ms))
            }
            EngineCommand::Volume { value } => ctx.state.send_modify(|s| s.volume = Some(value)),
            _ => {}
        }
        let _ = request.reply.send(Ok(()));
        if let Some(event) = event {
            let _ = ctx.events.send(event);
        }
    }
}

/// The current track ran out on the renderer.
fn end_current(engine: &EngineHandle) {
    let state: EngineState = engine.state.borrow().clone();
    let _ = engine.events.send(EngineEvent {
        owner: SourceId::Pyxis,
        token: state.token.unwrap(),
        kind: EngineEventKind::Ended,
    });
}

// ---------------------------------------------------------------- Pyxamp

struct Pyxamp {
    requests: mpsc::Sender<RequestEnvelope>,
    snapshot: watch::Receiver<Snapshot>,
    source: watch::Receiver<SourceState>,
    engine: EngineHandle,
    knobs: Knobs,
    active: watch::Receiver<Option<SourceId>>,
}

impl Pyxamp {
    async fn start(url: &str, token_file: PathBuf, device_name: &str) -> Self {
        let (engine, engine_ctx) = engine::channels();
        let (active_tx, active) = watch::channel(None);
        let knobs = Knobs::default();
        tokio::spawn(run_engine(engine_ctx, active.clone(), knobs.clone()));
        let config = PyxisConfig {
            url: url.into(),
            token_file,
            device_name: Some(device_name.into()),
        };
        let (handle, ctx) =
            sources::channels(SourceId::Pyxis, config, engine.clone(), active.clone());
        let source = handle.state.clone();
        tokio::spawn(sources::pyxis::run(ctx));
        let (state_tx, snapshot) = watch::channel(Snapshot::default());
        let (requests, requests_rx) = mpsc::channel(8);
        tokio::spawn(session::run(
            vec![handle],
            engine.clone(),
            active_tx,
            state_tx,
            requests_rx,
        ));
        Self {
            requests,
            snapshot,
            source,
            engine,
            knobs,
            active,
        }
    }

    async fn request(&self, id: &str, action: Value) -> Result<Option<Page>, Failure> {
        let request: Request = serde_json::from_value(json!({"id": id, "action": action})).unwrap();
        request.validate()?;
        let (reply, response) = oneshot::channel();
        self.requests
            .send(RequestEnvelope { request, reply })
            .await
            .unwrap();
        tokio::time::timeout(WAIT, response)
            .await
            .expect("request timed out")
            .unwrap()
    }

    async fn ok(&self, action: Value) -> Option<Page> {
        match self
            .request(&uuid::Uuid::new_v4().to_string(), action.clone())
            .await
        {
            Ok(page) => page,
            Err(failure) => panic!("{action} failed: {failure:?}"),
        }
    }

    async fn status(&self) -> SourceStatus {
        let mut source = self.source.clone();
        let state = tokio::time::timeout(
            WAIT,
            source.wait_for(|s| !matches!(s.summary.status, SourceStatus::Connecting(_))),
        )
        .await
        .expect("source stayed connecting")
        .unwrap()
        .clone();
        state.summary.status
    }

    async fn ready(&self) {
        let status = self.status().await;
        assert!(matches!(status, SourceStatus::Ready(_)), "{status:?}");
    }

    async fn until(&self, what: &str, done: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let mut snapshot = self.snapshot.clone();
        match tokio::time::timeout(WAIT, snapshot.wait_for(|s| done(s))).await {
            Ok(Ok(s)) => s.clone(),
            _ => panic!(
                "timed out waiting for {what}: {:#?}",
                self.snapshot.borrow()
            ),
        }
    }
}

async fn until_core(
    service: &Service,
    session: &str,
    what: &str,
    done: impl Fn(&Value) -> bool,
) -> Value {
    for _ in 0..300 {
        let state = service.session(session).await;
        if done(&state) {
            return state;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "timed out waiting for core {what}: {}",
        service.session(session).await
    );
}

fn host_session(snapshot: &Snapshot) -> String {
    let own = snapshot.own.as_ref().expect("own playback");
    own.occurrence
        .as_ref()
        .expect("occurrence")
        .rsplit_once(':')
        .unwrap()
        .0
        .into()
}

async fn open(pyxamp: &Pyxamp, node: Option<&str>, cursor: Option<&str>) -> Page {
    let mut action = json!({"_tag":"library.open","payload":{"source":"pyxis"}});
    if let Some(node) = node {
        action["payload"]["node"] = json!(node);
    }
    if let Some(cursor) = cursor {
        action["payload"]["cursor"] = json!(cursor);
    }
    pyxamp.ok(action).await.expect("a page")
}

// ---------------------------------------------------------------- tests

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn groups_are_reported_as_the_core_orders_them_and_pages_follow_its_cursor() {
    let service = Service::start().await;
    for n in 0..105 {
        let added = service
            .rpc(json!({"_tag":"library.album.add","payload":{
                "title":format!("Album {n:03}"),"artist":"Pager","tracks":[{"title":"Only","artist":"Pager"}]}}))
            .await;
        assert_eq!(added["outcome"]["status"], "ready");
    }
    let core = service
        .rpc(json!({"_tag":"library.groups.list","payload":{}}))
        .await;
    let core_labels: Vec<String> = core["outcome"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["label"].as_str().unwrap().to_owned())
        .collect();
    let pyxamp = Pyxamp::start(&service.url, service.token_file(&service.token), "Desk").await;
    pyxamp.ready().await;
    let snapshot = pyxamp
        .until(
            "groups",
            |s| matches!(&s.library, Library::Sources { sources } if !sources[0].groups.is_empty()),
        )
        .await;
    let Library::Sources { sources } = &snapshot.library else {
        unreachable!()
    };
    let labels: Vec<String> = sources[0].groups.iter().map(|g| g.label.clone()).collect();
    assert_eq!(
        labels, core_labels,
        "groups exactly as the core reports them"
    );
    assert_eq!(
        sources[0].groups[0].kind,
        GroupKind::Albums,
        "album groups first"
    );
    assert_eq!(sources[0].groups[0].count, Some(105));

    let root = open(&pyxamp, None, None).await;
    assert_eq!(root.items.len(), core_labels.len());
    let albums = &sources[0].groups[0].id;
    let first = open(&pyxamp, Some(albums), None).await;
    assert_eq!(first.items.len(), 100);
    assert!(
        first
            .items
            .iter()
            .all(|i| i.kind == ItemKind::Album && i.openable)
    );
    let rest = open(&pyxamp, Some(albums), first.next_cursor.as_deref()).await;
    assert_eq!(rest.items.len(), 5);
    assert!(rest.next_cursor.is_none());
    let album = open(&pyxamp, Some(&first.items[0].id), None).await;
    assert_eq!(album.items[0].title, "Only");
    assert_eq!(album.items[0].subtitle.as_deref(), Some("Pager"));
}

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn host_plays_through_the_engine_and_reports_only_confirmed_state() {
    let service = Service::start().await;
    let tracks = service.seed_album("Dots and Loops").await;
    let pyxamp = Pyxamp::start(&service.url, service.token_file(&service.token), "Desk").await;
    pyxamp.ready().await;
    let hits = pyxamp
        .ok(json!({"_tag":"search","payload":{"query":"Dots"}}))
        .await
        .unwrap();
    let album = hits
        .items
        .iter()
        .find(|i| i.kind == ItemKind::Album && i.title == "Dots and Loops")
        .expect("library album in search");

    pyxamp
        .ok(json!({"_tag":"item.play","payload":{"source":"pyxis","item":album.id,"mode":"now"}}))
        .await;
    let snapshot = pyxamp
        .until("playing", |s| {
            s.own
                .as_ref()
                .is_some_and(|o| o.transport == Transport::Playing)
        })
        .await;
    assert_eq!(*pyxamp.active.borrow(), Some(SourceId::Pyxis));
    let own = snapshot.own.as_ref().unwrap();
    assert!(own.output.local && own.output.name == "Desk");
    assert_eq!(own.track.title, "Track 1");
    assert_eq!(
        own.track.bitrate_kbps,
        Some(320.0),
        "engine's measured format"
    );
    assert_eq!(snapshot.queue.len(), 3);
    assert_eq!(snapshot.queue[0].status, QueueStatus::Current);
    assert!(
        snapshot
            .queue
            .iter()
            .all(|q| q.album_key == snapshot.queue[0].album_key && q.album_key.is_some())
    );
    let loads = pyxamp.knobs.loads();
    assert_eq!(loads.len(), 1);
    assert_eq!(
        loads[0].1, tracks[0],
        "the engine fetched the core stream with the bearer token"
    );

    let session = host_session(&snapshot);
    let core = until_core(&service, &session, "playing", |s| {
        s["transport"] == "playing"
    })
    .await;
    assert_eq!(core["name"], "Desk");
    assert_eq!(core["cursor"], 0);

    // Seek: renderer first, then the core.
    let occurrence = own.occurrence.clone().unwrap();
    pyxamp
        .ok(json!({"_tag":"own.seek","payload":{"positionMs":5000,"occurrence":occurrence}}))
        .await;
    until_core(&service, &session, "seek", |s| s["positionMs"] == 5000).await;
    let stale = pyxamp
        .request(
            "stale",
            json!({"_tag":"own.seek","payload":{"positionMs":1,"occurrence":"other:0"}}),
        )
        .await;
    assert_eq!(stale.unwrap_err().code, "stale");

    // A switch away pauses here and keeps the place.
    pyxamp
        .ok(json!({"_tag":"own.transport","payload":{"action":"pause"}}))
        .await;
    let paused = until_core(&service, &session, "paused", |s| s["transport"] == "paused").await;
    assert_eq!(paused["positionMs"], 5000);
    assert_eq!(pyxamp.engine.state.borrow().transport, Transport::Paused);
    pyxamp
        .ok(json!({"_tag":"own.transport","payload":{"action":"play"}}))
        .await;
    until_core(&service, &session, "resumed", |s| {
        s["transport"] == "playing"
    })
    .await;

    // The renderer ends the track: the core advances and the next one loads from 0.
    end_current(&pyxamp.engine);
    until_core(&service, &session, "advanced", |s| {
        s["cursor"] == 1 && s["transport"] == "playing"
    })
    .await;
    pyxamp
        .until("second track", |s| {
            s.own.as_ref().is_some_and(|o| o.track.title == "Track 2")
        })
        .await;
    let loads = pyxamp.knobs.loads();
    assert_eq!((loads[1].1.as_str(), loads[1].2), (tracks[1].as_str(), 0));
    let listens = service
        .rpc(json!({"_tag":"listen.history.list","payload":{}}))
        .await;
    assert_eq!(
        listens["outcome"]["value"][0]["trackId"],
        tracks[0].as_str(),
        "{listens}"
    );

    // Volume goes to the engine and the core.
    pyxamp
        .ok(json!({"_tag":"own.volume","payload":{"value":0.4}}))
        .await;
    until_core(&service, &session, "volume", |s| s["volume"] == 40).await;
    assert_eq!(pyxamp.engine.state.borrow().volume, Some(0.4));
}

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn modes_queue_edits_and_playlists_go_through_the_core() {
    let service = Service::start().await;
    let tracks = service.seed_album("Modes").await;
    let pyxamp = Pyxamp::start(&service.url, service.token_file(&service.token), "Desk").await;
    pyxamp.ready().await;
    for track in &tracks {
        let item = format!("track:{track}");
        pyxamp
            .ok(json!({"_tag":"item.play","payload":{"source":"pyxis","item":item,"mode":"last"}}))
            .await;
    }
    let snapshot = pyxamp.until("queue", |s| s.queue.len() == 3).await;
    let session = host_session(&snapshot);

    for (mode, core) in [("all", "all"), ("one", "one"), ("off", "off")] {
        pyxamp
            .ok(json!({"_tag":"own.repeat","payload":{"mode":mode}}))
            .await;
        until_core(&service, &session, mode, |s| s["repeat"] == core).await;
    }
    let original: Vec<String> = snapshot
        .queue
        .iter()
        .map(|q| q.occurrence.clone())
        .collect();
    pyxamp
        .ok(json!({"_tag":"own.shuffle","payload":{"enabled":true}}))
        .await;
    until_core(&service, &session, "shuffle on", |s| s["shuffle"] == true).await;
    pyxamp
        .ok(json!({"_tag":"own.shuffle","payload":{"enabled":false}}))
        .await;
    until_core(&service, &session, "shuffle off", |s| s["shuffle"] == false).await;
    let restored = pyxamp
        .until("restored order", |s| {
            s.own.as_ref().is_some_and(|o| o.shuffle == Some(false))
        })
        .await;
    let order: Vec<String> = restored
        .queue
        .iter()
        .map(|q| q.occurrence.clone())
        .collect();
    assert_eq!(
        order, original,
        "turning shuffle off restores the original order"
    );

    // Jump plays that entry; remove drops one occurrence.
    pyxamp
        .ok(json!({"_tag":"queue.jump","payload":{"occurrence":original[2]}}))
        .await;
    until_core(&service, &session, "jumped", |s| {
        s["cursor"] == 2 && s["transport"] == "playing"
    })
    .await;
    pyxamp
        .ok(json!({"_tag":"queue.remove","payload":{"occurrence":original[0]}}))
        .await;
    until_core(&service, &session, "removed", |s| {
        s["queue"].as_array().unwrap().len() == 2
    })
    .await;

    // Play next: inserted after the current entry on a core that supports it.
    let next = pyxamp.request("n", json!({"_tag":"item.play","payload":{"source":"pyxis","item":format!("track:{}", tracks[0]),"mode":"next"}})).await;
    match next {
        Err(failure) => {
            assert!(!newer_core(), "this core should insert: {failure:?}");
            assert_eq!(failure.code, "unsupported", "{failure:?}")
        }
        Ok(_) => {
            assert!(newer_core(), "d4d1b0aa cannot insert");
            let core = until_core(&service, &session, "inserted", |s| {
                s["queue"].as_array().unwrap().len() == 3
            })
            .await;
            let cursor = core["cursor"].as_u64().unwrap() as usize;
            assert_eq!(core["queue"][cursor + 1], tracks[0].as_str());
            assert_eq!(
                core["transport"], "playing",
                "insert does not stop playback"
            );
        }
    }

    // Save the queue as a playlist, then append a track to it.
    pyxamp
        .ok(json!({"_tag":"playlist.save","payload":{"source":"pyxis","name":"Saved"}}))
        .await;
    let lists = service
        .rpc(json!({"_tag":"library.playlists.list","payload":{}}))
        .await;
    let saved = lists["outcome"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["title"] == "Saved")
        .expect("saved")
        .clone();
    let queue_now = service.session(&session).await["queue"].clone();
    assert_eq!(saved["trackIds"], queue_now);
    let playlist = format!("playlist:{}", saved["id"].as_str().unwrap());
    pyxamp.ok(json!({"_tag":"playlist.add","payload":{"source":"pyxis","playlist":playlist,"item":format!("track:{}", tracks[1])}})).await;
    let lists = service
        .rpc(json!({"_tag":"library.playlists.list","payload":{}}))
        .await;
    let saved = lists["outcome"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["title"] == "Saved")
        .unwrap()
        .clone();
    assert_eq!(
        saved["trackIds"].as_array().unwrap().last().unwrap(),
        tracks[1].as_str()
    );
    let opened = open(&pyxamp, Some(&playlist), None).await;
    assert_eq!(
        opened.items.len(),
        saved["trackIds"].as_array().unwrap().len()
    );
}

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn stations_start_from_a_song_or_an_artist_and_top_up_at_the_end() {
    let service = Service::start().await;
    let pyxamp = Pyxamp::start(&service.url, service.token_file(&service.token), "Desk").await;
    pyxamp.ready().await;
    let hits = pyxamp
        .ok(json!({"_tag":"search","payload":{"query":"song"}}))
        .await
        .unwrap();
    let song = hits
        .items
        .iter()
        .find(|i| i.kind == ItemKind::Track)
        .unwrap();
    let artist = hits
        .items
        .iter()
        .find(|i| i.kind == ItemKind::Artist)
        .unwrap();
    assert!(
        hits.items
            .iter()
            .any(|i| i.kind == ItemKind::Album && i.openable)
    );

    pyxamp
        .ok(json!({"_tag":"station.start","payload":{"source":"pyxis","seed":song.id}}))
        .await;
    let snapshot = pyxamp
        .until("station playing", |s| {
            s.own.as_ref().is_some_and(|o| {
                o.transport == Transport::Playing && o.track.title == "Station Song"
            })
        })
        .await;
    assert_eq!(snapshot.queue.len(), 3, "the first station batch");
    let session = host_session(&snapshot);
    // Seek on a station track: a newer core bounds it by the reported duration; d4d1b0aa
    // seeks library tracks only, and the renderer is put back where the session says.
    until_core(&service, &session, "duration reported", |s| {
        s["durationMs"] == 180000
    })
    .await;
    let occurrence = snapshot.own.as_ref().unwrap().occurrence.clone().unwrap();
    let seek = pyxamp
        .request(
            "seek",
            json!({"_tag":"own.seek","payload":{"positionMs":7000,"occurrence":occurrence}}),
        )
        .await;
    match seek {
        Ok(_) => {
            assert!(newer_core(), "d4d1b0aa cannot seek a station track");
            until_core(&service, &session, "station seek", |s| {
                s["positionMs"] == 7000
            })
            .await;
        }
        Err(failure) => {
            assert!(
                !newer_core(),
                "this core should seek station tracks: {failure:?}"
            );
            assert_eq!(failure.code, "seekRefused", "{failure:?}");
            assert!(failure.message.contains("library"), "{failure:?}");
            assert_ne!(pyxamp.engine.state.borrow().position_ms, Some(7000));
        }
    }
    let last = snapshot.queue[2].occurrence.clone();
    pyxamp
        .ok(json!({"_tag":"queue.jump","payload":{"occurrence":last}}))
        .await;
    until_core(&service, &session, "last station track", |s| {
        s["cursor"] == 2 && s["transport"] == "playing"
    })
    .await;
    // The last station track ends: the next batch is added before the core advances.
    end_current(&pyxamp.engine);
    until_core(&service, &session, "topped up", |s| {
        s["queue"].as_array().unwrap().len() == 6 && s["cursor"] == 3
    })
    .await;

    pyxamp
        .ok(json!({"_tag":"station.start","payload":{"source":"pyxis","seed":artist.id}}))
        .await;
    until_core(&service, &session, "artist station", |s| {
        s["queue"].as_array().unwrap().len() == 3 && s["cursor"] == 0
    })
    .await;

    let groups = open(&pyxamp, None, None).await;
    let stations = groups.items.iter().find(|i| i.title == "Stations").unwrap();
    let listed = open(&pyxamp, Some(&stations.id), None).await;
    let radio = listed
        .items
        .iter()
        .find(|i| i.title == "Listed Radio")
        .expect("listed station");
    pyxamp
        .ok(json!({"_tag":"item.play","payload":{"source":"pyxis","item":radio.id,"mode":"now"}}))
        .await;
    until_core(&service, &session, "listed station", |s| {
        s["transport"] == "playing"
    })
    .await;
}

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn engine_failures_retry_after_two_and_ten_seconds_then_skip() {
    let service = Service::start().await;
    let tracks = service.seed_album("Flaky").await;
    let pyxamp = Pyxamp::start(&service.url, service.token_file(&service.token), "Desk").await;
    pyxamp.ready().await;
    pyxamp
        .knobs
        .failures
        .lock()
        .unwrap()
        .insert(tracks[0].clone(), 3);
    let hits = pyxamp
        .ok(json!({"_tag":"search","payload":{"query":"Flaky"}}))
        .await
        .unwrap();
    let started = std::time::Instant::now();
    pyxamp.ok(json!({"_tag":"item.play","payload":{"source":"pyxis","item":hits.items[0].id,"mode":"now"}})).await;
    let first = pyxamp
        .until("retry 1", |s| {
            s.own
                .as_ref()
                .and_then(|o| o.retry.as_ref())
                .is_some_and(|r| r.attempt == 1)
        })
        .await;
    assert_eq!(first.queue[0].status, QueueStatus::Retrying);
    pyxamp
        .until("retry 2", |s| {
            s.own
                .as_ref()
                .and_then(|o| o.retry.as_ref())
                .is_some_and(|r| r.attempt == 2)
        })
        .await;
    assert!(started.elapsed() >= Duration::from_secs(2));
    let mut snapshot = pyxamp.snapshot.clone();
    let skipped = tokio::time::timeout(
        Duration::from_secs(20),
        snapshot.wait_for(|s| {
            s.queue
                .first()
                .is_some_and(|q| q.status == QueueStatus::Failed)
                && s.own
                    .as_ref()
                    .is_some_and(|o| o.transport == Transport::Playing)
        }),
    )
    .await
    .expect("skip")
    .unwrap()
    .clone();
    assert!(started.elapsed() >= Duration::from_secs(12));
    assert!(skipped.own.as_ref().unwrap().retry.is_none());
    assert_eq!(skipped.queue[1].status, QueueStatus::Current);
    assert_eq!(
        pyxamp
            .knobs
            .loads()
            .iter()
            .filter(|l| l.1 == tracks[0])
            .count(),
        3
    );
    let session = host_session(&skipped);
    until_core(&service, &session, "skipped", |s| {
        s["cursor"] == 1 && s["transport"] == "playing"
    })
    .await;
}

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn console_mode_drives_another_host_and_a_speaker_without_the_engine() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let service = Service::start().await;
    let tracks = service.seed_album("Console").await;
    // Another device hosts "Kitchen" while it holds a realtime socket.
    let kitchen_token = service.pair("Kitchen").await;
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("{}/realtime", service.url.replace("http", "ws")))
            .await
            .unwrap();
    socket.send(Message::Text(json!({"_tag":"realtime.hello","payload":{"contractId":"pyxis-rpc-v3","bearerToken":kitchen_token,"topics":["sessions"]}}).to_string().into())).await.unwrap();
    let welcome = socket.next().await.unwrap().unwrap();
    assert!(welcome.to_text().unwrap().contains("realtime.welcome"));
    let created = service
        .rpc_as(
            Some(&kitchen_token),
            json!({"_tag":"session.create","payload":{"name":"Kitchen"}}),
        )
        .await;
    let kitchen = created["outcome"]["value"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let pyxamp = Pyxamp::start(&service.url, service.token_file(&service.token), "Desk").await;
    pyxamp.ready().await;
    // Play on lists outputs before Pyxis is the active source (source state, not snapshot).
    let mut source = pyxamp.source.clone();
    let outputs = tokio::time::timeout(
        WAIT,
        source.wait_for(|s| {
            s.session.as_ref().is_some_and(|session| {
                session.outputs.iter().any(|o| o.name == "Kitchen")
                    && session.outputs.iter().any(|o| o.name == "Test Room")
            })
        }),
    )
    .await
    .expect("outputs")
    .unwrap()
    .session
    .clone()
    .unwrap()
    .outputs;
    let kitchen_output = outputs
        .iter()
        .find(|o| o.name == "Kitchen")
        .unwrap()
        .id
        .clone();
    pyxamp
        .ok(json!({"_tag":"output.select","payload":{"source":"pyxis","output":kitchen_output}}))
        .await;
    pyxamp.ok(json!({"_tag":"item.play","payload":{"source":"pyxis","item":format!("track:{}", tracks[0]),"mode":"now"}})).await;
    let mut directives = vec![];
    while directives.len() < 3 {
        let frame = tokio::time::timeout(WAIT, socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(text.as_str()).unwrap();
        if value["_tag"] == "realtime.command" {
            assert_eq!(value["payload"]["sessionId"], kitchen.as_str());
            directives.push(
                value["payload"]["command"]["_tag"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
        }
    }
    assert_eq!(directives, ["queue.clear", "queue.add", "transport.play"]);
    let shown = pyxamp
        .until("console output", |s| {
            s.own
                .as_ref()
                .is_some_and(|o| o.output.name == "Kitchen" && !o.output.local)
        })
        .await;
    assert!(shown.outputs.iter().any(|o| o.local && !o.active));
    assert_eq!(
        pyxamp.knobs.loads().len(),
        0,
        "console mode never loads the engine"
    );

    // A speaker output: the core creates its session and drives the laboratory renderer.
    let room = shown
        .outputs
        .iter()
        .find(|o| o.name == "Test Room")
        .unwrap()
        .id
        .clone();
    pyxamp
        .ok(json!({"_tag":"output.select","payload":{"source":"pyxis","output":room}}))
        .await;
    // The laboratory speaker accepts only mp3 and the laboratory source serves webm/opus, so
    // the core refuses Play after queueing. Pyxamp reports that refusal as it is.
    let refused = pyxamp
        .request(
            "speaker",
            json!({"_tag":"item.play","payload":{"source":"pyxis","item":format!("track:{}", tracks[1]),"mode":"now"}}),
        )
        .await
        .unwrap_err();
    assert_eq!(refused.code, "output.invalidResponse", "{refused:?}");
    assert!(
        refused.message.contains("no format accepted"),
        "{refused:?}"
    );
    let speaker = pyxamp
        .until("speaker output", |s| {
            s.own.as_ref().is_some_and(|o| o.output.name == "Test Room") && s.queue.len() == 1
        })
        .await;
    assert_eq!(
        speaker.queue[0].item.id,
        format!("track:{}", tracks[1]),
        "the core queued it on the speaker session"
    );
    let calls = std::fs::read_to_string(&service.output_calls).unwrap_or_default();
    assert!(
        calls.lines().any(|l| l.starts_with("stream.profile|")),
        "the core asked the laboratory speaker for its formats: {calls}"
    );
    assert_eq!(
        pyxamp
            .knobs
            .count(|c| matches!(c, EngineCommand::Load { .. })),
        0
    );

    pyxamp
        .ok(json!({"_tag":"output.select","payload":{"source":"pyxis","output":"local"}}))
        .await;
    pyxamp
        .until("local again", |s| {
            s.own.as_ref().is_some_and(|o| o.output.local)
        })
        .await;
}

#[tokio::test]
#[ignore = "needs PYXIS_BIN and PYXIS_LAB_BIN; run daemon/tools/pyxis-test"]
async fn an_api_token_cannot_host_and_says_so() {
    let service = Service::start().await;
    let grant = service.rpc(json!({"_tag":"auth.token.create","payload":{"name":"api","scopes":["library:read","session:read","session:control","source:read","account:read"]}})).await;
    let api = grant["outcome"]["value"]["bearerToken"]
        .as_str()
        .expect("api token")
        .to_owned();
    let pyxamp = Pyxamp::start(&service.url, service.token_file(&api), "Desk").await;
    let SourceStatus::Unavailable(failure) = pyxamp.status().await else {
        panic!("not unavailable")
    };
    assert_eq!(failure.code, "auth.notDevice");
    assert!(
        failure.message.contains("device token"),
        "{}",
        failure.message
    );
}

/// A recorded pre-v3 core: its public status names another contract.
async fn old_core() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8; 8192];
                let _ = socket.read(&mut buffer).await;
                let body = r#"{"_tag":"system.status.get","outcome":{"status":"ready","value":{"version":"2.0.0","contractId":"pyxis-rpc-v2","accountCount":1,"pluginCount":0,"capabilities":[]}}}"#;
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body.as_bytes()).await;
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn a_core_on_another_contract_is_unavailable_with_a_clear_reason() {
    let url = old_core().await;
    let dir = tempfile::tempdir().unwrap();
    let token = dir.path().join("token");
    std::fs::write(&token, "pyx_dev_test\n").unwrap();
    let pyxamp = Pyxamp::start(&url, token, "Desk").await;
    let SourceStatus::Unavailable(failure) = pyxamp.status().await else {
        panic!("not unavailable")
    };
    assert_eq!(failure.code, "contract.mismatch");
    assert!(
        failure.message.contains("pyxis-rpc-v3") && failure.message.contains("pyxis-rpc-v2"),
        "{}",
        failure.message
    );
    let refused = pyxamp
        .request(
            "r",
            json!({"_tag":"library.open","payload":{"source":"pyxis"}}),
        )
        .await;
    assert_eq!(refused.unwrap_err().code, "contract.mismatch");
}

#[tokio::test]
async fn a_missing_token_file_is_unavailable_without_leaking_anything() {
    let pyxamp = Pyxamp::start(
        "http://127.0.0.1:9",
        PathBuf::from("/nonexistent/pyxis-token"),
        "Desk",
    )
    .await;
    let SourceStatus::Unavailable(failure) = pyxamp.status().await else {
        panic!("not unavailable")
    };
    assert_eq!(failure.code, "config");
}

/// `PYXIS_NEWER_CORE=1`: the core under test has insert and duration-bounded seek (Pyxis U38).
fn newer_core() -> bool {
    std::env::var("PYXIS_NEWER_CORE").is_ok_and(|v| v == "1")
}

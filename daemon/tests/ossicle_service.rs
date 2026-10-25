//! The Ossicle source against a locally run Ossicle service.
//!
//! Each test starts `ossicle-server` (`$OSSICLE_SERVER_BIN`) on 127.0.0.1 with
//! a scratch cache, and one local fixture origin that serves: an RSS feed and
//! a YouTube-shaped Atom feed with generated WAV audio, SponsorBlock spans, and
//! the comics server's books and narration calls (Ossicle's `/comics` bridge
//! forwards there). YouTube audio comes from `daemon/tools/ossicle-local-ytdlp`.
//! The engine is a recording engine that keeps the engine contract: it
//! refuses inactive owners, fetches every loaded URL and checks the bytes are
//! the generated WAV, and reports position, end and failure when the test says.
//!
//! Run with `daemon/tools/ossicle-test`. No live Ossicle is written to.
use pyxamp_daemon::{
    config::OssicleConfig,
    engine::{
        self, EngineCommand, EngineContext, EngineEvent, EngineEventKind, EngineState, Media,
    },
    protocol::*,
    sources::{self, SourceHandle, SourceState, ossicle},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{mpsc, watch},
};

const WAIT: Duration = Duration::from_secs(15);

// ---------- generated audio ----------

/// 8 kHz mono 16-bit PCM sine, `seconds` long.
fn wav(seconds: u32, hz: f64) -> Vec<u8> {
    let rate = 8000u32;
    let samples = rate * seconds;
    let data_len = samples * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..samples {
        let v = (f64::from(n) / f64::from(rate) * hz * std::f64::consts::TAU).sin();
        out.extend_from_slice(&((v * 8000.0) as i16).to_le_bytes());
    }
    out
}

// ---------- fixture origin: feeds, audio, SponsorBlock, comics ----------

#[derive(Default)]
struct Comics {
    /// Stored reading position: (locator, revision, device).
    position: Option<(Value, u64, String)>,
    writes: Vec<Value>,
}

struct Fixture {
    base: String,
    comics: Arc<Mutex<Comics>>,
}

fn rss(base: &str) -> String {
    let item = |n: u32, day: u32, title: &str| {
        format!(
            r#"<item><title>{title}</title><guid isPermaLink="false">qh-{n}</guid>
<pubDate>Sun, {day:02} Aug 2026 06:00:00 GMT</pubDate><itunes:duration>120</itunes:duration>
<enclosure url="{base}/audio/qh-{n}.wav" length="1920044" type="audio/wav"/></item>"#
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"><channel>
<title>The Quiet Hour</title><link>https://quiethour.example</link><description>Hum.</description>
{}{}{}</channel></rss>"#,
        item(3, 16, "Episode 3: Fog"),
        item(2, 9, "Episode 2: Rain"),
        item(1, 2, "Episode 1: Hum"),
    )
}

fn atom() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
<id>yt:channel:testchannel0001</id><yt:channelId>testchannel0001</yt:channelId>
<title>Turbine Channel</title><link rel="alternate" href="https://www.youtube.com/channel/UCtestchannel0001"/>
<author><name>Turbine Channel</name><uri>https://www.youtube.com/channel/UCtestchannel0001</uri></author>
<published>2020-03-14T00:00:00+00:00</published>
<entry><id>yt:video:vid001spons</id><yt:videoId>vid001spons</yt:videoId><yt:channelId>UCtestchannel0001</yt:channelId>
<title>How turbines spin</title><link rel="alternate" href="https://www.youtube.com/watch?v=vid001spons"/>
<author><name>Turbine Channel</name><uri>https://www.youtube.com/channel/UCtestchannel0001</uri></author>
<published>2026-08-19T12:00:00+00:00</published><updated>2026-08-19T12:30:00+00:00</updated>
<media:group><media:title>How turbines spin</media:title><media:description>Blades.</media:description></media:group>
</entry></feed>"#
        .into()
}

fn timeline() -> Value {
    json!({
        "bookId": "book-1", "generation": "gen-1", "extractionVersion": "1",
        "normalizationVersion": "1", "indexDigest": "d", "estimatedDurationMs": 8000,
        "segments": (0..4).map(|n| json!({
            "ordinal": n, "resource": "ch1.xhtml", "selector": format!("#p{n}"),
            "startOffset": n * 10, "endOffset": n * 10 + 10,
            "estimatedStartMs": n * 2000, "estimatedDurationMs": 2000,
        })).collect::<Vec<_>>(),
    })
}

/// The comics contract's `narration.prepare`: two sentences per window here.
fn batch(from: u32) -> Value {
    let ordinals: Vec<u32> = (from..(from + 2).min(4)).collect();
    json!({
        "engine": {"modelRevision": "m", "modelDigest": "d", "voice": "af_bella", "voiceDigest": "v",
                   "languageCode": "en", "sampleRate": 8000, "format": "wav"},
        "segments": ordinals.iter().map(|n| json!({
            "id": format!("s{n}"), "ordinal": n, "resource": "ch1.xhtml", "selector": format!("#p{n}"),
            "startOffset": n * 10, "endOffset": n * 10 + 10, "text": format!("Sentence {n}."),
            "state": {"state": "ready", "value": {"audioUrl": format!("/narration/s{n}.wav"),
                "mediaType": "audio/wav", "bytes": 32044, "digest": "x", "durationMs": 2000, "sampleRate": 8000}},
        })).collect::<Vec<_>>(),
        "window": {"id": format!("w{from}"), "audioUrl": format!("/narration/w{from}.wav"), "mediaType": "audio/wav",
                   "bytes": 64044, "digest": "x", "durationMs": 4000, "sampleRate": 8000,
                   "startsMs": [0, 2000], "durationsMs": [2000, 2000]},
    })
}

fn comics_rpc(comics: &Mutex<Comics>, request: &Value) -> Value {
    let tag = request["_tag"].as_str().unwrap_or_default();
    let payload = &request["payload"];
    let outcome = match tag {
        "books.list" => json!({"status": "ready", "value": {"publications": [{
            "id": "book-1", "title": "The Lighthouse Keeper", "authors": ["A. Writer"], "language": "en",
            "year": 1999, "sourceName": "lighthouse.epub", "sourceSignature": "sig", "generation": "gen-1"}],
            "failures": []}}),
        "book.position.read" => match &comics.lock().unwrap().position {
            Some((locator, revision, device)) => json!({"status": "found", "value": {
                "bookId": "book-1", "locator": locator, "revision": revision, "deviceId": device,
                "updatedAt": "2026-10-25T00:00:00Z"}}),
            None => json!({"status": "absent"}),
        },
        "book.position.write" => {
            let mut comics = comics.lock().unwrap();
            comics.writes.push(payload.clone());
            let mut locator = payload.clone();
            for key in ["bookId", "deviceId", "force", "baseRevision"] {
                locator.as_object_mut().unwrap().remove(key);
            }
            let base = payload["baseRevision"].as_u64();
            let force = payload["force"].as_bool().unwrap_or(false);
            let device = payload["deviceId"].as_str().unwrap_or_default().to_owned();
            match comics.position.clone() {
                Some((theirs, revision, their_device)) if base != Some(revision) && !force => {
                    json!({"status": "conflict", "value": {
                        "theirs": {"bookId": "book-1", "locator": theirs, "revision": revision,
                                   "deviceId": their_device, "updatedAt": "2026-10-25T00:00:00Z"},
                        "yours": locator}})
                }
                existing => {
                    let revision = existing.map_or(1, |(_, r, _)| r + 1);
                    comics.position = Some((locator.clone(), revision, device.clone()));
                    json!({"status": "accepted", "value": {"bookId": "book-1", "locator": locator,
                        "revision": revision, "deviceId": device, "updatedAt": "2026-10-25T00:00:00Z"}})
                }
            }
        }
        "narration.timeline" => json!({"status": "ready", "value": timeline()}),
        "narration.prepare" => {
            let offset = payload["characterOffset"].as_u64().unwrap_or(0) as u32;
            json!({"status": "ready", "value": batch(offset / 10)})
        }
        _ => json!({"status": "unavailable", "value": {"message": "not in fixture"}}),
    };
    json!({"_tag": tag, "outcome": outcome})
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<(String, Vec<u8>)> {
    let mut buffer = vec![];
    let mut chunk = [0u8; 8192];
    loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buffer).to_string();
        let Some(end) = text.find("\r\n\r\n") else {
            continue;
        };
        let length = text[..end]
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        if buffer.len() >= end + 4 + length {
            return Some((
                text[..end].to_owned(),
                buffer[end + 4..end + 4 + length].to_vec(),
            ));
        }
    }
}

async fn fixture() -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let comics = Arc::new(Mutex::new(Comics::default()));
    let episode_audio = Arc::new(wav(120, 440.0));
    let window_audio = Arc::new(wav(4, 330.0));
    let (feed, shared) = (rss(&base), comics.clone());
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let (feed, comics) = (feed.clone(), shared.clone());
            let (episode_audio, window_audio) = (episode_audio.clone(), window_audio.clone());
            tokio::spawn(async move {
                let Some((head, body)) = read_request(&mut stream).await else {
                    return;
                };
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let route = path.split('?').next().unwrap_or("/");
                let (status, kind, bytes): (&str, &str, Vec<u8>) = match route {
                    "/feed.xml" => ("200 OK", "application/rss+xml", feed.into_bytes()),
                    "/yt.xml" => ("200 OK", "application/atom+xml", atom().into_bytes()),
                    r if r.starts_with("/audio/") => {
                        ("200 OK", "audio/wav", episode_audio.to_vec())
                    }
                    r if r.starts_with("/books/narration/") => {
                        ("200 OK", "audio/wav", window_audio.to_vec())
                    }
                    "/api/skipSegments" if path.contains("vid001spons") => (
                        "200 OK",
                        "application/json",
                        br#"[{"category":"sponsor","actionType":"skip","segment":[10,20]}]"#
                            .to_vec(),
                    ),
                    "/books/rpc" => {
                        let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                        let answer = comics_rpc(&comics, &request).to_string();
                        ("200 OK", "application/json", answer.into_bytes())
                    }
                    _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&bytes).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    Fixture { base, comics }
}

// ---------- local Ossicle ----------

struct Ossicle {
    child: Child,
    base: String,
    _dir: tempfile::TempDir,
    http: reqwest::Client,
}
impl Drop for Ossicle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Ossicle {
    async fn start(fixture: &Fixture) -> Self {
        let binary = PathBuf::from(
            std::env::var("OSSICLE_SERVER_BIN")
                .expect("OSSICLE_SERVER_BIN; run daemon/tools/ossicle-test"),
        );
        let local_ytdlp = std::env::var("OSSICLE_LOCAL_YTDLP").unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/ossicle-local-ytdlp")
                .display()
                .to_string()
        });
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("clip.wav");
        std::fs::write(&clip, wav(120, 550.0)).unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = Command::new(binary)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .arg("--cache")
            .arg(dir.path().join("cache"))
            .arg("--web")
            .arg(dir.path().join("web"))
            .env("YTDLP_BIN", local_ytdlp)
            .env("OSSICLE_LOCAL_YTDLP_CLIP", &clip)
            .env("SPONSORBLOCK_BASE", &fixture.base)
            .env("RUST_LOG", "ossicle_server=warn")
            .stdout(Stdio::null())
            .spawn()
            .expect("start ossicle-server");
        let ossicle = Self {
            child,
            base: format!("http://127.0.0.1:{port}"),
            _dir: dir,
            http: reqwest::Client::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while ossicle
            .http
            .get(format!("{}/healthz", ossicle.base))
            .send()
            .await
            .map_or(true, |r| !r.status().is_success())
        {
            assert!(Instant::now() < deadline, "ossicle-server did not start");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        ossicle
    }

    async fn rpc(&self, tag: &str, payload: Value) -> Value {
        let response: Value = self
            .http
            .post(format!("{}/rpc", self.base))
            .json(&json!({"_tag": tag, "payload": payload}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["_tag"], tag);
        response["outcome"].clone()
    }

    async fn settings(&self, books: &str) {
        let outcome = self
            .rpc(
                "settings.write",
                json!({"skipBackSeconds": 10, "skipForwardSeconds": 20, "playbackSpeed": 1.5,
                       "autoAdvanceQueue": true, "refreshMinutes": 30, "sponsorSkip": true,
                       "continuousPlayback": false, "booksUpstream": books}),
            )
            .await;
        assert_eq!(outcome["status"], "accepted", "{outcome}");
    }

    async fn subscribe(&self, url: &str) -> (String, Vec<Value>) {
        let outcome = self.rpc("feed.subscribe", json!({"url": url})).await;
        assert_eq!(outcome["status"], "subscribed", "{outcome}");
        let id = outcome["value"]["podcast"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let episodes = outcome["value"]["episodes"].as_array().unwrap().clone();
        (id, episodes)
    }

    async fn position(&self, episode: &str) -> Value {
        let outcome = self.rpc("position.list", json!({})).await;
        outcome["value"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["episodeId"] == episode)
            .cloned()
            .unwrap_or(Value::Null)
    }

    async fn wait_position(&self, episode: &str, test: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            let position = self.position(episode).await;
            if test(&position) {
                return position;
            }
            assert!(
                Instant::now() < deadline,
                "position never matched: {position}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

// ---------- recording engine ----------

enum Control {
    Position(u64),
    End,
    FailLoads(u32),
}

struct Engine {
    log: Arc<Mutex<Vec<EngineCommand>>>,
    control: mpsc::Sender<Control>,
}
impl Engine {
    fn commands(&self) -> Vec<EngineCommand> {
        self.log.lock().unwrap().clone()
    }
    fn clear(&self) {
        self.log.lock().unwrap().clear();
    }
    async fn wait(&self, what: &str, test: impl Fn(&EngineCommand) -> bool) -> EngineCommand {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(found) = self.commands().into_iter().find(|c| test(c)) {
                return found;
            }
            assert!(
                Instant::now() < deadline,
                "engine never got {what}: {:?}",
                self.commands()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    async fn send(&self, control: Control) {
        self.control.send(control).await.unwrap();
    }
}

async fn fetch_wav(http: &reqwest::Client, media: &Media) -> bool {
    let mut get = http.get(&media.url);
    for (key, value) in &media.headers {
        get = get.header(key, value);
    }
    match get.send().await {
        Ok(response) if response.status().is_success() => response
            .bytes()
            .await
            .is_ok_and(|bytes| bytes.starts_with(b"RIFF")),
        _ => false,
    }
}

fn start_engine(ctx: EngineContext, active: watch::Receiver<Option<SourceId>>) -> Engine {
    let log = Arc::new(Mutex::new(vec![]));
    let (control, mut controls) = mpsc::channel(8);
    let recorded = log.clone();
    tokio::spawn(async move {
        let EngineContext {
            mut requests,
            state,
            events,
        } = ctx;
        let http = reqwest::Client::new();
        let mut fail_loads = 0u32;
        let mut preload: Option<(u64, Media)> = None;
        state.send_replace(EngineState {
            status: EngineStatus::Ready(Empty {}),
            ..EngineState::unavailable("")
        });
        let emit = |token, kind| {
            let _ = events.send(EngineEvent {
                owner: SourceId::Ossicle,
                token,
                kind,
            });
        };
        loop {
            tokio::select! {
                Some(request) = requests.recv() => {
                    if request.owner.is_some() && request.owner != *active.borrow() {
                        let _ = request.reply.send(Err(Failure::new("inactive", "not the active source")));
                        continue;
                    }
                    recorded.lock().unwrap().push(request.command.clone());
                    let mut event = None;
                    match request.command {
                        EngineCommand::Load { token, media, start_ms, paused } => {
                            preload = None;
                            let playable = fetch_wav(&http, &media).await;
                            if fail_loads > 0 || !playable {
                                fail_loads = fail_loads.saturating_sub(1);
                                event = Some((token, EngineEventKind::Failed(Failure::new("load", "cannot open"))));
                            } else {
                                state.send_modify(|s| {
                                    s.owner = request.owner;
                                    s.token = Some(token);
                                    s.position_ms = Some(start_ms);
                                    s.transport = if paused { Transport::Paused } else { Transport::Playing };
                                    s.track = Some(Track { channels: Some(1), sample_rate_hz: Some(8000.0), ..media.track.clone() });
                                });
                                event = Some((token, EngineEventKind::Started));
                            }
                        }
                        EngineCommand::Preload { token, media } => preload = media.map(|m| (token, m)),
                        EngineCommand::Play => state.send_modify(|s| s.transport = Transport::Playing),
                        EngineCommand::Pause => state.send_modify(|s| s.transport = Transport::Paused),
                        EngineCommand::Stop => state.send_modify(|s| { s.transport = Transport::Stopped; s.token = None; }),
                        EngineCommand::Seek { position_ms } => state.send_modify(|s| s.position_ms = Some(position_ms)),
                        EngineCommand::Speed { rate } => state.send_modify(|s| s.speed = Some(rate)),
                        EngineCommand::Volume { value } => state.send_modify(|s| s.volume = Some(value)),
                        _ => {}
                    }
                    let _ = request.reply.send(Ok(()));
                    if let Some((token, kind)) = event { emit(token, kind); }
                }
                Some(control) = controls.recv() => match control {
                    Control::Position(ms) => state.send_modify(|s| s.position_ms = Some(ms)),
                    Control::FailLoads(n) => fail_loads = n,
                    Control::End => {
                        let Some(token) = state.borrow().token else { continue };
                        emit(token, EngineEventKind::Ended);
                        match preload.take() {
                            Some((next, media)) => {
                                assert!(fetch_wav(&http, &media).await, "preloaded window is not audio");
                                state.send_modify(|s| { s.token = Some(next); s.position_ms = Some(0); s.track = Some(media.track); });
                                emit(next, EngineEventKind::Started);
                            }
                            None => state.send_modify(|s| { s.token = None; s.transport = Transport::Stopped; }),
                        }
                    }
                },
                else => break,
            }
        }
    });
    Engine { log, control }
}

// ---------- the source under test ----------

struct Harness {
    source: SourceHandle,
    active: watch::Sender<Option<SourceId>>,
    engine: Engine,
    next_id: u32,
}
impl Harness {
    fn start(url: &str) -> Self {
        let (engine_handle, engine_ctx) = engine::channels();
        let (active, active_rx) = watch::channel(None);
        let engine = start_engine(engine_ctx, active_rx.clone());
        let config = OssicleConfig {
            url: url.to_owned(),
            token_file: None,
        };
        let (source, ctx) = sources::channels(SourceId::Ossicle, config, engine_handle, active_rx);
        tokio::spawn(ossicle::run(ctx));
        Self {
            source,
            active,
            engine,
            next_id: 0,
        }
    }

    async fn request(&mut self, action: Value) -> Result<Option<Page>, Failure> {
        self.next_id += 1;
        let request: Request =
            serde_json::from_value(json!({"id": format!("t{}", self.next_id), "action": action}))
                .unwrap();
        request.validate()?;
        self.source.request(request.id, request.action).await
    }

    async fn page(&mut self, action: Value) -> Page {
        self.request(action).await.unwrap().unwrap()
    }

    /// What the session does for `item.play`: answer, then make Ossicle active.
    async fn play(&mut self, item: &str) {
        let result = self
            .request(json!({"_tag": "item.play", "payload": {"source": "ossicle", "item": item, "mode": "now"}}))
            .await;
        assert!(result.is_ok(), "{result:?}");
        self.active.send_replace(Some(SourceId::Ossicle));
    }

    async fn transport(&mut self, action: &str) -> Result<Option<Page>, Failure> {
        self.request(json!({"_tag": "own.transport", "payload": {"action": action}}))
            .await
    }

    async fn state(&self, what: &str, test: impl Fn(&SourceState) -> bool) -> SourceState {
        let mut rx = self.source.state.clone();
        let deadline = Instant::now() + WAIT;
        loop {
            if test(&rx.borrow_and_update()) {
                return rx.borrow().clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                tokio::time::timeout(left, rx.changed()).await.is_ok(),
                "state never showed {what}: {:#?}",
                rx.borrow()
            );
        }
    }
}

fn own(state: &SourceState) -> Option<&OwnPlayback> {
    state.session.as_ref().map(|s| &s.own)
}

fn item_id(podcast: &str, episode: &Value) -> String {
    format!("episode:{podcast}:{}", episode["id"].as_str().unwrap())
}

fn device() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "pyxamp".into())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs OSSICLE_SERVER_BIN; run daemon/tools/ossicle-test"]
async fn episodes_resume_skip_speed_conflicts_played_retry_and_sponsor_skip() {
    let fixture = fixture().await;
    let server = Ossicle::start(&fixture).await;
    server.settings(&format!("{}/books", fixture.base)).await;
    let (show, episodes) = server
        .subscribe(&format!("{}/feed.xml", fixture.base))
        .await;
    let (yt_show, yt_episodes) = server.subscribe(&format!("{}/yt.xml", fixture.base)).await;
    assert_eq!(episodes.len(), 3);
    // Newest first: Fog (3), Rain (2), Hum (1).
    let ep = |n: usize| episodes[3 - n]["id"].as_str().unwrap().to_owned();
    let outcome = server
        .rpc(
            "podcast.settings.write",
            json!({"podcastId": show, "skipForwardSeconds": 45}),
        )
        .await;
    assert_eq!(outcome["status"], "accepted", "{outcome}");
    server
        .rpc("queue.write", json!({"episodeIds": [ep(2), ep(3)]}))
        .await;
    // Another device left episode 1 at 0:30.
    let outcome = server
        .rpc(
            "position.write",
            json!({"episodeId": ep(1), "seconds": 30, "deviceId": "phone"}),
        )
        .await;
    assert_eq!(outcome["status"], "accepted");

    let mut h = Harness::start(&server.base);
    let ready = h
        .state("ready", |s| {
            matches!(s.summary.status, SourceStatus::Ready(_))
        })
        .await;
    let groups: Vec<(&str, GroupKind, Option<u64>)> = ready
        .summary
        .groups
        .iter()
        .map(|g| (g.label.as_str(), g.kind, g.count))
        .collect();
    assert_eq!(
        groups,
        vec![
            ("Up next", GroupKind::UpNext, Some(2)),
            ("Shows", GroupKind::Shows, Some(2)),
            ("Books", GroupKind::Books, Some(1)),
        ]
    );

    // Library: root groups, shows, a show's episodes with resume progress.
    let page = h
        .page(json!({"_tag": "library.open", "payload": {"source": "ossicle"}}))
        .await;
    assert_eq!(page.items.len(), 3);
    assert!(
        page.items
            .iter()
            .all(|i| i.kind == ItemKind::Group && i.openable)
    );
    let page = h
        .page(json!({"_tag": "library.open", "payload": {"source": "ossicle", "node": "shows"}}))
        .await;
    let quiet = page
        .items
        .iter()
        .find(|i| i.title == "The Quiet Hour")
        .unwrap()
        .clone();
    assert_eq!(
        (quiet.kind, quiet.count, quiet.playable),
        (ItemKind::Show, Some(3), false)
    );
    let page = h
        .page(json!({"_tag": "library.open", "payload": {"source": "ossicle", "node": quiet.id}}))
        .await;
    assert_eq!(page.total, Some(3));
    let hum = page
        .items
        .iter()
        .find(|i| i.title == "Episode 1: Hum")
        .unwrap()
        .clone();
    assert_eq!(hum.id, item_id(&show, &episodes[2]));
    assert_eq!(hum.duration_ms, Some(120_000));
    assert_eq!(
        hum.progress,
        Some(Progress {
            played: false,
            position_ms: Some(30_000)
        })
    );
    let bad = h
        .request(json!({"_tag": "library.open", "payload": {"source": "ossicle", "node": "shows", "cursor": "x"}}))
        .await;
    assert_eq!(bad.unwrap_err().code, "invalid");
    let page = h
        .page(json!({"_tag": "search", "payload": {"query": "fog"}}))
        .await;
    let titles: Vec<&str> = page.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Episode 3: Fog"]);
    let page = h
        .page(json!({"_tag": "search", "payload": {"query": "lighthouse"}}))
        .await;
    assert_eq!(page.items[0].kind, ItemKind::Book);

    // Resume where the phone left it, at the account speed and the show's skip.
    h.play(&hum.id).await;
    let load = h
        .engine
        .wait("load", |c| matches!(c, EngineCommand::Load { .. }))
        .await;
    let EngineCommand::Load {
        media,
        start_ms,
        paused,
        ..
    } = load
    else {
        unreachable!()
    };
    assert_eq!((start_ms, paused), (30_000, false));
    assert!(
        media.url.starts_with(&server.base) && media.url.contains("/audio/"),
        "{}",
        media.url
    );
    assert_eq!(media.track.artist, "The Quiet Hour");
    h.engine
        .wait("speed", |c| *c == EngineCommand::Speed { rate: 1.5 })
        .await;
    let state = h
        .state("playing", |s| {
            own(s).is_some_and(|o| o.transport == Transport::Playing && o.track.channels.is_some())
        })
        .await;
    let o = own(&state).unwrap();
    assert_eq!(
        o.skip,
        Some(SkipIntervals {
            back_s: 10,
            forward_s: 45
        })
    );
    assert_eq!(o.speed, Some(1.5));
    assert!(!o.capabilities.repeat && !o.capabilities.shuffle && o.capabilities.seek);
    assert_eq!((o.repeat, o.shuffle), (None, None));
    let queue = &state.session.as_ref().unwrap().queue;
    let rows: Vec<(&str, QueueStatus)> = queue
        .iter()
        .map(|q| (q.item.title.as_str(), q.status))
        .collect();
    assert_eq!(
        rows,
        [
            ("Episode 1: Hum", QueueStatus::Current),
            ("Episode 2: Rain", QueueStatus::Pending),
            ("Episode 3: Fog", QueueStatus::Pending)
        ]
    );
    assert!(
        queue.iter().all(|q| q.album_key == queue[0].album_key),
        "album mode groups by show"
    );
    let repeat = h
        .request(json!({"_tag": "own.repeat", "payload": {"mode": "all"}}))
        .await;
    assert_eq!(repeat.unwrap_err().code, "unsupported");
    let shuffle = h
        .request(json!({"_tag": "own.shuffle", "payload": {"enabled": true}}))
        .await;
    assert_eq!(shuffle.unwrap_err().code, "unsupported");

    // Next and Previous skip by the intervals and sync the position.
    h.transport("next").await.unwrap();
    h.engine
        .wait("skip forward", |c| {
            *c == EngineCommand::Seek {
                position_ms: 75_000,
            }
        })
        .await;
    server.wait_position(&ep(1), |p| p["seconds"] == 75).await;
    h.transport("previous").await.unwrap();
    h.engine
        .wait("skip back", |c| {
            *c == EngineCommand::Seek {
                position_ms: 65_000,
            }
        })
        .await;
    h.transport("pause").await.unwrap();
    h.engine.wait("pause", |c| *c == EngineCommand::Pause).await;
    let saved = server.wait_position(&ep(1), |p| p["seconds"] == 65).await;
    assert_eq!(saved["deviceId"], device());

    // Speed goes to the engine and to Ossicle (the show has no speed override).
    h.request(json!({"_tag": "own.speed", "payload": {"rate": 2.0}}))
        .await
        .unwrap();
    h.engine
        .wait("speed 2", |c| *c == EngineCommand::Speed { rate: 2.0 })
        .await;
    let snapshot = server.rpc("settings.snapshot", json!({})).await;
    assert_eq!(snapshot["value"]["settings"]["playbackSpeed"], 2.0);

    // The phone moves on; our next write meets a conflict.
    let revision = saved["revision"].as_u64().unwrap();
    server
        .rpc(
            "position.write",
            json!({"episodeId": ep(1), "seconds": 100, "baseRevision": revision, "deviceId": "phone"}),
        )
        .await;
    let occurrence = format!("now:{}", hum.id);
    h.request(
        json!({"_tag": "own.seek", "payload": {"positionMs": 10_000, "occurrence": occurrence}}),
    )
    .await
    .unwrap();
    let state = h
        .state("conflict", |s| {
            s.session.as_ref().is_some_and(|s| !s.conflicts.is_empty())
        })
        .await;
    let session = state.session.as_ref().unwrap();
    assert_eq!(session.own.note.as_deref(), Some(ossicle::NOTE_CONFLICT));
    let conflict = session.conflicts[0].clone();
    let positions: Vec<(String, String, u64)> = conflict
        .positions
        .iter()
        .map(|p| (p.id.clone(), p.device.clone(), p.position_ms))
        .collect();
    assert_eq!(
        positions,
        [
            ("theirs".into(), "phone".into(), 100_000),
            ("yours".into(), device(), 10_000)
        ]
    );
    let stale = h
        .request(json!({"_tag": "own.seek", "payload": {"positionMs": 1, "occurrence": "now:old"}}))
        .await;
    assert_eq!(stale.unwrap_err().code, "stale");
    h.request(json!({"_tag": "conflict.resolve", "payload": {"source": "ossicle", "conflict": conflict.id, "keep": "yours"}}))
        .await
        .unwrap();
    server.wait_position(&ep(1), |p| p["seconds"] == 10).await;
    let state = h
        .state("resolved", |s| {
            s.session.as_ref().is_some_and(|s| s.conflicts.is_empty())
        })
        .await;
    assert_eq!(own(&state).unwrap().note, None);

    // Played flags; the end marks played and auto-advance passes the played entry.
    let rain = item_id(&show, &episodes[1]);
    h.request(json!({"_tag": "item.markPlayed", "payload": {"source": "ossicle", "item": rain, "played": true}}))
        .await
        .unwrap();
    server.wait_position(&ep(2), |p| p["played"] == true).await;
    h.transport("play").await.unwrap();
    h.engine.send(Control::End).await;
    server.wait_position(&ep(1), |p| p["played"] == true).await;
    h.state("episode 3 next", |s| {
        own(s).is_some_and(|o| o.track.title == "Episode 3: Fog")
    })
    .await;
    let queue = server.rpc("queue.list", json!({})).await;
    let left: Vec<String> = queue["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["episode"]["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(left, [ep(2)], "the advanced entry leaves up next");

    // Retry after 2 s and 10 s, then failed.
    h.engine.clear();
    h.engine.send(Control::FailLoads(3)).await;
    h.play(&rain).await;
    let state = h
        .state("retrying", |s| own(s).is_some_and(|o| o.retry.is_some()))
        .await;
    assert_eq!(own(&state).unwrap().retry.as_ref().unwrap().attempt, 1);
    assert_eq!(
        state.session.as_ref().unwrap().queue[0].status,
        QueueStatus::Retrying
    );
    let started = Instant::now();
    let failed = h
        .state("failed", |s| {
            s.session
                .as_ref()
                .is_some_and(|s| s.queue[0].status == QueueStatus::Failed)
        })
        .await;
    assert!(own(&failed).unwrap().retry.is_none());
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_secs(11),
        "retries waited {waited:?}"
    );
    let loads = h
        .engine
        .commands()
        .iter()
        .filter(|c| matches!(c, EngineCommand::Load { .. }))
        .count();
    assert_eq!(loads, 3, "three loads, each with a new token");

    // Sponsor skip on the YouTube episode.
    let turbine = item_id(&yt_show, &yt_episodes[0]);
    let yt_id = yt_episodes[0]["id"].as_str().unwrap().to_owned();
    h.engine.clear();
    h.play(&turbine).await;
    h.engine
        .wait("load turbine", |c| matches!(c, EngineCommand::Load { .. }))
        .await;
    h.state("turbine playing", |s| {
        own(s).is_some_and(|o| {
            o.track.title == "How turbines spin" && o.transport == Transport::Playing
        })
    })
    .await;
    h.engine.send(Control::Position(10_500)).await;
    h.engine
        .wait("sponsor jump", |c| {
            *c == EngineCommand::Seek {
                position_ms: 20_000,
            }
        })
        .await;
    h.state("sponsor note", |s| {
        own(s).is_some_and(|o| o.note.as_deref() == Some(ossicle::NOTE_SPONSOR))
    })
    .await;
    server.wait_position(&yt_id, |p| p["seconds"] == 20).await;
    h.state("note clears", |s| own(s).is_some_and(|o| o.note.is_none()))
        .await;

    // The session's pause before a switch keeps the place; switching back resumes it.
    h.engine.send(Control::Position(42_000)).await;
    h.state("at 0:42", |s| {
        own(s).is_some_and(|o| o.position_ms == Some(42_000))
    })
    .await;
    let paused = h
        .source
        .request(
            "internal:deactivate:x".into(),
            SourceAction::Transport {
                action: OwnTransport::Pause,
            },
        )
        .await;
    assert!(paused.is_ok());
    h.active.send_replace(Some(SourceId::Pyxis));
    server.wait_position(&yt_id, |p| p["seconds"] == 42).await;
    let kept = h
        .state("kept place", |s| {
            own(s).is_some_and(|o| o.transport == Transport::Paused)
        })
        .await;
    assert_eq!(own(&kept).unwrap().position_ms, Some(42_000));
    h.engine.clear();
    h.play(&turbine).await;
    let load = h
        .engine
        .wait("resume", |c| matches!(c, EngineCommand::Load { .. }))
        .await;
    assert!(
        matches!(
            load,
            EngineCommand::Load {
                start_ms: 42_000,
                paused: false,
                ..
            }
        ),
        "{load:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs OSSICLE_SERVER_BIN; run daemon/tools/ossicle-test"]
async fn books_narrate_window_by_window_and_sync_the_reading_position() {
    let fixture = fixture().await;
    let server = Ossicle::start(&fixture).await;
    server.settings(&format!("{}/books", fixture.base)).await;
    let mut h = Harness::start(&server.base);
    h.state("ready", |s| {
        s.summary
            .groups
            .iter()
            .any(|g| g.kind == GroupKind::Books && g.count == Some(1))
    })
    .await;
    let page = h
        .page(json!({"_tag": "library.open", "payload": {"source": "ossicle", "node": "books"}}))
        .await;
    let book = page.items[0].clone();
    assert_eq!(
        (book.kind, book.title.as_str(), book.subtitle.as_deref()),
        (ItemKind::Book, "The Lighthouse Keeper", Some("A. Writer"))
    );
    let queued = h
        .request(json!({"_tag": "item.play", "payload": {"source": "ossicle", "item": book.id, "mode": "next"}}))
        .await;
    assert!(
        queued.is_ok(),
        "nothing plays yet, so next plays now: {queued:?}"
    );
    h.active.send_replace(Some(SourceId::Ossicle));

    let load = h
        .engine
        .wait("first window", |c| matches!(c, EngineCommand::Load { .. }))
        .await;
    let EngineCommand::Load {
        media, start_ms, ..
    } = load
    else {
        unreachable!()
    };
    assert!(
        media.url.ends_with("/comics/narration/w0.wav"),
        "{}",
        media.url
    );
    assert_eq!(start_ms, 0);
    let preload = h
        .engine
        .wait("next window", |c| {
            matches!(c, EngineCommand::Preload { media: Some(_), .. })
        })
        .await;
    let EngineCommand::Preload {
        media: Some(next), ..
    } = preload
    else {
        unreachable!()
    };
    assert!(
        next.url.ends_with("/comics/narration/w2.wav"),
        "{}",
        next.url
    );
    let state = h
        .state("book playing", |s| {
            own(s).is_some_and(|o| o.transport == Transport::Playing)
        })
        .await;
    let session = state.session.as_ref().unwrap();
    assert_eq!(session.queue.len(), 1, "a book is one row");
    assert_eq!(session.own.track.duration_ms, Some(4000));

    // Second sentence of the first window, then the preloaded window takes over.
    h.engine.send(Control::Position(2500)).await;
    h.state("second sentence", |s| {
        own(s).is_some_and(|o| o.position_ms == Some(2500))
    })
    .await;
    h.engine.send(Control::End).await;
    h.state("second window", |s| {
        own(s).is_some_and(|o| o.position_ms == Some(0) && o.transport == Transport::Playing)
    })
    .await;
    let deadline = Instant::now() + WAIT;
    loop {
        let writes = fixture.comics.lock().unwrap().writes.clone();
        if writes.iter().any(|w| w["selector"] == "#p1") {
            assert_eq!(writes.last().unwrap()["deviceId"], device().as_str());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "reading position never written: {writes:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    h.transport("pause").await.unwrap();
    let stored = fixture.comics.lock().unwrap().position.clone().unwrap();
    assert_eq!(
        stored.0["selector"], "#p2",
        "pause writes the sounding sentence"
    );
    let played = h
        .request(json!({"_tag": "item.markPlayed", "payload": {"source": "ossicle", "item": book.id, "played": true}}))
        .await;
    assert_eq!(played.unwrap_err().code, "unsupported");

    // Another device reads on; Pyxamp's next write is a conflict, kept as theirs.
    {
        let mut comics = fixture.comics.lock().unwrap();
        let (mut locator, revision, _) = comics.position.clone().unwrap();
        locator["selector"] = json!("#p3");
        locator["characterOffset"] = json!(30);
        comics.position = Some((locator, revision + 1, "tablet".into()));
    }
    h.transport("play").await.unwrap();
    h.engine.send(Control::Position(2100)).await;
    h.state("at second sentence", |s| {
        own(s).is_some_and(|o| o.position_ms == Some(2100))
    })
    .await;
    h.transport("pause").await.unwrap();
    let state = h
        .state("book conflict", |s| {
            s.session.as_ref().is_some_and(|s| !s.conflicts.is_empty())
        })
        .await;
    assert_eq!(
        own(&state).unwrap().note.as_deref(),
        Some(ossicle::NOTE_CONFLICT)
    );
    let conflict = state.session.as_ref().unwrap().conflicts[0].clone();
    assert_eq!(conflict.positions[0].device, "tablet");
    assert_eq!(
        conflict.positions[0].position_ms, 6000,
        "the timeline's estimate of sentence 3"
    );
    h.engine.clear();
    h.request(json!({"_tag": "conflict.resolve", "payload": {"source": "ossicle", "conflict": conflict.id, "keep": "theirs"}}))
        .await
        .unwrap();
    // Their sentence is 3; the fixture's window that starts there is "w3".
    let load = h
        .engine
        .wait("their sentence", |c| {
            matches!(c, EngineCommand::Load { .. })
        })
        .await;
    let EngineCommand::Load { media, paused, .. } = load else {
        unreachable!()
    };
    assert!(
        media.url.ends_with("/comics/narration/w3.wav"),
        "{}",
        media.url
    );
    assert!(paused, "resolving does not start playback");
    let state = h
        .state("resolved", |s| {
            s.session.as_ref().is_some_and(|s| s.conflicts.is_empty())
        })
        .await;
    assert_eq!(own(&state).unwrap().note, None);
}

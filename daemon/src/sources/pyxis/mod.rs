//! Pyxis source: Pyxamp as a Pyxis playback host and as a console.
//!
//! The actor talks `pyxis-rpc-v3` (HTTP RPC plus the realtime socket). In host mode it
//! owns a Pyxis session named for this desktop, renders it through the engine and
//! reports only what the engine confirmed (`session.command.run`). In console mode it
//! drives another session (another host, or a speaker output such as Sonos) with
//! `session.command.send` and never touches the engine. The core owns the queue, its
//! order and its modes; this module never keeps a queue of its own.
mod client;
mod library;
mod realtime;
mod wire;

use super::{SourceContext, SourceRequest, SourceSession, SourceState};
use crate::{
    config::PyxisConfig,
    engine::{EngineCommand, EngineEvent, EngineEventKind, EngineHandle, Media},
    protocol::{
        Capabilities, Empty, Failure, Group, Item, ItemKind, Output, OwnPlayback, OwnTransport,
        Page, PlayMode, QueueItem, QueueStatus, Repeat, Retry, SourceAction, SourceId,
        SourceStatus, SourceSummary, Track, Transport,
    },
};
use client::Client;
use library::{Catalog, Node, Seed};
use realtime::Heard;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, watch};
use wire::{
    Command, Directive, GroupPage, LibraryAlbum, LibraryGroup, OutputTopology, Playlist, Plugin,
    RepeatMode, Request, SearchResult, Session, SessionTransport, SourceAlbum, Station,
    StationBatch, StationSeed,
};

const ME: SourceId = SourceId::Pyxis;
const PAGE: usize = 100;
const POSITION_REPORT_EVERY: Duration = Duration::from_secs(15);
/// Speaker discovery runs on the Pyxis server; keep it infrequent.
const TARGETS_REFRESH_EVERY: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const RETRY_MISMATCH_AFTER: Duration = Duration::from_secs(60);

pub use crate::sources::retry_delay;

pub async fn run(ctx: SourceContext<PyxisConfig>) -> anyhow::Result<()> {
    let SourceContext {
        source: _,
        config,
        engine,
        active,
        state,
        mut requests,
    } = ctx;
    let mut events = engine.events.subscribe();
    let mut engine_state = engine.state.clone();
    let mut engine_alive = true;
    let mut active_rx = active.clone();
    let (heard_tx, mut heard_rx) = mpsc::channel(64);
    let mut actor = Actor::new(config, engine, active, state, heard_tx);
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    actor.publish();
    loop {
        let retry_at = actor.retry.as_ref().map(|retry| retry.at);
        let connect_at = actor.connect_at;
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { break };
                actor.request(request).await;
            }
            Some((generation, heard)) = heard_rx.recv() => {
                if generation == actor.generation {
                    actor.heard(heard).await;
                }
            }
            event = events.recv() => match event {
                Ok(event) => actor.engine_event(event).await,
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => {
                    events = actor.engine.events.subscribe();
                }
            },
            changed = active_rx.changed() => {
                if changed.is_err() { break }
                let now = *active_rx.borrow_and_update();
                actor.activated(now).await;
            }
            _ = sleep_until(retry_at), if retry_at.is_some() => actor.retry_now().await,
            _ = sleep_until(connect_at), if connect_at.is_some() => {
                actor.connect_at = None;
                actor.connect().await;
            }
            changed = engine_state.changed(), if engine_alive => {
                if changed.is_err() {
                    engine_alive = false;
                } else {
                    actor.engine_changed().await;
                }
            }
            _ = tick.tick() => actor.tick().await,
        }
    }
    actor.shutdown();
    Ok(())
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at.into()).await,
        None => std::future::pending().await,
    }
}

/// The engine side of host mode. `token` names one load.
#[derive(Clone, Debug, PartialEq)]
enum Render {
    Idle,
    Loading {
        token: u64,
        ordinal: u32,
        track: String,
        /// Report `transport.play` once the engine starts, with this command id.
        report: Option<Option<String>>,
    },
    Loaded {
        token: u64,
        ordinal: u32,
        track: String,
    },
}

struct PendingRetry {
    ordinal: u32,
    attempt: u32,
    at: Instant,
    at_wall_ms: u64,
    report: Option<Option<String>>,
}

struct StationRun {
    plugin: String,
    station: String,
    cursor: Option<String>,
    exhausted: bool,
}

struct Actor {
    config: PyxisConfig,
    engine: EngineHandle,
    active: watch::Receiver<Option<SourceId>>,
    state: watch::Sender<SourceState>,
    heard_tx: mpsc::Sender<(u64, Heard)>,
    name: String,
    client: Option<Client>,
    status: SourceStatus,
    connect_at: Option<Instant>,
    backoff: Duration,
    generation: u64,
    socket: Option<tokio::task::JoinHandle<()>>,
    groups: Vec<Group>,
    library_dirty: bool,
    catalog: Catalog,
    sessions: HashMap<String, Session>,
    host: Option<String>,
    device: Option<String>,
    console: Option<String>,
    targets: Vec<(String, wire::OutputGroup)>,
    targets_at: Option<Instant>,
    render: Render,
    next_token: u64,
    play_when_active: bool,
    /// Bumped to ask the session to make Pyxis active (a console asked this host to play).
    activation: u64,
    retry: Option<PendingRetry>,
    failed: HashSet<u32>,
    station: Option<StationRun>,
    reported_at: Instant,
    directives: VecDeque<String>,
    /// The last engine transport reported for a load the session paused or resumed (yield).
    synced: Option<(u64, Transport)>,
}

fn unsupported(what: &str) -> Failure {
    Failure::new("unsupported", format!("Pyxis does not support {what}"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// A ULID (the core requires one for listen event ids): 48-bit time, 80 random bits.
fn ulid() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let random = uuid::Uuid::new_v4().as_u128() & ((1u128 << 80) - 1);
    let value = (u128::from(now_ms() & ((1 << 48) - 1)) << 80) | random;
    (0..26)
        .rev()
        .map(|digit| ALPHABET[((value >> (digit * 5)) & 31) as usize] as char)
        .collect()
}

/// RFC 3339 UTC timestamp for listen events.
fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant), proleptic Gregorian, UTC.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn host_name() -> String {
    let mut buffer = [0u8; 256];
    // SAFETY: the buffer is valid for its length; gethostname NUL-terminates on success.
    let result = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    if result == 0 {
        let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
        if let Ok(name) = std::str::from_utf8(&buffer[..end])
            && !name.is_empty()
        {
            return name.to_owned();
        }
    }
    "Pyxamp".into()
}

fn repeat_from(mode: RepeatMode) -> Repeat {
    match mode {
        RepeatMode::Off => Repeat::Off,
        RepeatMode::All => Repeat::All,
        RepeatMode::One => Repeat::One,
    }
}

fn repeat_to(mode: Repeat) -> RepeatMode {
    match mode {
        Repeat::Off => RepeatMode::Off,
        Repeat::All => RepeatMode::All,
        Repeat::One => RepeatMode::One,
    }
}

fn transport_from(transport: SessionTransport) -> Transport {
    match transport {
        SessionTransport::Playing => Transport::Playing,
        SessionTransport::Paused => Transport::Paused,
        SessionTransport::Stopped | SessionTransport::Ended => Transport::Stopped,
    }
}

fn occurrence(session: &Session, ordinal: u32) -> String {
    format!("{}:{ordinal}", session.id)
}

/// The effective index of an occurrence, if it still belongs to this session.
fn index_of(session: &Session, occurrence: &str) -> Option<usize> {
    let (id, ordinal) = occurrence.rsplit_once(':')?;
    if id != session.id {
        return None;
    }
    let ordinal: u32 = ordinal.parse().ok()?;
    session.ordinals().iter().position(|o| *o == ordinal)
}

fn stale_occurrence() -> Failure {
    Failure::new("stale", "that queue entry is no longer in the Pyxis queue")
}

impl Actor {
    fn new(
        config: PyxisConfig,
        engine: EngineHandle,
        active: watch::Receiver<Option<SourceId>>,
        state: watch::Sender<SourceState>,
        heard_tx: mpsc::Sender<(u64, Heard)>,
    ) -> Self {
        let name = config
            .device_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(host_name);
        Self {
            config,
            engine,
            active,
            state,
            heard_tx,
            name,
            client: None,
            status: SourceStatus::Connecting(Empty {}),
            connect_at: Some(Instant::now()),
            backoff: Duration::from_secs(2),
            generation: 0,
            socket: None,
            groups: vec![],
            library_dirty: false,
            catalog: Catalog::default(),
            sessions: HashMap::new(),
            host: None,
            device: None,
            console: None,
            targets: vec![],
            targets_at: None,
            render: Render::Idle,
            next_token: 0,
            play_when_active: false,
            activation: 0,
            retry: None,
            failed: HashSet::new(),
            station: None,
            reported_at: Instant::now(),
            directives: VecDeque::new(),
            synced: None,
        }
    }

    fn shutdown(&mut self) {
        if let Some(socket) = self.socket.take() {
            socket.abort();
        }
    }

    fn client(&self) -> Result<&Client, Failure> {
        match (&self.status, &self.client) {
            (SourceStatus::Ready(_), Some(client)) => Ok(client),
            (SourceStatus::Unavailable(failure), _) => Err(failure.clone()),
            _ => Err(Failure::new("connecting", "Pyxis is still connecting")),
        }
    }

    // ---------- connection ----------

    async fn connect(&mut self) {
        self.shutdown();
        self.generation += 1;
        if !matches!(self.status, SourceStatus::Ready(_)) {
            self.status = SourceStatus::Connecting(Empty {});
            self.publish();
        }
        match self.try_connect().await {
            Ok(()) => {
                self.backoff = Duration::from_secs(2);
                self.status = SourceStatus::Ready(Empty {});
                tracing::info!(name = %self.name, "Pyxis connected");
            }
            Err(failure) => self.unavailable(failure),
        }
        self.publish();
    }

    fn unavailable(&mut self, failure: Failure) {
        tracing::warn!(code = %failure.code, message = %failure.message, "Pyxis unavailable");
        self.shutdown();
        let wait = if failure.code == "contract.mismatch" || failure.code == "auth.notDevice" {
            RETRY_MISMATCH_AFTER
        } else {
            let wait = self.backoff;
            self.backoff = (self.backoff * 2).min(Duration::from_secs(60));
            wait
        };
        self.connect_at = Some(Instant::now() + wait);
        self.status = SourceStatus::Unavailable(failure);
    }

    async fn try_connect(&mut self) -> Result<(), Failure> {
        let token = crate::config::read_secret(&self.config.token_file)
            .map_err(|e| Failure::new("config", e.to_string()))?;
        let client = Client::new(&self.config.url, token)?;
        let contract = client.contract_id().await?;
        if contract != wire::CONTRACT_ID {
            return Err(client::mismatch(&format!("it reports {contract}")));
        }
        // Attach the realtime socket first: a device is reachable only while it holds one,
        // so its own sessions list as reachable only after this.
        let generation = self.generation;
        let (tx, mut rx) = mpsc::channel(64);
        let socket = realtime::spawn(client.realtime_url(), client.token().into(), generation, tx);
        let first = tokio::time::timeout(CONNECT_TIMEOUT, rx.recv()).await;
        match first {
            Ok(Some((_, Heard::Connected { .. }))) => {}
            Ok(Some((_, Heard::Refused(failure) | Heard::Disconnected(failure)))) => {
                socket.abort();
                return Err(failure);
            }
            _ => {
                socket.abort();
                return Err(Failure::new(
                    "pyxis.realtime",
                    "the Pyxis realtime socket did not open",
                ));
            }
        }
        // Welcomed: forward the rest of this socket's reports to the actor loop.
        let forward = self.heard_tx.clone();
        tokio::spawn(async move {
            while let Some(heard) = rx.recv().await {
                if forward.send(heard).await.is_err() {
                    return;
                }
            }
        });
        self.socket = Some(socket);
        self.client = Some(client);
        self.refresh_sessions().await?;
        self.find_host().await?;
        self.refresh_library().await?;
        self.refresh_targets().await;
        Ok(())
    }

    fn raw(&self) -> Result<&Client, Failure> {
        self.client
            .as_ref()
            .ok_or_else(|| Failure::new("connecting", "Pyxis is still connecting"))
    }

    async fn refresh_sessions(&mut self) -> Result<(), Failure> {
        let sessions: Vec<Session> = self
            .raw()?
            .ready(
                &Request::SessionList {
                    include_unreachable: false,
                },
                "ready",
                "listing sessions",
            )
            .await?;
        let keep: Vec<String> = [self.host.clone(), self.console.clone()]
            .into_iter()
            .flatten()
            .collect();
        self.sessions.retain(|id, _| keep.contains(id));
        for session in sessions {
            self.upsert(session);
        }
        for id in keep {
            if let Ok(session) = self.session_state(&id).await {
                self.upsert(session);
            }
        }
        if let Some(console) = &self.console
            && !self.sessions.contains_key(console)
        {
            self.console = None;
        }
        Ok(())
    }

    async fn session_state(&self, id: &str) -> Result<Session, Failure> {
        self.raw()?
            .ready(
                &Request::SessionState {
                    session_id: id.into(),
                },
                "ready",
                "reading a session",
            )
            .await
    }

    fn upsert(&mut self, session: Session) {
        match self.sessions.get(&session.id) {
            Some(known) if known.revision > session.revision => {}
            _ => {
                self.sessions.insert(session.id.clone(), session);
            }
        }
    }

    /// Find the session this device hosts, or create one named for it.
    ///
    /// The core has no call that names a token's device. Reachable sessions with this
    /// name are probed with a host-only no-op (`volume.set` to the same volume); the core
    /// answers `notHost` for another device's session.
    async fn find_host(&mut self) -> Result<(), Failure> {
        if let Some(id) = &self.host
            && self.sessions.contains_key(id)
        {
            return Ok(());
        }
        let mut candidates: Vec<Session> = self
            .sessions
            .values()
            .filter(|s| s.reachable && s.output.is_none() && s.name == self.name)
            .cloned()
            .collect();
        candidates.sort_by(|a, b| a.id.cmp(&b.id));
        for candidate in candidates {
            let outcome = self
                .raw()?
                .call(&Request::CommandRun {
                    session_id: candidate.id.clone(),
                    command_id: None,
                    command: Command::VolumeSet {
                        volume: candidate.volume,
                    },
                })
                .await?;
            match outcome.status.as_str() {
                "applied" => {
                    let session: Session = outcome.value()?;
                    self.device = Some(session.host_device_id.clone());
                    self.host = Some(session.id.clone());
                    self.upsert(session);
                    return Ok(());
                }
                "notDevice" => return Err(not_device()),
                _ => continue,
            }
        }
        let outcome = self
            .raw()?
            .call(&Request::SessionCreate {
                name: self.name.clone(),
            })
            .await?;
        match outcome.status.as_str() {
            "ready" => {
                let session: Session = outcome.value()?;
                self.device = Some(session.host_device_id.clone());
                self.host = Some(session.id.clone());
                self.upsert(session);
                Ok(())
            }
            "notDevice" => Err(not_device()),
            _ => Err(outcome.failure("creating this computer's session")),
        }
    }

    async fn refresh_library(&mut self) -> Result<(), Failure> {
        let groups: Vec<LibraryGroup> = self
            .raw()?
            .ready(&Request::Groups {}, "ready", "listing library groups")
            .await?;
        self.groups = library::groups(&groups);
        let albums: Vec<LibraryAlbum> = self
            .raw()?
            .ready(&Request::Albums {}, "ready", "listing albums")
            .await?;
        self.catalog.replace_albums(albums);
        self.library_dirty = false;
        Ok(())
    }

    async fn refresh_groups(&mut self) {
        let Ok(client) = self.client() else { return };
        match client
            .ready::<Vec<LibraryGroup>>(&Request::Groups {}, "ready", "listing library groups")
            .await
        {
            Ok(groups) => {
                self.groups = library::groups(&groups);
                self.library_dirty = false;
            }
            Err(failure) => tracing::warn!(message = %failure.message, "Pyxis groups refresh"),
        }
    }

    /// Speaker outputs from every live output plugin. A failing plugin lists nothing.
    async fn refresh_targets(&mut self) {
        self.targets_at = Some(Instant::now());
        let Some(client) = self.client.clone() else {
            return;
        };
        let Ok(plugins) = client
            .ready::<Vec<Plugin>>(&Request::PluginList {}, "ready", "listing plugins")
            .await
        else {
            return;
        };
        let mut targets = vec![];
        for plugin in plugins
            .iter()
            .filter(|p| p.capabilities.iter().any(|c| c == "output") && p.status == "live")
        {
            match client
                .ready::<OutputTopology>(
                    &Request::OutputTargets {
                        plugin_id: plugin.id.clone(),
                    },
                    "ready",
                    "listing speakers",
                )
                .await
            {
                Ok(topology) => targets.extend(
                    topology
                        .groups
                        .into_iter()
                        .map(|group| (topology.plugin_id.clone(), group)),
                ),
                Err(failure) => {
                    tracing::warn!(plugin = %plugin.id, message = %failure.message, "Pyxis outputs")
                }
            }
        }
        self.targets = targets;
    }

    // ---------- published state ----------

    fn shown(&self) -> Option<&Session> {
        let id = self.console.as_ref().or(self.host.as_ref())?;
        self.sessions.get(id)
    }

    fn host_session(&self) -> Option<&Session> {
        self.sessions.get(self.host.as_ref()?)
    }

    fn outputs(&self) -> Vec<Output> {
        let mut outputs = vec![Output {
            id: "local".into(),
            name: self.name.clone(),
            local: true,
            active: self.console.is_none(),
        }];
        let mut sessions: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| {
                Some(&s.id) != self.host.as_ref()
                    && (s.reachable || Some(&s.id) == self.console.as_ref())
            })
            .collect();
        sessions.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        for session in &sessions {
            outputs.push(Output {
                id: format!("session:{}", session.id),
                name: session.name.clone(),
                local: false,
                active: self.console.as_ref() == Some(&session.id),
            });
        }
        for (plugin, group) in &self.targets {
            let taken = self.sessions.values().any(|s| {
                s.output.as_ref().is_some_and(|o| {
                    &o.plugin_id == plugin
                        && (o.target_id == group.coordinator_id
                            || group.rooms.iter().any(|room| room.id == o.target_id))
                })
            });
            if !taken {
                outputs.push(Output {
                    id: format!("target:{plugin}/{}", group.coordinator_id),
                    name: group.coordinator_name.clone(),
                    local: false,
                    active: false,
                });
            }
        }
        outputs
    }

    fn engine_ours(&self) -> Option<crate::engine::EngineState> {
        let token = match &self.render {
            Render::Loaded { token, .. } | Render::Loading { token, .. } => *token,
            Render::Idle => return None,
        };
        let state = self.engine.state.borrow().clone();
        (state.owner == Some(ME) && state.token == Some(token)).then_some(state)
    }

    fn session_view(&self) -> Option<SourceSession> {
        let session = self.shown()?;
        let console = self.console.is_some();
        let ordinals = session.ordinals();
        let cursor = session.cursor();
        let current = cursor.map(|c| ordinals[c]);
        let retrying = self
            .retry
            .as_ref()
            .filter(|_| !console)
            .map(|retry| retry.ordinal);
        let queue = session
            .queue
            .iter()
            .zip(&ordinals)
            .enumerate()
            .map(|(index, (track, ordinal))| QueueItem {
                occurrence: occurrence(session, *ordinal),
                item: self.catalog.track_item(track),
                album_key: self.catalog.album_key(track),
                status: if !console && self.failed.contains(ordinal) {
                    QueueStatus::Failed
                } else if Some(*ordinal) == retrying {
                    QueueStatus::Retrying
                } else {
                    match cursor {
                        Some(c) if index < c => QueueStatus::Played,
                        Some(c) if index == c => QueueStatus::Current,
                        _ => QueueStatus::Pending,
                    }
                },
            })
            .collect();
        let outputs = self.outputs();
        let output = outputs
            .iter()
            .find(|o| o.active)
            .cloned()
            .unwrap_or_else(|| outputs[0].clone());
        let engine = (!console).then(|| self.engine_ours()).flatten();
        let track_id = session.current_track_id.clone();
        let mut track = track_id
            .as_deref()
            .map(|id| {
                let info = self.catalog.track(id).cloned().unwrap_or_default();
                Track {
                    title: info.title,
                    artist: info.artist.unwrap_or_default(),
                    id: Some(id.into()),
                    album: info.album,
                    duration_ms: session.duration_ms.map(u64::from).or(info.duration_ms),
                    artwork_url: info.artwork_url,
                    ..Track::default()
                }
            })
            .unwrap_or_default();
        if let Some(measured) = engine.as_ref().and_then(|e| e.track.clone()) {
            track.bitrate_kbps = measured.bitrate_kbps;
            track.sample_rate_hz = measured.sample_rate_hz;
            track.bit_depth = measured.bit_depth;
            track.channels = measured.channels;
            track.duration_ms = track.duration_ms.or(measured.duration_ms);
        }
        let len = session.queue.len();
        let wraps = session.repeat == RepeatMode::All && len > 0;
        let transport = match &engine {
            Some(state) if matches!(self.render, Render::Loaded { .. }) => state.transport,
            _ => transport_from(session.transport),
        };
        let position_ms = match &engine {
            Some(state) if matches!(self.render, Render::Loaded { .. }) => state.position_ms,
            _ => Some(u64::from(session.position_ms)),
        };
        let own = OwnPlayback {
            source: ME,
            output,
            capabilities: Capabilities {
                play: len > 0,
                pause: transport == Transport::Playing,
                stop: current.is_some(),
                previous: cursor.is_some_and(|c| c > 0) || wraps,
                next: cursor.is_some_and(|c| c + 1 < len) || wraps,
                seek: current.is_some() && track.duration_ms.is_some_and(|d| d > 0),
                volume: true,
                repeat: true,
                shuffle: true,
            },
            track,
            transport,
            occurrence: current.map(|o| occurrence(session, o)),
            position_ms,
            volume: Some(f64::from(session.volume.min(100)) / 100.0),
            repeat: Some(repeat_from(session.repeat)),
            shuffle: Some(session.shuffle),
            speed: None,
            skip: None,
            retry: self.retry.as_ref().filter(|_| !console).map(|retry| Retry {
                attempt: retry.attempt,
                next_at_ms: retry.at_wall_ms,
            }),
            yielded: None,
            note: None,
        };
        Some(SourceSession {
            own,
            queue,
            outputs,
            conflicts: vec![],
        })
    }

    fn publish(&self) {
        let state = SourceState {
            summary: SourceSummary {
                source: ME,
                label: ME.label().into(),
                status: self.status.clone(),
                groups: self.groups.clone(),
            },
            session: self.session_view(),
            outputs: self.outputs(),
            activation: self.activation,
        };
        self.state.send_if_modified(|current| {
            let changed = *current != state;
            if changed {
                *current = state;
            }
            changed
        });
    }

    // ---------- requests ----------

    async fn request(&mut self, request: SourceRequest) {
        let SourceRequest { id, action, reply } = request;
        let result = self.handle(&id, action).await;
        if let Err(failure) = &result {
            tracing::debug!(code = %failure.code, message = %failure.message, "Pyxis request");
        }
        let _ = reply.send(result);
        self.publish();
    }

    async fn handle(&mut self, id: &str, action: SourceAction) -> Result<Option<Page>, Failure> {
        let internal = id.starts_with("internal:");
        let user_action = !matches!(
            action,
            SourceAction::Open { .. }
                | SourceAction::Search { .. }
                | SourceAction::SavePlaylist { .. }
                | SourceAction::AddToPlaylist { .. }
        );
        if user_action {
            self.retry = None;
        }
        if internal
            && matches!(
                action,
                SourceAction::Transport {
                    action: OwnTransport::Pause
                }
            )
        {
            // Leaving Pyxis for another source: pause here, keep the place and the session.
            self.play_when_active = false;
            if self.client().is_err() {
                if self.engine_allowed() && self.engine_ours().is_some() {
                    let _ = self.engine_send(EngineCommand::Pause).await;
                }
                return Ok(None);
            }
            return self.pause(None).await.map(|()| None);
        }
        self.client()?;
        match action {
            SourceAction::Open { node, cursor, .. } => self.open(id, node, cursor).await.map(Some),
            SourceAction::Search { query } => self.search(id, &query).await.map(Some),
            SourceAction::Play { item, mode, .. } => {
                self.play_item(&item, mode).await.map(|()| None)
            }
            SourceAction::StartStation { seed, .. } => {
                self.start_station(&seed).await.map(|()| None)
            }
            SourceAction::Jump { occurrence } => {
                let index = self.index(&occurrence)?;
                self.dispatch(Command::CursorJump { index }, None).await?;
                self.dispatch(Command::Play {}, None).await.map(|()| None)
            }
            SourceAction::Remove { occurrence } => {
                let index = self.index(&occurrence)?;
                self.dispatch(Command::QueueRemove { index }, None)
                    .await
                    .map(|()| None)
            }
            SourceAction::SelectOutput { output, .. } => {
                self.select_output(&output).await.map(|()| None)
            }
            SourceAction::SavePlaylist { name, .. } => {
                self.save_playlist(&name).await.map(|()| None)
            }
            SourceAction::AddToPlaylist { playlist, item, .. } => {
                self.add_to_playlist(&playlist, &item).await.map(|()| None)
            }
            SourceAction::Transport { action } => self.transport(action).await.map(|()| None),
            SourceAction::Seek {
                position_ms,
                occurrence,
            } => {
                let session = self.shown().ok_or_else(no_session)?;
                let current = session.cursor().map(|c| session.ordinals()[c]);
                if current.map(|o| self::occurrence(session, o)).as_deref() != Some(&occurrence) {
                    return Err(stale_occurrence());
                }
                let position_ms = u32::try_from(position_ms)
                    .map_err(|_| Failure::new("invalid", "seek position out of range"))?;
                self.dispatch(Command::Seek { position_ms }, None)
                    .await
                    .map(|()| None)
            }
            SourceAction::Volume { value } => {
                let volume = (value.clamp(0.0, 1.0) * 100.0).round() as u8;
                self.dispatch(Command::VolumeSet { volume }, None)
                    .await
                    .map(|()| None)
            }
            SourceAction::Repeat { mode } => self
                .dispatch(
                    Command::RepeatSet {
                        mode: repeat_to(mode),
                    },
                    None,
                )
                .await
                .map(|()| None),
            SourceAction::Shuffle { enabled } => self
                .dispatch(Command::ShuffleSet { enabled }, None)
                .await
                .map(|()| None),
            SourceAction::Speed { .. } => Err(unsupported("playback speed")),
            SourceAction::MarkPlayed { .. } => Err(unsupported("played flags")),
            SourceAction::ResolveConflict { .. } => Err(unsupported("position conflicts")),
            SourceAction::SetEq(_) | SourceAction::Balance { .. } => Err(Failure::new(
                "misrouted",
                "EQ and balance belong to the engine",
            )),
        }
    }

    fn index(&self, occurrence: &str) -> Result<u32, Failure> {
        let session = self.shown().ok_or_else(no_session)?;
        index_of(session, occurrence)
            .map(|i| i as u32)
            .ok_or_else(stale_occurrence)
    }

    async fn transport(&mut self, action: OwnTransport) -> Result<(), Failure> {
        let session = self.shown().ok_or_else(no_session)?.clone();
        let len = session.queue.len();
        let wraps = session.repeat == RepeatMode::All && len > 0;
        match action {
            OwnTransport::Play => self.dispatch(Command::Play {}, None).await,
            OwnTransport::Pause => self.pause(None).await,
            OwnTransport::PlayPause => {
                // Decide from what sounds: a yield pauses the engine before the core hears.
                if self.shown_transport(&session) == Transport::Playing {
                    self.pause(None).await
                } else {
                    self.dispatch(Command::Play {}, None).await
                }
            }
            OwnTransport::Stop => self.dispatch(Command::Stop {}, None).await,
            OwnTransport::Next | OwnTransport::Previous => {
                let cursor = session.cursor();
                let target = match (action, cursor) {
                    (OwnTransport::Next, Some(c)) if c + 1 < len => c + 1,
                    (OwnTransport::Next, _) if wraps => 0,
                    (OwnTransport::Previous, Some(c)) if c > 0 => c - 1,
                    (OwnTransport::Previous, Some(_)) if wraps => len - 1,
                    (OwnTransport::Previous, Some(c)) => c,
                    _ => return Err(Failure::new("endOfQueue", "nothing to skip to")),
                };
                let playing = session.transport == SessionTransport::Playing;
                self.dispatch(
                    Command::CursorJump {
                        index: target as u32,
                    },
                    None,
                )
                .await?;
                if playing {
                    self.dispatch(Command::Play {}, None).await?;
                }
                Ok(())
            }
        }
    }

    /// Pause only when playing; a paused or stopped session is already where it should be.
    async fn pause(&mut self, command_id: Option<String>) -> Result<(), Failure> {
        let Some(session) = self.shown() else {
            return Ok(());
        };
        if session.transport != SessionTransport::Playing {
            if self.console.is_none() && self.engine_allowed() && self.engine_ours().is_some() {
                let _ = self.engine_send(EngineCommand::Pause).await;
            }
            return Ok(());
        }
        self.dispatch(Command::Pause {}, command_id).await
    }

    /// Send a session command: the engine first in host mode, the remote host in console.
    async fn dispatch(
        &mut self,
        command: Command,
        command_id: Option<String>,
    ) -> Result<(), Failure> {
        if let Some(console) = self.console.clone() {
            return self.send_console(&console, command).await;
        }
        self.apply_host(command, command_id).await
    }

    async fn send_console(&mut self, session_id: &str, command: Command) -> Result<(), Failure> {
        let outcome = self
            .client()?
            .call(&Request::CommandSend {
                session_id: session_id.into(),
                command_id: Some(uuid::Uuid::new_v4().to_string()),
                command: command.clone(),
            })
            .await
            .map_err(unsupported_insert(&command))?;
        let name = self
            .sessions
            .get(session_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "that output".into());
        match outcome.status.as_str() {
            "dispatched" => Ok(()),
            "unreachable" => Err(Failure::new(
                "unreachable",
                format!("{name} is not reachable"),
            )),
            "busy" => Err(Failure::new(
                "busy",
                format!("{name} is not taking commands"),
            )),
            "unknownSession" => Err(Failure::new("unknownSession", format!("{name} is gone"))),
            "hostOnly" => Err(Failure::new("hostOnly", "only the host can report that")),
            _ => Err(outcome.failure("the console command")),
        }
    }

    /// `session.command.run` on this device's session.
    async fn run_host(
        &mut self,
        command: Command,
        command_id: Option<String>,
    ) -> Result<Session, Failure> {
        let session_id = self.host.clone().ok_or_else(no_session)?;
        let outcome = self
            .client()?
            .call(&Request::CommandRun {
                session_id,
                command_id,
                command: command.clone(),
            })
            .await
            .map_err(unsupported_insert(&command))?;
        match outcome.status.as_str() {
            "applied" => {
                let session: Session = outcome.value()?;
                self.upsert(session.clone());
                Ok(session)
            }
            "notHost" | "unknownSession" => {
                self.host = None;
                Err(Failure::new(
                    "pyxis.session",
                    "this computer's Pyxis session moved or was removed",
                ))
            }
            "notDevice" => Err(not_device()),
            _ => {
                let failure = outcome.failure("the host command");
                if matches!(command, Command::Seek { .. })
                    && failure.code == "session.invalidTransportCommand"
                {
                    return Err(Failure::new(
                        "seekRefused",
                        format!("Pyxis refused the seek: {}", failure.message),
                    ));
                }
                Err(failure)
            }
        }
    }

    // ---------- host mode ----------

    fn engine_allowed(&self) -> bool {
        *self.active.borrow() == Some(ME)
    }

    async fn engine_send(&self, command: EngineCommand) -> Result<(), Failure> {
        self.engine.send(Some(ME), command).await
    }

    async fn stop_engine(&mut self) {
        if self.engine_allowed() && self.render != Render::Idle {
            let _ = self.engine_send(EngineCommand::Stop).await;
        }
        self.render = Render::Idle;
    }

    /// Apply one host command: renderer effect first, then the core's record of it.
    async fn apply_host(
        &mut self,
        command: Command,
        command_id: Option<String>,
    ) -> Result<(), Failure> {
        match command {
            Command::Play {} => self.host_play(command_id).await,
            Command::Pause {} => {
                if self.engine_allowed() && self.engine_ours().is_some() {
                    self.engine_send(EngineCommand::Pause).await?;
                    self.report_position().await;
                }
                if matches!(self.render, Render::Loading { .. }) {
                    self.stop_engine().await;
                }
                self.run_host(Command::Pause {}, command_id).await.map(drop)
            }
            Command::Stop {} => {
                self.stop_engine().await;
                self.run_host(Command::Stop {}, command_id).await.map(drop)
            }
            Command::Seek { position_ms } => self.host_seek(position_ms, command_id).await,
            Command::CursorJump { .. } | Command::QueueClear {} => {
                self.stop_engine().await;
                if matches!(command, Command::QueueClear {}) {
                    self.failed.clear();
                }
                self.run_host(command, command_id).await.map(drop)
            }
            Command::QueueRemove { index } => {
                let current = self.host_session().and_then(|s| s.cursor()) == Some(index as usize);
                if current {
                    self.stop_engine().await;
                }
                self.run_host(command, command_id).await.map(drop)
            }
            Command::VolumeSet { volume } => {
                if self.engine_allowed() {
                    self.engine_send(EngineCommand::Volume {
                        value: f64::from(volume.min(100)) / 100.0,
                    })
                    .await?;
                }
                self.run_host(command, command_id).await.map(drop)
            }
            Command::QueueAdd { .. }
            | Command::RepeatSet { .. }
            | Command::ShuffleSet { .. }
            | Command::PositionReport { .. }
            | Command::TrackEnded {} => self.run_host(command, command_id).await.map(drop),
        }
    }

    async fn host_play(&mut self, command_id: Option<String>) -> Result<(), Failure> {
        let session = self.host_session().ok_or_else(no_session)?.clone();
        if session.queue.is_empty() {
            return Err(Failure::new("emptyQueue", "the Pyxis queue is empty"));
        }
        if !self.engine_allowed() {
            // The session activates Pyxis after accepting the request; play then.
            self.play_when_active = true;
            return Ok(());
        }
        if let (Some(state), Render::Loaded { track, .. }) = (self.engine_ours(), &self.render)
            && Some(track) == session.current_track_id.as_ref()
        {
            if state.transport != Transport::Playing {
                self.engine_send(EngineCommand::Play).await?;
            }
            if session.transport != SessionTransport::Playing {
                self.run_host(Command::Play {}, command_id).await?;
            }
            return Ok(());
        }
        self.load_current(Some(command_id)).await
    }

    /// Load the session's current track. `report` reports `transport.play` once started.
    async fn load_current(&mut self, report: Option<Option<String>>) -> Result<(), Failure> {
        let session = self.host_session().ok_or_else(no_session)?.clone();
        let (Some(cursor), Some(track)) = (session.cursor(), session.current_track_id.clone())
        else {
            self.render = Render::Idle;
            return Ok(());
        };
        let ordinal = session.ordinals()[cursor];
        let client = self.client()?.clone();
        self.next_token += 1;
        let token = self.next_token;
        let info = self.catalog.track(&track).cloned().unwrap_or_default();
        let media = Media {
            url: client.stream_url(&track),
            headers: vec![client.auth_header()],
            track: Track {
                title: info.title,
                artist: info.artist.unwrap_or_default(),
                id: Some(track.clone()),
                album: info.album,
                duration_ms: session.duration_ms.map(u64::from).or(info.duration_ms),
                artwork_url: info.artwork_url,
                ..Track::default()
            },
        };
        let start_ms = if session.transport == SessionTransport::Ended {
            0
        } else {
            u64::from(session.position_ms)
        };
        self.render = Render::Loading {
            token,
            ordinal,
            track,
            report,
        };
        if let Err(failure) = self
            .engine_send(EngineCommand::Volume {
                value: f64::from(session.volume.min(100)) / 100.0,
            })
            .await
            .and(
                self.engine_send(EngineCommand::Load {
                    token,
                    media,
                    start_ms,
                    paused: false,
                })
                .await,
            )
        {
            self.render = Render::Idle;
            if failure.code == "inactive" {
                self.play_when_active = true;
                return Ok(());
            }
            return Err(failure);
        }
        Ok(())
    }

    async fn host_seek(
        &mut self,
        position_ms: u32,
        command_id: Option<String>,
    ) -> Result<(), Failure> {
        let loaded = matches!(self.render, Render::Loaded { .. });
        let Some(state) = self
            .engine_ours()
            .filter(|_| loaded && self.engine_allowed())
        else {
            return Err(Failure::new(
                "notLoaded",
                "the track is not loaded here; play it before seeking",
            ));
        };
        let before = state.position_ms.unwrap_or_default();
        let target = u64::from(position_ms);
        self.engine_send(EngineCommand::Seek {
            position_ms: target,
        })
        .await?;
        // Renderer confirmation: wait until the engine reports the new position.
        let mut watch = self.engine.state.clone();
        let confirmed = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let near = watch
                    .borrow_and_update()
                    .position_ms
                    .is_some_and(|p| p.abs_diff(target) <= 1500);
                if near || watch.changed().await.is_err() {
                    return near;
                }
            }
        })
        .await
        .unwrap_or(false);
        if !confirmed {
            let _ = self
                .engine_send(EngineCommand::Seek {
                    position_ms: before,
                })
                .await;
            return Err(Failure::new(
                "seekRefused",
                "the engine did not confirm the seek",
            ));
        }
        if let Err(failure) = self
            .run_host(Command::Seek { position_ms }, command_id)
            .await
        {
            // The core did not record it: put the renderer back where the session says.
            let _ = self
                .engine_send(EngineCommand::Seek {
                    position_ms: before,
                })
                .await;
            let unsaved = self
                .host_session()
                .and_then(|s| s.current_track_id.as_deref())
                .is_some_and(|track| !self.catalog.is_library_track(track));
            if failure.code == "seekRefused" && unsaved {
                return Err(Failure::new(
                    "seekRefused",
                    "this Pyxis server only seeks tracks saved in the library",
                ));
            }
            return Err(failure);
        }
        Ok(())
    }

    async fn report_position(&mut self) {
        let Some(state) = self.engine_ours() else {
            return;
        };
        if !matches!(self.render, Render::Loaded { .. }) {
            return;
        }
        let Some(position) = state.position_ms.and_then(|p| u32::try_from(p).ok()) else {
            return;
        };
        let duration_ms = state
            .track
            .as_ref()
            .and_then(|t| t.duration_ms)
            .and_then(|d| u32::try_from(d).ok())
            .filter(|d| *d > 0);
        self.reported_at = Instant::now();
        if let Err(failure) = self
            .run_host(
                Command::PositionReport {
                    position_ms: position,
                    duration_ms,
                },
                None,
            )
            .await
        {
            tracing::debug!(message = %failure.message, "Pyxis position report");
        }
    }

    async fn engine_event(&mut self, event: EngineEvent) {
        if event.owner != ME {
            return;
        }
        match (&self.render, &event.kind) {
            (Render::Loading { token, .. }, EngineEventKind::Started) if *token == event.token => {
                let Render::Loading {
                    token,
                    ordinal,
                    track,
                    report,
                } = std::mem::replace(&mut self.render, Render::Idle)
                else {
                    unreachable!()
                };
                self.render = Render::Loaded {
                    token,
                    ordinal,
                    track,
                };
                self.retry = None;
                self.failed.remove(&ordinal);
                let playing = self
                    .host_session()
                    .is_some_and(|s| s.transport == SessionTransport::Playing);
                if let Some(command_id) = report
                    && !playing
                    && let Err(failure) = self.run_host(Command::Play {}, command_id).await
                {
                    // The core did not record Playing: do not keep sounding.
                    tracing::warn!(message = %failure.message, "Pyxis refused play");
                    let _ = self.engine_send(EngineCommand::Pause).await;
                }
                self.report_position().await;
            }
            (Render::Loaded { token, .. }, EngineEventKind::Ended) if *token == event.token => {
                self.track_ended().await;
            }
            (
                Render::Loaded { token, ordinal, .. } | Render::Loading { token, ordinal, .. },
                EngineEventKind::Failed(failure),
            ) if *token == event.token => {
                tracing::warn!(message = %failure.message, "Pyxis track failed");
                let ordinal = *ordinal;
                let report = match &self.render {
                    Render::Loading { report, .. } => report.clone(),
                    _ => None,
                };
                self.render = Render::Idle;
                self.failed_load(ordinal, report).await;
            }
            _ => {}
        }
        self.publish();
    }

    async fn track_ended(&mut self) {
        let Some(session) = self.host_session().cloned() else {
            return;
        };
        self.render = Render::Idle;
        if let Some(track) = &session.current_track_id {
            self.append_listen(&session, track).await;
        }
        let last = session
            .cursor()
            .is_some_and(|c| c + 1 == session.queue.len());
        if last && session.repeat != RepeatMode::One {
            self.top_up_station().await;
        }
        match self.run_host(Command::TrackEnded {}, None).await {
            Ok(after) if after.transport == SessionTransport::Playing => {
                if let Err(failure) = self.load_current(None).await {
                    tracing::warn!(message = %failure.message, "Pyxis next track");
                }
            }
            Ok(_) => {}
            Err(failure) => tracing::warn!(message = %failure.message, "Pyxis track end"),
        }
    }

    async fn top_up_station(&mut self) {
        let Some(station) = self.station.as_ref().filter(|s| !s.exhausted) else {
            return;
        };
        let request = Request::StationNext {
            plugin_id: station.plugin.clone(),
            station_id: station.station.clone(),
            cursor: station.cursor.clone(),
            limit: None,
        };
        let Ok(client) = self.client() else { return };
        match client
            .ready::<StationBatch>(&request, "ready", "the next station batch")
            .await
        {
            Ok(batch) => {
                if let Some(station) = self.station.as_mut() {
                    station.cursor = batch.cursor.clone();
                    station.exhausted = batch.exhausted;
                }
                for track in &batch.tracks {
                    self.catalog.put_search_track(track);
                }
                let ids: Vec<String> = batch.tracks.into_iter().map(|t| t.id).collect();
                if !ids.is_empty()
                    && let Err(failure) = self
                        .run_host(
                            Command::QueueAdd {
                                track_ids: ids,
                                index: None,
                            },
                            None,
                        )
                        .await
                {
                    tracing::warn!(message = %failure.message, "Pyxis station top-up");
                }
            }
            Err(failure) => tracing::warn!(message = %failure.message, "Pyxis station top-up"),
        }
    }

    async fn append_listen(&mut self, session: &Session, track: &str) {
        let Some(device) = self.device.clone() else {
            return;
        };
        let played_ms = self
            .engine_ours()
            .and_then(|s| s.position_ms)
            .and_then(|p| u32::try_from(p).ok())
            .filter(|p| *p > 0);
        let event = wire::ListenEvent {
            id: ulid(),
            track_id: track.into(),
            album_id: self.catalog.track(track).and_then(|t| t.album_id.clone()),
            device_id: device,
            listened_at: rfc3339_now(),
            played_ms,
            completed: true,
            context: "queue".into(),
            context_id: Some(session.id.clone()),
        };
        let Ok(client) = self.client() else { return };
        if let Err(failure) = client
            .ready::<serde_json::Value>(
                &Request::ListenAppend {
                    events: vec![event],
                },
                "ready",
                "the listen",
            )
            .await
        {
            tracing::debug!(message = %failure.message, "Pyxis listen");
        }
    }

    /// Retry 2 s and 10 s after an engine failure, then mark the item failed and move on.
    async fn failed_load(&mut self, ordinal: u32, report: Option<Option<String>>) {
        let attempt = match &self.retry {
            Some(retry) if retry.ordinal == ordinal => retry.attempt + 1,
            _ => 1,
        };
        if let Some(delay) = retry_delay(attempt) {
            self.retry = Some(PendingRetry {
                ordinal,
                attempt,
                at: Instant::now() + delay,
                at_wall_ms: now_ms() + delay.as_millis() as u64,
                report,
            });
            return;
        }
        self.retry = None;
        self.failed.insert(ordinal);
        let Some(session) = self.host_session().cloned() else {
            return;
        };
        let ordinals = session.ordinals();
        let len = ordinals.len();
        let next = session.cursor().and_then(|c| {
            if c + 1 < len {
                Some(c + 1)
            } else if session.repeat == RepeatMode::All && len > 1 {
                Some(0)
            } else {
                None
            }
        });
        match next.filter(|n| !self.failed.contains(&ordinals[*n])) {
            Some(next) => {
                let moved = self
                    .run_host(Command::CursorJump { index: next as u32 }, None)
                    .await;
                if moved.is_ok()
                    && let Err(failure) = self.load_current(Some(None)).await
                {
                    tracing::warn!(message = %failure.message, "Pyxis skip after failure");
                }
            }
            None => {
                if session.transport == SessionTransport::Playing {
                    let _ = self.run_host(Command::Stop {}, None).await;
                }
            }
        }
    }

    async fn retry_now(&mut self) {
        let Some(retry) = self.retry.as_ref() else {
            return;
        };
        let report = retry.report.clone();
        let current = self
            .host_session()
            .and_then(|s| s.cursor().map(|c| s.ordinals()[c]));
        if current != Some(retry.ordinal) || self.console.is_some() || !self.engine_allowed() {
            self.retry = None;
        } else {
            // Keep `retry` so the next failure counts as the following attempt.
            if let Some(retry) = self.retry.as_mut() {
                retry.at = Instant::now() + Duration::from_secs(3600);
            }
            if let Err(failure) = self.load_current(report).await {
                tracing::warn!(message = %failure.message, "Pyxis retry");
            }
        }
        self.publish();
    }

    /// The transport the face shows: the engine's for a loaded host track, else the core's.
    fn shown_transport(&self, session: &Session) -> Transport {
        match (&self.render, self.engine_ours()) {
            (Render::Loaded { .. }, Some(state)) if self.console.is_none() => state.transport,
            _ => transport_from(session.transport),
        }
    }

    /// The engine paused or resumed a Pyxis load without Pyxis asking (the session's yield):
    /// report that renderer-confirmed state so the core and other clients match the audio.
    async fn engine_changed(&mut self) {
        let Render::Loaded { token, .. } = self.render else {
            return;
        };
        let Some(state) = self.engine_ours() else {
            return;
        };
        let Some(core) = self.host_session().map(|s| s.transport) else {
            return;
        };
        let command = match (state.transport, core) {
            (Transport::Paused, SessionTransport::Playing) => Command::Pause {},
            (Transport::Playing, SessionTransport::Paused) => Command::Play {},
            _ => {
                self.synced = None;
                return;
            }
        };
        // Once per load and transport: a refused report must not repeat on every position tick.
        if self.synced == Some((token, state.transport)) || self.client().is_err() {
            return;
        }
        self.synced = Some((token, state.transport));
        if matches!(command, Command::Pause {}) {
            self.report_position().await;
        }
        if let Err(failure) = self.run_host(command, None).await {
            tracing::warn!(message = %failure.message, "Pyxis engine transport report");
        }
        self.publish();
    }

    async fn activated(&mut self, now: Option<SourceId>) {
        if now == Some(ME) {
            if std::mem::take(&mut self.play_when_active)
                && self.console.is_none()
                && let Err(failure) = self.host_play(None).await
            {
                tracing::warn!(message = %failure.message, "Pyxis play on activation");
            }
        } else {
            // Another source owns the engine now; reload at the session's place on return.
            self.play_when_active = false;
            self.retry = None;
            self.render = Render::Idle;
        }
        self.publish();
    }

    async fn tick(&mut self) {
        if self.client().is_ok() {
            let playing = self
                .engine_ours()
                .is_some_and(|s| s.transport == Transport::Playing);
            if playing && self.reported_at.elapsed() >= POSITION_REPORT_EVERY {
                self.report_position().await;
            }
            if self.library_dirty {
                self.refresh_groups().await;
            }
            if self
                .targets_at
                .is_none_or(|at| at.elapsed() >= TARGETS_REFRESH_EVERY)
            {
                self.refresh_targets().await;
            }
        }
        self.publish();
    }

    // ---------- realtime ----------

    async fn heard(&mut self, heard: Heard) {
        match heard {
            Heard::Connected { complete } => {
                if !complete && self.client.is_some() {
                    if let Err(failure) = self.refresh_sessions().await {
                        tracing::warn!(message = %failure.message, "Pyxis resync");
                    }
                    self.library_dirty = true;
                }
            }
            Heard::Session(session) => self.upsert(*session),
            Heard::Album(album) => {
                self.catalog.put_album(*album);
                self.library_dirty = true;
            }
            Heard::AlbumRemoved(id) => {
                self.catalog.remove_album(&id);
                self.library_dirty = true;
            }
            Heard::Directive(directive) => self.directive(*directive).await,
            Heard::Disconnected(failure) => {
                tracing::info!(message = %failure.message, "Pyxis realtime reconnecting");
            }
            Heard::Refused(failure) => self.unavailable(failure),
        }
        self.publish();
    }

    /// A console's command for this host. The directive id is the command id, so the
    /// core's receipt makes a redelivered directive effect-free.
    async fn directive(&mut self, directive: Directive) {
        if Some(&directive.session_id) != self.host.as_ref() {
            return;
        }
        if self.directives.contains(&directive.directive_id) {
            return;
        }
        self.directives.push_back(directive.directive_id.clone());
        if self.directives.len() > 64 {
            self.directives.pop_front();
        }
        if matches!(directive.command, Command::Play {}) && !self.engine_allowed() {
            // A console asked this host to play: ask the session to make Pyxis active.
            // host_play then waits for activation (play_when_active).
            tracing::info!(by = %directive.issued_by, "Pyxis play directive: asking to become the active source");
            self.activation += 1;
        }
        self.retry = None;
        let id = Some(directive.directive_id);
        if let Err(failure) = self.apply_host(directive.command, id).await {
            tracing::warn!(message = %failure.message, "Pyxis directive");
        }
    }

    // ---------- library ----------

    async fn open(
        &mut self,
        id: &str,
        node: Option<String>,
        cursor: Option<String>,
    ) -> Result<Page, Failure> {
        let page = |items: Vec<Item>, next_cursor: Option<String>, total: Option<u64>| Page {
            id: id.into(),
            source: Some(ME),
            node: node.clone(),
            query: None,
            items,
            next_cursor,
            total,
        };
        let Some(text) = node.as_deref() else {
            let items: Vec<Item> = self.groups.iter().map(library::group_item).collect();
            let total = items.len() as u64;
            return Ok(page(items, None, Some(total)));
        };
        let parsed = Node::parse(text).ok_or_else(unknown_node)?;
        let tracks: Vec<String> = match parsed {
            Node::Group(group) => {
                let result: GroupPage = {
                    let outcome = self
                        .client()?
                        .call(&Request::GroupItems {
                            group_id: group,
                            cursor,
                            limit: PAGE as u32,
                        })
                        .await?;
                    match outcome.status.as_str() {
                        "ready" => outcome.value()?,
                        "unknownGroup" => {
                            self.library_dirty = true;
                            return Err(Failure::new("unknownNode", "that group is gone"));
                        }
                        _ => return Err(outcome.failure("the group page")),
                    }
                };
                let items = result
                    .items
                    .into_iter()
                    .map(|member| self.catalog.member_item(member))
                    .collect();
                return Ok(page(items, result.cursor, None));
            }
            Node::Album(album) => {
                if self.catalog.album(&album).is_none() {
                    self.refresh_library().await?;
                }
                let album = self.catalog.album(&album).ok_or_else(unknown_node)?;
                album.tracks.iter().map(|t| t.id.clone()).collect()
            }
            Node::Playlist(playlist) => self.playlist(&playlist).await?.track_ids,
            Node::SourceAlbum { plugin, id } => self.source_album(&plugin, &id).await?,
            Node::Track(_) | Node::Station { .. } | Node::Artist { .. } => {
                return Err(Failure::new("notOpenable", "that item has no children"));
            }
        };
        let offset: usize = match cursor {
            Some(cursor) => cursor
                .strip_prefix("offset:")
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| Failure::new("invalidCursor", "that cursor is not valid here"))?,
            None => 0,
        };
        let total = tracks.len();
        let end = (offset + PAGE).min(total);
        let items = tracks
            .get(offset.min(total)..end)
            .unwrap_or_default()
            .iter()
            .map(|t| self.catalog.track_item(t))
            .collect();
        let next = (end < total).then(|| format!("offset:{end}"));
        Ok(page(items, next, Some(total as u64)))
    }

    async fn playlist(&self, id: &str) -> Result<Playlist, Failure> {
        let playlists: Vec<Playlist> = self
            .client()?
            .ready(&Request::Playlists {}, "ready", "listing playlists")
            .await?;
        playlists
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(unknown_node)
    }

    async fn source_album(&mut self, plugin: &str, id: &str) -> Result<Vec<String>, Failure> {
        let album: SourceAlbum = self
            .client()?
            .ready(
                &Request::SourceAlbum {
                    plugin_id: plugin.into(),
                    external_id: id.into(),
                },
                "ready",
                "the album",
            )
            .await?;
        for track in &album.tracks {
            self.catalog.put_search_track(track);
        }
        Ok(album.tracks.into_iter().map(|t| t.id).collect())
    }

    async fn search(&mut self, id: &str, query: &str) -> Result<Page, Failure> {
        let mut items: Vec<Item> = self
            .catalog
            .search_albums(query)
            .into_iter()
            .map(library::album_item)
            .collect();
        let outcome = self
            .client()?
            .call(&Request::Search {
                query: query.into(),
                limit: 20,
            })
            .await?;
        match outcome.status.as_str() {
            "ready" => {
                let result: SearchResult = outcome.value()?;
                for track in &result.tracks {
                    self.catalog.put_search_track(track);
                    items.push(self.catalog.track_item(&track.id));
                }
                items.extend(result.albums.into_iter().map(|album| {
                    Item {
                        source: ME,
                        id: Node::SourceAlbum {
                            plugin: album.source_plugin_id,
                            id: album.external_id,
                        }
                        .encode(),
                        kind: ItemKind::Album,
                        title: album.title,
                        subtitle: Some(album.artist),
                        album: None,
                        year: album.year,
                        duration_ms: None,
                        count: None,
                        openable: true,
                        playable: true,
                        progress: None,
                    }
                }));
                items.extend(result.artists.into_iter().map(|artist| {
                    Item {
                        source: ME,
                        id: Node::Artist {
                            plugin: artist.source_plugin_id,
                            id: artist.external_id,
                        }
                        .encode(),
                        kind: ItemKind::Artist,
                        title: artist.name,
                        subtitle: None,
                        album: None,
                        year: None,
                        duration_ms: None,
                        count: None,
                        openable: false,
                        playable: false,
                        progress: None,
                    }
                }));
            }
            "noSources" => {}
            _ => return Err(outcome.failure("the search")),
        }
        Ok(Page {
            id: id.into(),
            source: Some(ME),
            node: None,
            query: Some(query.into()),
            total: Some(items.len() as u64),
            items,
            next_cursor: None,
        })
    }

    /// The core track ids an item stands for.
    async fn tracks_of(&mut self, item: &str) -> Result<Vec<String>, Failure> {
        let ids = match Node::parse(item).ok_or_else(unknown_node)? {
            Node::Track(id) => vec![id],
            Node::Album(album) => {
                if self.catalog.album(&album).is_none() {
                    self.refresh_library().await?;
                }
                let album = self.catalog.album(&album).ok_or_else(unknown_node)?;
                album.tracks.iter().map(|t| t.id.clone()).collect()
            }
            Node::Playlist(id) => self.playlist(&id).await?.track_ids,
            Node::SourceAlbum { plugin, id } => self.source_album(&plugin, &id).await?,
            Node::Group(_) | Node::Artist { .. } | Node::Station { .. } => {
                return Err(Failure::new("notPlayable", "that item cannot be queued"));
            }
        };
        if ids.is_empty() {
            return Err(Failure::new("empty", "that item has no tracks"));
        }
        Ok(ids)
    }

    async fn play_item(&mut self, item: &str, mode: PlayMode) -> Result<(), Failure> {
        if let Some(Node::Station { .. }) = Node::parse(item) {
            if mode != PlayMode::Now {
                return Err(Failure::new("notQueueable", "a station replaces the queue"));
            }
            return self.start_station(item).await;
        }
        let ids = self.tracks_of(item).await?;
        self.queue_tracks(ids, mode).await?;
        if mode == PlayMode::Now {
            self.station = None;
        }
        Ok(())
    }

    async fn queue_tracks(&mut self, ids: Vec<String>, mode: PlayMode) -> Result<(), Failure> {
        match mode {
            PlayMode::Now => {
                self.dispatch(Command::QueueClear {}, None).await?;
                self.dispatch(
                    Command::QueueAdd {
                        track_ids: ids,
                        index: None,
                    },
                    None,
                )
                .await?;
                self.dispatch(Command::Play {}, None).await
            }
            PlayMode::Next => {
                let index = self.shown().and_then(|s| s.cursor()).map(|c| c as u32 + 1);
                self.dispatch(
                    Command::QueueAdd {
                        track_ids: ids,
                        index,
                    },
                    None,
                )
                .await
            }
            PlayMode::Last => {
                self.dispatch(
                    Command::QueueAdd {
                        track_ids: ids,
                        index: None,
                    },
                    None,
                )
                .await
            }
        }
    }

    async fn start_station(&mut self, seed_text: &str) -> Result<(), Failure> {
        let (plugin, station) = match library::seed(&self.catalog, seed_text)
            .map_err(|message| Failure::new("invalidSeed", message))?
        {
            Seed::Existing { plugin, station } => (plugin, station),
            Seed::New {
                plugin,
                kind,
                external,
            } => {
                let outcome = self
                    .client()?
                    .call(&Request::StationCreate {
                        plugin_id: plugin.clone(),
                        seed: StationSeed {
                            kind,
                            external_id: external,
                        },
                    })
                    .await?;
                match outcome.status.as_str() {
                    "ready" => (plugin, outcome.value::<Station>()?.external_id),
                    "unsupportedSeed" => {
                        return Err(Failure::new(
                            "unsupportedSeed",
                            "this source cannot start a station from that",
                        ));
                    }
                    _ => return Err(outcome.failure("the station")),
                }
            }
        };
        let outcome = self
            .client()?
            .call(&Request::StationNext {
                plugin_id: plugin.clone(),
                station_id: station.clone(),
                cursor: None,
                limit: None,
            })
            .await?;
        let batch: StationBatch = match outcome.status.as_str() {
            "ready" => outcome.value()?,
            "unknownStation" => return Err(Failure::new("unknownStation", "that station is gone")),
            _ => return Err(outcome.failure("the station")),
        };
        for track in &batch.tracks {
            self.catalog.put_search_track(track);
        }
        let ids: Vec<String> = batch.tracks.iter().map(|t| t.id.clone()).collect();
        if ids.is_empty() {
            return Err(Failure::new("empty", "the station has nothing to play"));
        }
        self.queue_tracks(ids, PlayMode::Now).await?;
        self.station = Some(StationRun {
            plugin,
            station,
            cursor: batch.cursor,
            exhausted: batch.exhausted,
        });
        Ok(())
    }

    async fn save_playlist(&mut self, name: &str) -> Result<(), Failure> {
        let session = self.shown().ok_or_else(no_session)?;
        if session.queue.is_empty() {
            return Err(Failure::new("emptyQueue", "the queue is empty"));
        }
        let request = Request::PlaylistCreate {
            title: name.into(),
            track_ids: session.queue.clone(),
        };
        self.client()?
            .ready::<Playlist>(&request, "ready", "saving the playlist")
            .await?;
        self.refresh_groups().await;
        Ok(())
    }

    async fn add_to_playlist(&mut self, playlist: &str, item: &str) -> Result<(), Failure> {
        let Some(Node::Playlist(playlist)) = Node::parse(playlist) else {
            return Err(Failure::new("invalid", "that is not a Pyxis playlist"));
        };
        let track_ids = self.tracks_of(item).await?;
        let outcome = self
            .client()?
            .call(&Request::PlaylistAdd {
                playlist_id: playlist,
                track_ids,
                command_id: uuid::Uuid::new_v4().to_string(),
            })
            .await?;
        match outcome.status.as_str() {
            "applied" => {
                self.refresh_groups().await;
                Ok(())
            }
            "unknownPlaylist" => Err(Failure::new("unknownPlaylist", "that playlist is gone")),
            _ => Err(outcome.failure("adding to the playlist")),
        }
    }

    async fn select_output(&mut self, output: &str) -> Result<(), Failure> {
        if output == "local" {
            self.console = None;
            return Ok(());
        }
        let session_id = if let Some(id) = output.strip_prefix("session:") {
            if !self.sessions.contains_key(id) || Some(id) == self.host.as_deref() {
                return Err(Failure::new("unknownOutput", "that output is gone"));
            }
            id.to_owned()
        } else if let Some(target) = output.strip_prefix("target:") {
            let (plugin, target) = target
                .split_once('/')
                .ok_or_else(|| Failure::new("unknownOutput", "that output is not valid"))?;
            let outcome = self
                .client()?
                .call(&Request::OutputSessionCreate {
                    plugin_id: plugin.into(),
                    target_id: target.into(),
                    name: self
                        .targets
                        .iter()
                        .find(|(p, g)| p == plugin && g.coordinator_id == target)
                        .map(|(_, g)| g.coordinator_name.clone())
                        .unwrap_or_else(|| target.into()),
                })
                .await?;
            let session: Session = match outcome.status.as_str() {
                "ready" => outcome.value()?,
                "unknownTarget" => {
                    return Err(Failure::new("unknownOutput", "that speaker is gone"));
                }
                _ => return Err(outcome.failure("the speaker session")),
            };
            let id = session.id.clone();
            self.upsert(session);
            id
        } else {
            return Err(Failure::new("unknownOutput", "that output is not valid"));
        };
        // Leaving local playback: pause here and keep the place.
        if self.console.is_none() {
            self.retry = None;
            self.pause(None).await?;
        }
        self.console = Some(session_id);
        Ok(())
    }
}

fn no_session() -> Failure {
    Failure::new("noSession", "there is no Pyxis session yet")
}

fn unknown_node() -> Failure {
    Failure::new("unknownNode", "that Pyxis item is not known")
}

fn not_device() -> Failure {
    Failure::new(
        "auth.notDevice",
        "the Pyxis token is not a device token, so Pyxamp cannot host playback; \
         give it a device token from auth.device.claim",
    )
}

/// A core without insert support rejects `queue.add` with `index` at decode time.
fn unsupported_insert(command: &Command) -> impl Fn(Failure) -> Failure + '_ {
    move |failure| match command {
        Command::QueueAdd { index: Some(_), .. } if failure.code == "request.invalidPayload" => {
            Failure::new(
                "unsupported",
                "this Pyxis server cannot insert after the current track; \
                 play next needs a newer Pyxis",
            )
        }
        _ => failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listen_ids_are_ulids() {
        let id = ulid();
        assert_eq!(id.len(), 26);
        assert!(id.as_bytes()[0] <= b'7', "a ULID fits 128 bits");
        assert_ne!(id, ulid());
    }

    #[test]
    fn listen_timestamps_are_rfc3339_utc() {
        let text = rfc3339_now();
        assert_eq!(text.len(), 20);
        assert!(text.ends_with('Z') && text.as_bytes()[10] == b'T');
    }

    #[test]
    fn occurrences_are_fenced_to_their_session() {
        let session: Session = serde_json::from_value(serde_json::json!({
            "repeat":"off","shuffle":true,"queueOrder":[2,0,1],"id":"s1","name":"Desk",
            "hostDeviceId":"d","queue":["a","b","c"],"cursor":0,"transport":"stopped",
            "positionMs":0,"volume":50,"reachable":true,"revision":1,"updatedAt":"x"
        }))
        .unwrap();
        assert_eq!(index_of(&session, "s1:0"), Some(1));
        assert_eq!(index_of(&session, "s2:0"), None);
        assert_eq!(index_of(&session, "s1:9"), None);
    }
}

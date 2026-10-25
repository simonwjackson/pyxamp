//! Ossicle source: podcasts and narrated books (spoken word).
//!
//! Ossicle (`services/ossicle` in the Ossicle repo) owns every piece of state
//! here: the up-next queue, resume positions with revision conflicts, played
//! flags, speed and skip settings with per-show overrides, and sponsor
//! segments. This actor reads them over `POST /rpc`, plays through the engine
//! while `ctx.active` names Ossicle, and writes progress back periodically
//! and on pause, stop, seek and end. Books are the comics server's EPUB
//! narration, reached through Ossicle's `/comics` bridge; each narration
//! window (up to 8 sentences) is one engine load, and the next is preloaded.
use super::{SourceContext, SourceSession, SourceState};
use crate::{
    config::OssicleConfig,
    engine::{EngineCommand, EngineEvent, EngineEventKind, EngineState, Media},
    protocol::{
        Capabilities, Conflict, ConflictPosition, Empty, Failure, Group, GroupKind, Item, ItemKind,
        Output, OwnPlayback, OwnTransport, Page, PlayMode, QueueItem, QueueStatus, Retry,
        SkipIntervals, SourceAction, SourceId, SourceStatus, SourceSummary, Track, Transport,
    },
};
use logic::{ItemId, Span};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast::error::RecvError, mpsc};
use wire::{
    Anchor, Book, BookLocator, BookPosition, BookPositionWrite, Client, Episode, NarrationBatch,
    Position, PositionWriteOutcome, QueueEntry, SettingsSnapshot, Timeline,
};

pub mod logic;
pub mod wire;

const ME: SourceId = SourceId::Ossicle;
const TICK: Duration = Duration::from_millis(250);
const COMMIT_EVERY: Duration = Duration::from_secs(15);
const REFRESH_EVERY: Duration = Duration::from_secs(30);
const RETRY_CONNECT_EVERY: Duration = Duration::from_secs(5);
const BOOKS_EVERY: Duration = Duration::from_secs(300);
const NOTE_FOR: Duration = Duration::from_secs(2);
const PAGE_SIZE: usize = 100;
const SEARCH_LIMIT: usize = 50;
const SEARCH_EPISODES: u32 = 1000;
/// Live synthesis can lag; Ossicle's player polls this often, this many times.
const PREPARE_POLL: Duration = Duration::from_millis(1500);
const PREPARE_ATTEMPTS: u32 = 40;

pub const NOTE_SPONSOR: &str = "SKIPPED SPONSOR";
pub const NOTE_CONFLICT: &str = "POSITION CONFLICT";

pub async fn run(ctx: SourceContext<OssicleConfig>) -> anyhow::Result<()> {
    let SourceContext {
        config,
        engine,
        mut active,
        state,
        mut requests,
        ..
    } = ctx;
    let mut events = engine.events.subscribe();
    let mut engine_state = engine.state.clone();
    let (internal_tx, mut internal) = mpsc::channel(8);
    let mut actor = Actor {
        client: Client::new(&config),
        device: device_id(),
        engine,
        active: active.clone(),
        state,
        internal: internal_tx,
        status: SourceStatus::Connecting(Empty {}),
        snapshot: None,
        podcasts: vec![],
        queue: vec![],
        books: None,
        books_at: None,
        refreshed_at: None,
        current: None,
        conflict: None,
        retry: None,
        note: None,
        failed: HashSet::new(),
        tokens: 0,
        runs: 0,
    };
    actor.refresh().await;
    actor.publish();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut events_open = true;
    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { break };
                let result = actor.handle(&request.id, request.action).await;
                let _ = request.reply.send(result);
            }
            Ok(()) = active.changed() => actor.on_active().await,
            event = events.recv(), if events_open => match event {
                Ok(event) => actor.on_event(event).await,
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => events_open = false,
            },
            Ok(()) = engine_state.changed() => {
                let state = engine_state.borrow_and_update().clone();
                actor.on_engine(&state).await;
            }
            Some(message) = internal.recv() => actor.on_internal(message).await,
            _ = tick.tick() => actor.on_tick().await,
        }
        actor.publish();
    }
    Ok(())
}

/// The host name: Ossicle records it as this device in conflict provenance.
fn device_id() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "pyxamp".into())
}

fn wall_ms(after: Duration) -> u64 {
    (SystemTime::now() + after)
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn unsupported(message: &str) -> Failure {
    Failure::new("unsupported", message)
}

fn stale(message: &str) -> Failure {
    Failure::new("stale", message)
}

enum Internal {
    /// A narration batch for book run `run`; `first` starts the book, else it is the next window.
    Batch {
        run: u64,
        first: bool,
        result: Result<NarrationBatch, Failure>,
    },
}

enum Kind {
    Episode(Box<EpisodeState>),
    Book(Box<BookState>),
}

struct EpisodeState {
    episode: Episode,
    revision: Option<u64>,
    committed_s: Option<u32>,
    spans: Vec<Span>,
    suppressed: Vec<u64>,
}

enum NextWindow {
    None,
    Preparing,
    Ready(NarrationBatch),
    Preloaded { token: u64, batch: NarrationBatch },
    End,
}

struct BookState {
    book: Book,
    timeline: Timeline,
    run: u64,
    batch: Option<NarrationBatch>,
    index: usize,
    next: NextWindow,
    /// The window ended before the next one was ready.
    waiting: bool,
    revision: Option<u64>,
    committed: Option<BookLocator>,
}

struct Current {
    item: Item,
    occurrence: String,
    album_key: String,
    media: Media,
    kind: Kind,
    /// The engine load this item owns, while it owns one.
    token: Option<u64>,
    /// Load into the engine when Ossicle is active (play request, activation or retry).
    pending_load: bool,
    autoplay: bool,
    position_ms: u64,
    transport: Transport,
    volume: Option<f64>,
    measured: Option<Track>,
    speed: f64,
    skip: SkipIntervals,
    status: QueueStatus,
    last_commit: Instant,
}

enum PendingConflict {
    Episode {
        id: String,
        theirs: Position,
        yours_s: u32,
    },
    Book {
        id: String,
        theirs: BookPosition,
        yours: BookLocator,
    },
}

struct RetryState {
    attempt: u32,
    at: Instant,
    next_at_ms: u64,
}

struct Actor {
    client: Result<Client, Failure>,
    device: String,
    engine: crate::engine::EngineHandle,
    active: tokio::sync::watch::Receiver<Option<SourceId>>,
    state: tokio::sync::watch::Sender<SourceState>,
    internal: mpsc::Sender<Internal>,
    status: SourceStatus,
    snapshot: Option<SettingsSnapshot>,
    podcasts: Vec<wire::Podcast>,
    queue: Vec<QueueEntry>,
    /// `None` when Books are off in Ossicle; `Some(Err)` when the books server failed.
    books: Option<Result<Vec<Book>, Failure>>,
    books_at: Option<Instant>,
    refreshed_at: Option<Instant>,
    current: Option<Current>,
    conflict: Option<PendingConflict>,
    retry: Option<RetryState>,
    note: Option<(String, Instant)>,
    failed: HashSet<String>,
    tokens: u64,
    runs: u64,
}

impl Actor {
    fn client(&self) -> Result<Client, Failure> {
        self.client.clone()
    }

    fn is_active(&self) -> bool {
        *self.active.borrow() == Some(ME)
    }

    async fn engine(&self, command: EngineCommand) -> Result<(), Failure> {
        if !self.is_active() {
            return Err(Failure::new("inactive", "Ossicle is not the active source"));
        }
        self.engine.send(Some(ME), command).await
    }

    fn loaded(&self) -> bool {
        self.is_active() && self.current.as_ref().is_some_and(|c| c.token.is_some())
    }

    // ---------- library state ----------

    async fn refresh(&mut self) {
        self.refreshed_at = Some(Instant::now());
        let client = match self.client() {
            Ok(client) => client,
            Err(failure) => {
                self.status = SourceStatus::Unavailable(failure);
                return;
            }
        };
        let result = async {
            let status = client.status().await?;
            if status.contract_id != wire::CONTRACT_ID {
                return Err(Failure::new(
                    "contract",
                    format!(
                        "Ossicle speaks {}, Pyxamp needs {}",
                        status.contract_id,
                        wire::CONTRACT_ID
                    ),
                ));
            }
            let podcasts = client.podcasts().await?;
            let queue = client.queue().await?;
            let snapshot = client.settings().await?;
            Ok((podcasts, queue, snapshot))
        }
        .await;
        match result {
            Ok((podcasts, queue, snapshot)) => {
                self.podcasts = podcasts;
                self.queue = queue;
                let books_on = !snapshot.settings.books_upstream.is_empty();
                self.snapshot = Some(snapshot);
                self.status = SourceStatus::Ready(Empty {});
                if !books_on {
                    self.books = None;
                } else if self.books.is_none()
                    || self.books_at.is_none_or(|at| at.elapsed() >= BOOKS_EVERY)
                {
                    self.books_at = Some(Instant::now());
                    self.books = Some(client.books().await);
                }
            }
            Err(failure) => {
                tracing::warn!(code = %failure.code, message = %failure.message, "Ossicle unavailable");
                self.status = SourceStatus::Unavailable(failure);
            }
        }
    }

    async fn refresh_queue(&mut self) -> Result<(), Failure> {
        self.queue = self.client()?.queue().await?;
        Ok(())
    }

    fn groups(&self) -> Vec<Group> {
        if !matches!(self.status, SourceStatus::Ready(_)) {
            return vec![];
        }
        let mut groups = vec![
            Group {
                id: logic::GROUP_UP_NEXT.into(),
                label: "Up next".into(),
                kind: GroupKind::UpNext,
                count: Some(self.queue.len() as u64),
            },
            Group {
                id: logic::GROUP_SHOWS.into(),
                label: "Shows".into(),
                kind: GroupKind::Shows,
                count: Some(self.podcasts.len() as u64),
            },
        ];
        if let Some(books) = &self.books {
            groups.push(Group {
                id: logic::GROUP_BOOKS.into(),
                label: "Books".into(),
                kind: GroupKind::Books,
                count: books.as_ref().ok().map(|b| b.len() as u64),
            });
        }
        groups
    }

    // ---------- publishing ----------

    fn session(&self) -> Option<SourceSession> {
        let current = self.current.as_ref()?;
        let mut track = current.media.track.clone();
        if let Some(measured) = &current.measured {
            track.bitrate_kbps = measured.bitrate_kbps;
            track.sample_rate_hz = measured.sample_rate_hz;
            track.bit_depth = measured.bit_depth;
            track.channels = measured.channels;
            if track.duration_ms.is_none() {
                track.duration_ms = measured.duration_ms;
            }
        }
        let note = if self.conflict.is_some() {
            Some(NOTE_CONFLICT.to_owned())
        } else {
            self.note
                .as_ref()
                .filter(|(_, until)| Instant::now() < *until)
                .map(|(text, _)| text.clone())
        };
        let own = OwnPlayback {
            source: ME,
            output: Output {
                id: "local".into(),
                name: "This computer".into(),
                local: true,
                active: true,
            },
            capabilities: Capabilities {
                play: true,
                pause: true,
                stop: true,
                previous: true,
                next: true,
                seek: track.duration_ms.is_some(),
                volume: true,
                repeat: false,
                shuffle: false,
            },
            track,
            transport: current.transport,
            occurrence: Some(current.occurrence.clone()),
            position_ms: Some(current.position_ms),
            volume: current.volume,
            repeat: None,
            shuffle: None,
            speed: Some(current.speed),
            skip: Some(current.skip.clone()),
            retry: self.retry.as_ref().map(|r| Retry {
                attempt: r.attempt,
                next_at_ms: r.next_at_ms,
            }),
            yielded: None,
            note,
        };
        let current_episode = match &current.kind {
            Kind::Episode(e) => Some(e.episode.id.as_str()),
            Kind::Book(_) => None,
        };
        let mut queue = vec![QueueItem {
            occurrence: current.occurrence.clone(),
            item: current.item.clone(),
            album_key: Some(current.album_key.clone()),
            status: current.status,
        }];
        for entry in &self.queue {
            if Some(entry.episode.id.as_str()) == current_episode {
                continue;
            }
            let item = logic::episode_item(&entry.episode, Some(&entry.podcast_title));
            let status = if self.failed.contains(&item.id) {
                QueueStatus::Failed
            } else {
                QueueStatus::Pending
            };
            queue.push(QueueItem {
                occurrence: logic::next_occurrence(&entry.episode.id),
                album_key: Some(logic::show_key(&entry.episode.podcast_id)),
                item,
                status,
            });
        }
        let conflicts = self.conflict_rows(current);
        Some(SourceSession {
            own,
            queue,
            outputs: vec![],
            conflicts,
        })
    }

    fn conflict_rows(&self, current: &Current) -> Vec<Conflict> {
        let Some(conflict) = &self.conflict else {
            return vec![];
        };
        let (id, theirs_device, theirs_ms, yours_ms) = match (conflict, &current.kind) {
            (
                PendingConflict::Episode {
                    id,
                    theirs,
                    yours_s,
                },
                _,
            ) => (
                id,
                theirs.device_id.clone(),
                u64::from(theirs.seconds) * 1000,
                u64::from(*yours_s) * 1000,
            ),
            (PendingConflict::Book { id, theirs, yours }, Kind::Book(book)) => (
                id,
                theirs.device_id.clone(),
                logic::estimated_ms(&book.timeline, &theirs.locator),
                logic::estimated_ms(&book.timeline, yours),
            ),
            _ => return vec![],
        };
        vec![Conflict {
            id: id.clone(),
            item: current.item.clone(),
            positions: vec![
                ConflictPosition {
                    id: "theirs".into(),
                    device: theirs_device,
                    position_ms: theirs_ms,
                    updated_at_ms: None,
                },
                ConflictPosition {
                    id: "yours".into(),
                    device: self.device.clone(),
                    position_ms: yours_ms,
                    updated_at_ms: None,
                },
            ],
        }]
    }

    fn publish(&self) {
        let next = SourceState {
            summary: SourceSummary {
                source: ME,
                label: ME.label().into(),
                status: self.status.clone(),
                groups: self.groups(),
            },
            session: self.session(),
            outputs: vec![],
            activation: 0,
        };
        self.state.send_if_modified(|state| {
            let changed = *state != next;
            if changed {
                *state = next;
            }
            changed
        });
    }

    // ---------- requests ----------

    async fn handle(&mut self, id: &str, action: SourceAction) -> Result<Option<Page>, Failure> {
        match action {
            SourceAction::Open { node, cursor, .. } => {
                self.open(id, node, cursor.as_deref()).await.map(Some)
            }
            SourceAction::Search { query } => self.search(id, &query).await.map(Some),
            SourceAction::Play { item, mode, .. } => {
                self.cancel_retry();
                self.play(&item, mode).await.map(|()| None)
            }
            SourceAction::Jump { occurrence } => {
                self.cancel_retry();
                self.jump(&occurrence).await.map(|()| None)
            }
            SourceAction::Remove { occurrence } => self.remove(&occurrence).await.map(|()| None),
            SourceAction::MarkPlayed { item, played, .. } => {
                self.mark_played(&item, played).await.map(|()| None)
            }
            SourceAction::ResolveConflict { conflict, keep, .. } => {
                self.resolve(&conflict, &keep).await.map(|()| None)
            }
            SourceAction::Transport { action } => {
                let internal = id.starts_with("internal:");
                if !internal {
                    self.cancel_retry();
                }
                let result = self.transport(action).await;
                // The session's pause before a source switch must never fail.
                if internal {
                    Ok(None)
                } else {
                    result.map(|()| None)
                }
            }
            SourceAction::Seek {
                position_ms,
                occurrence,
            } => {
                self.cancel_retry();
                self.seek(position_ms, &occurrence).await.map(|()| None)
            }
            SourceAction::Volume { value } => {
                self.engine(EngineCommand::Volume { value }).await?;
                if let Some(current) = &mut self.current {
                    current.volume = Some(value);
                }
                Ok(None)
            }
            SourceAction::Speed { rate } => self.set_speed(rate).await.map(|()| None),
            SourceAction::Repeat { .. } => Err(unsupported("Ossicle has no repeat")),
            SourceAction::Shuffle { .. } => Err(unsupported("Ossicle has no shuffle")),
            SourceAction::SelectOutput { .. } => Err(unsupported("Ossicle plays on this computer")),
            SourceAction::SavePlaylist { .. } | SourceAction::AddToPlaylist { .. } => {
                Err(unsupported("Ossicle has no playlists"))
            }
            SourceAction::StartStation { .. } => Err(unsupported("Ossicle has no stations")),
            SourceAction::SetEq(_) | SourceAction::Balance { .. } => {
                Err(unsupported("EQ and balance belong to the engine"))
            }
            SourceAction::SystemVolume { .. } => {
                Err(unsupported("the system volume belongs to the session"))
            }
        }
    }

    async fn open(
        &mut self,
        id: &str,
        node: Option<String>,
        cursor: Option<&str>,
    ) -> Result<Page, Failure> {
        let client = self.client()?;
        let items: Vec<Item> = match node.as_deref() {
            None => {
                self.refresh().await;
                if let SourceStatus::Unavailable(failure) = &self.status {
                    return Err(failure.clone());
                }
                self.groups()
                    .into_iter()
                    .map(|g| Item {
                        source: ME,
                        id: g.id,
                        kind: ItemKind::Group,
                        title: g.label,
                        subtitle: None,
                        album: None,
                        year: None,
                        duration_ms: None,
                        count: g.count,
                        openable: true,
                        playable: false,
                        progress: None,
                    })
                    .collect()
            }
            Some(logic::GROUP_UP_NEXT) => {
                self.refresh_queue().await?;
                self.queue
                    .iter()
                    .map(|e| logic::episode_item(&e.episode, Some(&e.podcast_title)))
                    .collect()
            }
            Some(logic::GROUP_SHOWS) => {
                self.podcasts = client.podcasts().await?;
                self.podcasts
                    .iter()
                    .map(|p| logic::show_item(&p.id, &p.title, p.episode_count))
                    .collect()
            }
            Some(logic::GROUP_BOOKS) => {
                let books = client.books().await;
                self.books_at = Some(Instant::now());
                self.books = Some(books.clone());
                books?.iter().map(logic::book_item).collect()
            }
            Some(other) => match ItemId::parse(other) {
                Some(ItemId::Show(podcast)) => {
                    let detail = client.podcast(&podcast).await?;
                    detail
                        .episodes
                        .iter()
                        .map(|e| logic::episode_item(e, Some(&detail.podcast.title)))
                        .collect()
                }
                _ => return Err(Failure::new("notFound", "Ossicle cannot open that")),
            },
        };
        let offset = match cursor {
            None => 0,
            Some(text) => text
                .parse::<usize>()
                .map_err(|_| Failure::new("invalid", "unknown cursor"))?,
        };
        let total = items.len();
        let end = (offset + PAGE_SIZE).min(total);
        Ok(Page {
            id: id.to_owned(),
            source: Some(ME),
            node,
            query: None,
            items: items.get(offset..end).unwrap_or_default().to_vec(),
            next_cursor: (end < total).then(|| end.to_string()),
            total: Some(total as u64),
        })
    }

    /// Ossicle has no search call: match shows, recent episodes and books here.
    async fn search(&mut self, id: &str, query: &str) -> Result<Page, Failure> {
        let client = self.client()?;
        self.podcasts = client.podcasts().await?;
        let mut items: Vec<Item> = self
            .podcasts
            .iter()
            .filter(|p| logic::matches(query, &[&p.title]))
            .map(|p| logic::show_item(&p.id, &p.title, p.episode_count))
            .collect();
        let titles: HashMap<&str, &str> = self
            .podcasts
            .iter()
            .map(|p| (p.id.as_str(), p.title.as_str()))
            .collect();
        for episode in client.recent_episodes(SEARCH_EPISODES).await? {
            let show = titles.get(episode.podcast_id.as_str()).copied();
            if logic::matches(query, &[&episode.title, show.unwrap_or_default()]) {
                items.push(logic::episode_item(&episode, show));
            }
        }
        if let Some(Ok(books)) = &self.books {
            for book in books {
                let authors = book.authors.join(" ");
                if logic::matches(query, &[&book.title, &authors]) {
                    items.push(logic::book_item(book));
                }
            }
        }
        items.truncate(SEARCH_LIMIT);
        Ok(Page {
            id: id.to_owned(),
            source: Some(ME),
            node: None,
            query: Some(query.to_owned()),
            items,
            next_cursor: None,
            total: None,
        })
    }

    async fn play(&mut self, item: &str, mode: PlayMode) -> Result<(), Failure> {
        let parsed = ItemId::parse(item).ok_or_else(|| Failure::new("notFound", "unknown item"))?;
        let mode = if self.current.is_none() {
            PlayMode::Now
        } else {
            mode
        };
        match (parsed, mode) {
            (ItemId::Show(_), _) => Err(unsupported("open a show and pick an episode")),
            (ItemId::Episode { podcast, episode }, PlayMode::Now) => {
                if self.queue.iter().any(|e| e.episode.id == episode) {
                    self.write_queue(|ids| ids.retain(|id| *id != episode))
                        .await?;
                }
                self.start_episode(&podcast, &episode, true).await
            }
            (ItemId::Episode { episode, .. }, PlayMode::Next) => {
                self.write_queue(|ids| {
                    ids.retain(|id| *id != episode);
                    ids.insert(0, episode.clone());
                })
                .await
            }
            (ItemId::Episode { episode, .. }, PlayMode::Last) => {
                self.write_queue(|ids| {
                    ids.retain(|id| *id != episode);
                    ids.push(episode.clone());
                })
                .await
            }
            (ItemId::Book(book), PlayMode::Now) => self.start_book(&book, true).await,
            (ItemId::Book(_), _) => Err(unsupported("Ossicle's up next holds episodes only")),
        }
    }

    async fn write_queue(&mut self, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), Failure> {
        let client = self.client()?;
        self.queue = client.queue().await?;
        let mut ids: Vec<String> = self.queue.iter().map(|e| e.episode.id.clone()).collect();
        edit(&mut ids);
        self.queue = client.write_queue(&ids).await?;
        Ok(())
    }

    async fn jump(&mut self, occurrence: &str) -> Result<(), Failure> {
        if self
            .current
            .as_ref()
            .is_some_and(|c| c.occurrence == occurrence)
        {
            return self.transport(OwnTransport::Play).await;
        }
        let episode =
            logic::parse_next_occurrence(occurrence).ok_or_else(|| stale("that row is gone"))?;
        let podcast = self
            .queue
            .iter()
            .find(|e| e.episode.id == episode)
            .map(|e| e.episode.podcast_id.clone())
            .ok_or_else(|| stale("that row is gone"))?;
        self.write_queue(|ids| ids.retain(|id| *id != episode))
            .await?;
        self.start_episode(&podcast, &episode, true).await
    }

    async fn remove(&mut self, occurrence: &str) -> Result<(), Failure> {
        if self
            .current
            .as_ref()
            .is_some_and(|c| c.occurrence == occurrence)
        {
            return Err(unsupported("the playing item cannot be removed"));
        }
        let episode =
            logic::parse_next_occurrence(occurrence).ok_or_else(|| stale("that row is gone"))?;
        if !self.queue.iter().any(|e| e.episode.id == episode) {
            return Err(stale("that row is gone"));
        }
        self.write_queue(|ids| ids.retain(|id| *id != episode))
            .await
    }

    async fn mark_played(&mut self, item: &str, played: bool) -> Result<(), Failure> {
        let Some(ItemId::Episode { episode, .. }) = ItemId::parse(item) else {
            return Err(unsupported("only episodes have a played flag"));
        };
        let position = self
            .client()?
            .mark_played(&episode, played, &self.device)
            .await?;
        let apply = |e: &mut Episode| {
            if e.id == position.episode_id {
                e.position = Some(position.clone());
            }
        };
        self.queue.iter_mut().for_each(|e| apply(&mut e.episode));
        if let Some(current) = &mut self.current
            && let Kind::Episode(state) = &mut current.kind
            && state.episode.id == position.episode_id
        {
            apply(&mut state.episode);
            // The played write advances the revision; later progress chains from it.
            state.revision = Some(position.revision);
            current.item.progress = logic::progress_of(&state.episode);
        }
        Ok(())
    }

    // ---------- starting items ----------

    async fn settings(&mut self) -> Result<SettingsSnapshot, Failure> {
        match self.client()?.settings().await {
            Ok(snapshot) => {
                self.snapshot = Some(snapshot.clone());
                Ok(snapshot)
            }
            Err(failure) => self.snapshot.clone().ok_or(failure),
        }
    }

    async fn leave_current(&mut self) {
        if self.current.is_some() {
            self.commit(true).await;
            if self.loaded() {
                let _ = self.engine(EngineCommand::Stop).await;
            }
        }
        self.current = None;
        self.conflict = None;
        self.retry = None;
        self.note = None;
    }

    async fn start_episode(
        &mut self,
        podcast_id: &str,
        episode_id: &str,
        autoplay: bool,
    ) -> Result<(), Failure> {
        let client = self.client()?;
        let detail = client.podcast(podcast_id).await?;
        let episode = detail
            .episodes
            .iter()
            .find(|e| e.id == episode_id)
            .cloned()
            .ok_or_else(|| Failure::new("notFound", "Ossicle no longer has that episode"))?;
        let snapshot = self.settings().await?;
        let show = snapshot
            .podcasts
            .iter()
            .find(|s| s.podcast_id == podcast_id);
        let effective = logic::effective(&snapshot.settings, show);
        let spans = if snapshot.settings.sponsor_skip && episode.video_path.is_some() {
            match client.sponsor_segments(&episode.id).await {
                Ok(segments) => logic::merge_segments(&segments),
                Err(failure) => {
                    tracing::warn!(message = %failure.message, "No sponsor segments");
                    vec![]
                }
            }
        } else {
            vec![]
        };
        self.leave_current().await;
        let show_title = detail.podcast.title.clone();
        let item = logic::episode_item(&episode, Some(&show_title));
        let position_ms = match &episode.position {
            Some(p) if !p.played => u64::from(p.seconds) * 1000,
            _ => 0,
        };
        let media = Media {
            url: client.url(&episode.audio_path),
            headers: client.headers(),
            track: Track {
                title: episode.title.clone(),
                artist: show_title.clone(),
                id: Some(item.id.clone()),
                album: Some(show_title),
                duration_ms: item.duration_ms,
                artwork_url: detail.podcast.cover_path.as_deref().map(|p| client.url(p)),
                ..Track::default()
            },
        };
        self.failed.remove(&item.id);
        self.current = Some(Current {
            occurrence: logic::now_occurrence(&item.id),
            album_key: logic::show_key(podcast_id),
            media,
            kind: Kind::Episode(Box::new(EpisodeState {
                revision: episode.position.as_ref().map(|p| p.revision),
                committed_s: episode.position.as_ref().map(|p| p.seconds),
                episode,
                spans,
                suppressed: vec![],
            })),
            item,
            token: None,
            pending_load: true,
            autoplay,
            position_ms,
            transport: Transport::Paused,
            volume: None,
            measured: None,
            speed: effective.speed,
            skip: effective.skip,
            status: QueueStatus::Current,
            last_commit: Instant::now(),
        });
        self.try_load().await;
        Ok(())
    }

    async fn start_book(&mut self, book_id: &str, autoplay: bool) -> Result<(), Failure> {
        let client = self.client()?;
        let books = client.books().await;
        self.books = Some(books.clone());
        let book = books?
            .into_iter()
            .find(|b| b.id == book_id)
            .ok_or_else(|| Failure::new("notFound", "the books server no longer has that book"))?;
        let position = client.book_position(&book).await.unwrap_or_else(|failure| {
            tracing::warn!(message = %failure.message, "Book position unread; starting at the top");
            None
        });
        let timeline = client.timeline(&book).await?;
        let anchor = anchor_for(&timeline, position.as_ref().map(|p| &p.locator))?;
        let snapshot = self.settings().await?;
        let effective = logic::effective(&snapshot.settings, None);
        self.leave_current().await;
        self.runs += 1;
        let run = self.runs;
        let item = logic::book_item(&book);
        let media = Media {
            url: String::new(),
            headers: client.headers(),
            track: Track {
                title: book.title.clone(),
                artist: book.authors.join(", "),
                id: Some(item.id.clone()),
                artwork_url: book.cover.as_deref().map(|c| client.comics_url(c)),
                ..Track::default()
            },
        };
        self.current = Some(Current {
            occurrence: logic::now_occurrence(&item.id),
            album_key: item.id.clone(),
            item,
            media,
            kind: Kind::Book(Box::new(BookState {
                revision: position.as_ref().map(|p| p.revision),
                committed: position.map(|p| p.locator),
                book: book.clone(),
                timeline,
                run,
                batch: None,
                index: 0,
                next: NextWindow::None,
                waiting: false,
            })),
            token: None,
            // Loads once the first window is synthesized.
            pending_load: false,
            autoplay,
            position_ms: 0,
            transport: Transport::Paused,
            volume: None,
            measured: None,
            speed: effective.speed,
            skip: effective.skip,
            status: QueueStatus::Current,
            last_commit: Instant::now(),
        });
        self.prepare(book, anchor, run, true);
        Ok(())
    }

    /// Narration synthesis is live: poll until the batch has a stitched window.
    fn prepare(&self, book: Book, anchor: Anchor, run: u64, first: bool) {
        let Ok(client) = self.client() else { return };
        let internal = self.internal.clone();
        tokio::spawn(async move {
            let mut result = Err(Failure::new("unavailable", "narration synthesis timed out"));
            for _ in 0..PREPARE_ATTEMPTS {
                match client.prepare(&book, &anchor).await {
                    Ok(batch) if batch.window.is_some() => {
                        result = Ok(batch);
                        break;
                    }
                    Ok(_) => tokio::time::sleep(PREPARE_POLL).await,
                    Err(failure) => {
                        result = Err(failure);
                        break;
                    }
                }
            }
            let _ = internal.send(Internal::Batch { run, first, result }).await;
        });
    }

    /// Load the current item when Ossicle is active and a load is due.
    async fn try_load(&mut self) {
        if !self.is_active() {
            return;
        }
        let Some(current) = &mut self.current else {
            return;
        };
        if !current.pending_load {
            return;
        }
        current.pending_load = false;
        self.tokens += 1;
        let token = self.tokens;
        let command = EngineCommand::Load {
            token,
            media: current.media.clone(),
            start_ms: current.position_ms,
            paused: !current.autoplay,
        };
        let speed = current.speed;
        let autoplay = current.autoplay;
        current.token = Some(token);
        current.measured = None;
        let result = self.engine(command).await;
        let result = match result {
            Ok(()) => self.engine(EngineCommand::Speed { rate: speed }).await,
            Err(failure) => Err(failure),
        };
        match result {
            Ok(()) => {
                if let Some(current) = &mut self.current {
                    current.transport = if autoplay {
                        Transport::Playing
                    } else {
                        Transport::Paused
                    };
                    current.last_commit = Instant::now();
                }
                self.preload_next().await;
            }
            Err(failure) => {
                tracing::warn!(code = %failure.code, message = %failure.message, "Ossicle load refused");
                self.on_failed(token).await;
            }
        }
    }

    // ---------- transport ----------

    async fn transport(&mut self, action: OwnTransport) -> Result<(), Failure> {
        let Some(current) = &self.current else {
            return Err(Failure::new("idle", "nothing from Ossicle is loaded"));
        };
        let playing = current.transport == Transport::Playing;
        let action = match action {
            OwnTransport::PlayPause if playing => OwnTransport::Pause,
            OwnTransport::PlayPause => OwnTransport::Play,
            other => other,
        };
        match action {
            OwnTransport::PlayPause | OwnTransport::Play => {
                if current.token.is_none() || current.pending_load {
                    let current = self.current.as_mut().expect("current");
                    if matches!(&current.kind, Kind::Book(b) if b.batch.is_none()) {
                        current.autoplay = true;
                        return Ok(());
                    }
                    current.pending_load = true;
                    current.autoplay = true;
                    if !self.is_active() {
                        return Err(Failure::new("inactive", "Ossicle is not the active source"));
                    }
                    self.try_load().await;
                    return Ok(());
                }
                self.engine(EngineCommand::Play).await?;
                if let Some(current) = &mut self.current {
                    current.transport = Transport::Playing;
                }
                Ok(())
            }
            OwnTransport::Pause => {
                if let Some(current) = &mut self.current {
                    current.autoplay = false;
                }
                if self.loaded() {
                    self.engine(EngineCommand::Pause).await?;
                }
                if let Some(current) = &mut self.current
                    && current.transport == Transport::Playing
                {
                    current.transport = Transport::Paused;
                }
                self.commit(true).await;
                Ok(())
            }
            OwnTransport::Stop => {
                if self.loaded() {
                    self.engine(EngineCommand::Stop).await?;
                }
                self.commit(true).await;
                if let Some(current) = &mut self.current {
                    current.token = None;
                    current.pending_load =
                        !matches!(&current.kind, Kind::Book(b) if b.batch.is_none());
                    current.autoplay = false;
                    current.transport = Transport::Stopped;
                }
                Ok(())
            }
            OwnTransport::Previous => {
                let back = -i64::from(current.skip.back_s);
                self.skip_by(back).await
            }
            OwnTransport::Next => {
                let forward = i64::from(current.skip.forward_s);
                self.skip_by(forward).await
            }
        }
    }

    async fn skip_by(&mut self, delta_s: i64) -> Result<(), Failure> {
        let current = self.current.as_ref().expect("current");
        let duration = current
            .media
            .track
            .duration_ms
            .or(current.measured.as_ref().and_then(|t| t.duration_ms));
        let target = logic::skip_target(current.position_ms, delta_s, duration);
        let occurrence = current.occurrence.clone();
        self.seek(target, &occurrence).await
    }

    async fn seek(&mut self, position_ms: u64, occurrence: &str) -> Result<(), Failure> {
        let current = self
            .current
            .as_mut()
            .ok_or_else(|| Failure::new("idle", "nothing from Ossicle is loaded"))?;
        if current.occurrence != occurrence {
            return Err(stale("the item changed before the seek"));
        }
        if let Kind::Episode(state) = &mut current.kind
            && let Some(span) = logic::span_at(&state.spans, position_ms)
        {
            // Seeking into a sponsor span means the listener wants to hear it.
            state.suppressed.push(span.start_ms);
        }
        let loaded = current.token.is_some();
        current.position_ms = position_ms;
        if loaded && self.is_active() {
            self.engine(EngineCommand::Seek { position_ms }).await?;
        }
        self.commit(true).await;
        Ok(())
    }

    async fn set_speed(&mut self, rate: f64) -> Result<(), Failure> {
        let rate = logic::round_speed(rate);
        let client = self.client()?;
        if self.loaded() {
            self.engine(EngineCommand::Speed { rate }).await?;
        }
        let show = match self.current.as_ref().map(|c| &c.kind) {
            Some(Kind::Episode(state)) => Some(state.episode.podcast_id.clone()),
            _ => None,
        };
        if let Some(current) = &mut self.current {
            current.speed = rate;
        }
        // Last write wins in Ossicle, so start from a fresh snapshot.
        let snapshot = client.settings().await?;
        let override_ = show.and_then(|id| {
            snapshot
                .podcasts
                .iter()
                .find(|s| s.podcast_id == id && s.playback_speed.is_some())
                .cloned()
        });
        match override_ {
            Some(mut show) => {
                show.playback_speed = Some(rate);
                client.write_podcast_settings(&show).await?;
            }
            None => {
                let mut settings = snapshot.settings.clone();
                settings.playback_speed = rate;
                client.write_settings(&settings).await?;
            }
        }
        self.snapshot = client.settings().await.ok().or(Some(snapshot));
        Ok(())
    }

    // ---------- progress and conflicts ----------

    /// Report the position to Ossicle; `now` skips the periodic throttle.
    async fn commit(&mut self, now: bool) {
        if self.conflict.is_some() {
            return;
        }
        let Ok(client) = self.client() else { return };
        let device = self.device.clone();
        let Some(current) = &mut self.current else {
            return;
        };
        if !now && current.last_commit.elapsed() < COMMIT_EVERY {
            return;
        }
        current.last_commit = Instant::now();
        let seconds = (current.position_ms / 1000).min(u64::from(u32::MAX)) as u32;
        let mut conflict = None;
        let mut pause = false;
        match &mut current.kind {
            Kind::Episode(state) => {
                if state.committed_s == Some(seconds) {
                    return;
                }
                match client
                    .write_position(&state.episode.id, seconds, state.revision, &device, false)
                    .await
                {
                    Ok(PositionWriteOutcome::Accepted(position)) => {
                        state.revision = Some(position.revision);
                        state.committed_s = Some(seconds);
                    }
                    Ok(PositionWriteOutcome::Conflict(c)) => {
                        conflict = Some(PendingConflict::Episode {
                            id: format!("episode:{}@{}", state.episode.id, c.theirs.revision),
                            theirs: c.theirs,
                            yours_s: c.yours,
                        });
                        pause = true;
                    }
                    Ok(PositionWriteOutcome::UnknownEpisode) => {
                        tracing::warn!("Ossicle no longer has the playing episode");
                    }
                    Ok(PositionWriteOutcome::Unavailable(f)) => {
                        tracing::warn!(message = %f.message, "Position not saved");
                    }
                    Err(f) => tracing::warn!(message = %f.message, "Position not saved"),
                }
            }
            Kind::Book(book) => {
                let Some(segment) = book.batch.as_ref().and_then(|b| b.segments.get(book.index))
                else {
                    return;
                };
                let locator = logic::locator_for(&book.timeline, &book.book.generation, segment);
                if book.committed.as_ref() == Some(&locator) {
                    return;
                }
                match client
                    .write_book_position(&book.book.id, &locator, book.revision, &device, false)
                    .await
                {
                    Ok(BookPositionWrite::Accepted(position)) => {
                        book.revision = Some(position.revision);
                        book.committed = Some(locator);
                    }
                    Ok(BookPositionWrite::Conflict(c)) => {
                        conflict = Some(PendingConflict::Book {
                            id: format!("book:{}@{}", book.book.id, c.theirs.revision),
                            theirs: c.theirs,
                            yours: locator,
                        });
                        pause = true;
                    }
                    Ok(BookPositionWrite::Refused) => {
                        tracing::warn!("The books server refused the reading position");
                    }
                    Err(f) => tracing::warn!(message = %f.message, "Book position not saved"),
                }
            }
        }
        if let Some(conflict) = conflict {
            self.conflict = Some(conflict);
            if pause
                && self.loaded()
                && self
                    .current
                    .as_ref()
                    .is_some_and(|c| c.transport == Transport::Playing)
            {
                let _ = self.engine(EngineCommand::Pause).await;
                if let Some(current) = &mut self.current {
                    current.transport = Transport::Paused;
                }
            }
        }
    }

    async fn resolve(&mut self, id: &str, keep: &str) -> Result<(), Failure> {
        let Some(conflict) = &self.conflict else {
            return Err(stale("that conflict is resolved"));
        };
        let matches_id = match conflict {
            PendingConflict::Episode { id: c, .. } | PendingConflict::Book { id: c, .. } => c == id,
        };
        if !matches_id {
            return Err(stale("that conflict changed"));
        }
        if keep != "theirs" && keep != "yours" {
            return Err(Failure::new(
                "invalid",
                "keep must name one of the positions",
            ));
        }
        let client = self.client()?;
        let conflict = self.conflict.take().expect("conflict");
        match conflict {
            PendingConflict::Episode {
                theirs, yours_s, ..
            } => {
                let seconds = if keep == "theirs" {
                    theirs.seconds
                } else {
                    yours_s
                };
                if keep == "yours" {
                    match client
                        .write_position(
                            &theirs.episode_id,
                            yours_s,
                            Some(theirs.revision),
                            &self.device,
                            true,
                        )
                        .await
                    {
                        Ok(PositionWriteOutcome::Accepted(position)) => {
                            self.set_episode_revision(position.revision, seconds);
                        }
                        Ok(PositionWriteOutcome::Conflict(c)) => {
                            self.conflict = Some(PendingConflict::Episode {
                                id: format!("episode:{}@{}", theirs.episode_id, c.theirs.revision),
                                theirs: c.theirs,
                                yours_s: c.yours,
                            });
                            return Err(Failure::new("conflict", "another device moved again"));
                        }
                        Ok(_) => {
                            return Err(Failure::new(
                                "unavailable",
                                "Ossicle refused the position",
                            ));
                        }
                        Err(failure) => {
                            self.conflict = Some(PendingConflict::Episode {
                                id: id.to_owned(),
                                theirs,
                                yours_s,
                            });
                            return Err(failure);
                        }
                    }
                } else {
                    self.set_episode_revision(theirs.revision, seconds);
                }
                self.move_to(u64::from(seconds) * 1000).await;
            }
            PendingConflict::Book { theirs, yours, .. } => {
                if keep == "yours" {
                    match client
                        .write_book_position(
                            &theirs.book_id,
                            &yours,
                            Some(theirs.revision),
                            &self.device,
                            true,
                        )
                        .await
                    {
                        Ok(BookPositionWrite::Accepted(position)) => {
                            if let Some(Current {
                                kind: Kind::Book(book),
                                ..
                            }) = &mut self.current
                            {
                                book.revision = Some(position.revision);
                                book.committed = Some(yours);
                            }
                        }
                        Ok(BookPositionWrite::Conflict(c)) => {
                            self.conflict = Some(PendingConflict::Book {
                                id: format!("book:{}@{}", theirs.book_id, c.theirs.revision),
                                theirs: c.theirs,
                                yours,
                            });
                            return Err(Failure::new("conflict", "another device moved again"));
                        }
                        Ok(BookPositionWrite::Refused) => {
                            return Err(Failure::new(
                                "unavailable",
                                "the books server refused the position",
                            ));
                        }
                        Err(failure) => {
                            self.conflict = Some(PendingConflict::Book {
                                id: id.to_owned(),
                                theirs,
                                yours,
                            });
                            return Err(failure);
                        }
                    }
                } else {
                    // Narrate again from the other device's sentence.
                    let Some(Current {
                        kind: Kind::Book(book),
                        ..
                    }) = &mut self.current
                    else {
                        return Ok(());
                    };
                    let anchor = anchor_for(&book.timeline, Some(&theirs.locator))?;
                    self.runs += 1;
                    book.run = self.runs;
                    book.revision = Some(theirs.revision);
                    book.committed = Some(theirs.locator.clone());
                    book.next = NextWindow::None;
                    book.waiting = false;
                    let (bk, run) = (book.book.clone(), book.run);
                    if self.loaded() {
                        let _ = self.engine(EngineCommand::Stop).await;
                    }
                    if let Some(current) = &mut self.current {
                        current.token = None;
                        current.pending_load = false;
                        current.transport = Transport::Paused;
                    }
                    self.prepare(bk, anchor, run, true);
                }
            }
        }
        Ok(())
    }

    fn set_episode_revision(&mut self, revision: u64, seconds: u32) {
        if let Some(Current {
            kind: Kind::Episode(state),
            ..
        }) = &mut self.current
        {
            state.revision = Some(revision);
            state.committed_s = Some(seconds);
        }
    }

    /// Put the playhead at `position_ms` without reporting it (it is already Ossicle's).
    async fn move_to(&mut self, position_ms: u64) {
        let Some(current) = &mut self.current else {
            return;
        };
        current.position_ms = position_ms;
        if current.token.is_some() && self.is_active() {
            let _ = self.engine(EngineCommand::Seek { position_ms }).await;
        }
    }

    // ---------- events ----------

    async fn on_active(&mut self) {
        if self.is_active() {
            self.try_load().await;
            return;
        }
        // Another source took the engine: our load is gone, our place is kept.
        self.cancel_retry();
        if let Some(current) = &mut self.current {
            if current.token.is_some() {
                current.token = None;
                current.pending_load = !matches!(&current.kind, Kind::Book(b) if b.batch.is_none());
            }
            current.autoplay = false;
            if current.transport == Transport::Playing {
                current.transport = Transport::Paused;
            }
            if let Kind::Book(book) = &mut current.kind
                && let NextWindow::Preloaded { batch, .. } =
                    std::mem::replace(&mut book.next, NextWindow::None)
            {
                book.next = NextWindow::Ready(batch);
            }
        }
        self.commit(true).await;
    }

    async fn on_engine(&mut self, state: &EngineState) {
        let Some(current) = &mut self.current else {
            return;
        };
        if state.owner != Some(ME) || current.token.is_none() || state.token != current.token {
            return;
        }
        if let Some(position) = state.position_ms {
            current.position_ms = position;
        }
        if state.transport != Transport::Unknown {
            current.transport = state.transport;
        }
        current.volume = state.volume.or(current.volume);
        current.measured = state.track.clone();
        if let Kind::Book(book) = &mut current.kind
            && let Some(window) = book.batch.as_ref().and_then(|b| b.window.as_ref())
        {
            book.index = logic::window_index_at(&window.starts_ms, current.position_ms);
        }
    }

    async fn on_event(&mut self, event: EngineEvent) {
        if event.owner != ME {
            return;
        }
        let Some(current) = &mut self.current else {
            return;
        };
        if current.token != Some(event.token) {
            return;
        }
        match event.kind {
            EngineEventKind::Started => {
                self.retry = None;
                current.status = QueueStatus::Current;
            }
            EngineEventKind::Ended => self.on_ended().await,
            EngineEventKind::Failed(failure) => {
                tracing::warn!(code = %failure.code, message = %failure.message, "Ossicle item failed");
                self.on_failed(event.token).await;
            }
        }
    }

    fn enter_window(&mut self, batch: NarrationBatch, token: Option<u64>) {
        let Ok(client) = self.client() else { return };
        let Some(current) = &mut self.current else {
            return;
        };
        let Some(window) = batch.window.as_ref() else {
            return;
        };
        current.media.url = client.comics_url(&window.audio_url);
        current.media.track.duration_ms = Some(u64::from(window.duration_ms));
        current.position_ms = 0;
        current.measured = None;
        current.token = token;
        if let Kind::Book(book) = &mut current.kind {
            book.batch = Some(batch);
            book.index = 0;
        }
    }

    /// After a window loads: prepare the next one, and preload it when ready.
    async fn preload_next(&mut self) {
        let loaded = self.loaded();
        let client = self.client();
        let Some(Current {
            kind: Kind::Book(book),
            ..
        }) = &mut self.current
        else {
            return;
        };
        match std::mem::replace(&mut book.next, NextWindow::None) {
            NextWindow::None => {
                let last = book
                    .batch
                    .as_ref()
                    .and_then(|b| b.segments.last())
                    .map(|s| s.ordinal);
                match last.and_then(|ordinal| logic::next_after(&book.timeline, ordinal)) {
                    Some(segment) => {
                        let anchor = Anchor {
                            resource: segment.resource.clone(),
                            selector: segment.selector.clone(),
                            character_offset: segment.start_offset,
                            text_context: String::new(),
                        };
                        book.next = NextWindow::Preparing;
                        let (bk, run) = (book.book.clone(), book.run);
                        self.prepare(bk, anchor, run, false);
                    }
                    None => book.next = NextWindow::End,
                }
            }
            NextWindow::Ready(batch) => {
                if !loaded {
                    book.next = NextWindow::Ready(batch);
                    return;
                }
                let Ok(client) = client else {
                    book.next = NextWindow::Ready(batch);
                    return;
                };
                let Some(window) = batch.window.as_ref() else {
                    return;
                };
                self.tokens += 1;
                let token = self.tokens;
                let mut media = self.current.as_ref().expect("current").media.clone();
                media.url = client.comics_url(&window.audio_url);
                media.track.duration_ms = Some(u64::from(window.duration_ms));
                let result = self
                    .engine(EngineCommand::Preload {
                        token,
                        media: Some(media),
                    })
                    .await;
                if let Some(Current {
                    kind: Kind::Book(book),
                    ..
                }) = &mut self.current
                {
                    book.next = match result {
                        Ok(()) => NextWindow::Preloaded { token, batch },
                        Err(_) => NextWindow::Ready(batch),
                    };
                }
            }
            other => book.next = other,
        }
    }

    async fn on_internal(&mut self, message: Internal) {
        let Internal::Batch { run, first, result } = message;
        let Some(Current {
            kind: Kind::Book(book),
            ..
        }) = &mut self.current
        else {
            return;
        };
        if book.run != run {
            return;
        }
        match (first, result) {
            (true, Ok(batch)) => {
                self.enter_window(batch, None);
                if let Some(current) = &mut self.current {
                    current.pending_load = true;
                }
                self.try_load().await;
            }
            (true, Err(failure)) => {
                tracing::warn!(message = %failure.message, "Narration failed");
                if let Some(current) = &mut self.current {
                    current.status = QueueStatus::Failed;
                    current.transport = Transport::Stopped;
                }
            }
            (false, Ok(batch)) => {
                book.next = NextWindow::Ready(batch);
                if std::mem::take(&mut book.waiting) {
                    self.start_ready_window().await;
                } else {
                    self.preload_next().await;
                }
            }
            (false, Err(failure)) => {
                tracing::warn!(message = %failure.message, "Next narration window failed");
                book.next = NextWindow::End;
                if std::mem::take(&mut book.waiting) {
                    self.finish_book();
                }
            }
        }
    }

    async fn start_ready_window(&mut self) {
        let Some(Current {
            kind: Kind::Book(book),
            ..
        }) = &mut self.current
        else {
            return;
        };
        let NextWindow::Ready(batch) = std::mem::replace(&mut book.next, NextWindow::None) else {
            return;
        };
        self.enter_window(batch, None);
        if let Some(current) = &mut self.current {
            current.pending_load = true;
            current.autoplay = true;
        }
        self.try_load().await;
    }

    fn finish_book(&mut self) {
        if let Some(current) = &mut self.current {
            current.token = None;
            current.transport = Transport::Stopped;
            current.status = QueueStatus::Played;
        }
    }

    async fn on_ended(&mut self) {
        let Some(current) = &mut self.current else {
            return;
        };
        match &mut current.kind {
            Kind::Episode(state) => {
                let end = current
                    .media
                    .track
                    .duration_ms
                    .or(current.measured.as_ref().and_then(|t| t.duration_ms))
                    .unwrap_or(current.position_ms);
                current.position_ms = end;
                current.token = None;
                current.pending_load = true;
                current.autoplay = false;
                current.transport = Transport::Stopped;
                let id = state.episode.id.clone();
                let item = current.item.id.clone();
                self.commit(true).await;
                // Reaching the end is what "played" means.
                if let Err(failure) = self.mark_played(&item, true).await {
                    tracing::warn!(message = %failure.message, "Played flag not saved");
                }
                let auto = self
                    .snapshot
                    .as_ref()
                    .is_none_or(|s| s.settings.auto_advance_queue);
                if auto {
                    self.advance(&id).await;
                }
            }
            Kind::Book(book) => {
                book.index = book
                    .batch
                    .as_ref()
                    .map_or(0, |b| b.segments.len().saturating_sub(1));
                current.token = None;
                self.commit(true).await;
                let Some(Current {
                    kind: Kind::Book(book),
                    ..
                }) = &mut self.current
                else {
                    return;
                };
                match std::mem::replace(&mut book.next, NextWindow::None) {
                    // The engine already plays the preloaded window under its token.
                    NextWindow::Preloaded { token, batch } => {
                        self.enter_window(batch, Some(token));
                        if let Some(current) = &mut self.current {
                            current.transport = Transport::Playing;
                        }
                        self.preload_next().await;
                    }
                    NextWindow::Ready(batch) => {
                        book.next = NextWindow::Ready(batch);
                        self.start_ready_window().await;
                    }
                    NextWindow::Preparing => {
                        book.next = NextWindow::Preparing;
                        book.waiting = true;
                    }
                    NextWindow::None | NextWindow::End => self.finish_book(),
                }
            }
        }
    }

    /// Play the first unplayed, not-failed up-next entry after `after`.
    async fn advance(&mut self, after: &str) {
        if let Err(failure) = self.refresh_queue().await {
            tracing::warn!(message = %failure.message, "Up next unread");
        }
        let next = self
            .queue
            .iter()
            .filter(|e| e.episode.id != after)
            .find(|e| {
                !e.episode.position.as_ref().is_some_and(|p| p.played)
                    && !self.failed.contains(
                        &ItemId::Episode {
                            podcast: e.episode.podcast_id.clone(),
                            episode: e.episode.id.clone(),
                        }
                        .encode(),
                    )
            })
            .map(|e| (e.episode.podcast_id.clone(), e.episode.id.clone()));
        let Some((podcast, episode)) = next else {
            return;
        };
        let removed = self
            .write_queue(|ids| ids.retain(|id| *id != episode))
            .await;
        if let Err(failure) = removed.and(self.start_episode(&podcast, &episode, true).await) {
            tracing::warn!(message = %failure.message, "Could not move to the next episode");
        }
    }

    async fn on_failed(&mut self, token: u64) {
        let Some(current) = &mut self.current else {
            return;
        };
        if current.token != Some(token) {
            return;
        }
        current.token = None;
        let attempt = self.retry.as_ref().map_or(1, |r| r.attempt + 1);
        match logic::retry_delay(attempt) {
            Some(delay) => {
                current.status = QueueStatus::Retrying;
                self.retry = Some(RetryState {
                    attempt,
                    at: Instant::now() + delay,
                    next_at_ms: wall_ms(delay),
                });
            }
            None => {
                self.retry = None;
                current.status = QueueStatus::Failed;
                current.transport = Transport::Stopped;
                current.pending_load = true;
                current.autoplay = false;
                let item = current.item.id.clone();
                self.failed.insert(item);
                let after = match &current.kind {
                    Kind::Episode(state) => Some(state.episode.id.clone()),
                    Kind::Book(_) => None,
                };
                if let Some(after) = after {
                    Box::pin(self.advance(&after)).await;
                }
            }
        }
    }

    fn cancel_retry(&mut self) {
        if self.retry.take().is_some()
            && let Some(current) = &mut self.current
        {
            current.status = QueueStatus::Current;
            current.transport = Transport::Paused;
            current.autoplay = false;
        }
    }

    async fn on_tick(&mut self) {
        if self
            .note
            .as_ref()
            .is_some_and(|(_, until)| Instant::now() >= *until)
        {
            self.note = None;
        }
        if let Some(retry) = &self.retry
            && Instant::now() >= retry.at
            && self.is_active()
            && let Some(current) = &mut self.current
            && current.token.is_none()
        {
            // Reload from the saved position with a new token.
            current.pending_load = true;
            current.autoplay = true;
            retry_marker(&mut self.retry);
            self.try_load().await;
        }
        self.skip_sponsor().await;
        if self
            .current
            .as_ref()
            .is_some_and(|c| c.transport == Transport::Playing)
        {
            self.commit(false).await;
        }
        let every = if matches!(self.status, SourceStatus::Ready(_)) {
            REFRESH_EVERY
        } else {
            RETRY_CONNECT_EVERY
        };
        if self.refreshed_at.is_none_or(|at| at.elapsed() >= every) {
            self.refresh().await;
        }
    }

    async fn skip_sponsor(&mut self) {
        let sponsor_skip = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.settings.sponsor_skip);
        if !sponsor_skip || !self.loaded() {
            return;
        }
        let Some(current) = &self.current else { return };
        let Kind::Episode(state) = &current.kind else {
            return;
        };
        if current.transport != Transport::Playing {
            return;
        }
        let Some(target) =
            logic::sponsor_jump(&state.spans, current.position_ms, &state.suppressed)
        else {
            return;
        };
        if self
            .engine(EngineCommand::Seek {
                position_ms: target,
            })
            .await
            .is_ok()
        {
            if let Some(current) = &mut self.current {
                current.position_ms = target;
            }
            self.note = Some((NOTE_SPONSOR.into(), Instant::now() + NOTE_FOR));
            // The synced position must never rest inside a skipped span.
            self.commit(true).await;
        }
    }
}

/// Keep the attempt count but stop the timer while the reload runs.
fn retry_marker(retry: &mut Option<RetryState>) {
    if let Some(retry) = retry {
        retry.at = Instant::now() + Duration::from_secs(3600);
    }
}

fn anchor_for(timeline: &Timeline, locator: Option<&BookLocator>) -> Result<Anchor, Failure> {
    let ordinal = logic::start_ordinal_for(timeline, locator);
    let segment = timeline
        .segments
        .iter()
        .find(|s| s.ordinal == ordinal)
        .ok_or_else(|| Failure::new("unavailable", "this book has no narratable text"))?;
    Ok(Anchor {
        resource: segment.resource.clone(),
        selector: segment.selector.clone(),
        character_offset: segment.start_offset,
        text_context: locator.map(|l| l.text_context.clone()).unwrap_or_default(),
    })
}

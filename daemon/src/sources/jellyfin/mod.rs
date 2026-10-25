//! Jellyfin source: Artists, Albums, Playlists and Videos; audio through the
//! engine; video handed off to its own mpv window; plays and progress
//! reported to Jellyfin.
//!
//! The actor owns the queue (`queue.rs`). It drives the engine only while
//! `ctx.active` names Jellyfin. An accepted `item.play` that should sound
//! starts when the session makes Jellyfin active, or at once when it already is.
mod api;
mod library;
mod queue;
mod video;

use super::{SourceContext, SourceRequest, SourceSession, SourceState};
use crate::{
    config::JellyfinConfig,
    engine::{EngineCommand, EngineEvent, EngineEventKind, Media},
    protocol::{
        Capabilities, Empty, Failure, Group, Output, OwnPlayback, OwnTransport, Page, PlayMode,
        QueueStatus, Retry, SourceAction, SourceId, SourceStatus, Track, Transport,
    },
};
use api::{BaseItem, Client, ItemsPage, PlaybackReport, ReportKind, TICKS_PER_MS};
use library::{GroupId, Node, PAGE, Playable};
use queue::{NewEntry, Queue};
use std::time::Duration;
use tokio::{
    sync::{broadcast, mpsc},
    time::Instant,
};
use video::{VideoEvent, VideoHandle, VideoRequest};

const ME: SourceId = SourceId::Jellyfin;
const RECONNECT_EVERY: Duration = Duration::from_secs(30);
const PROGRESS_EVERY: Duration = Duration::from_secs(10);
/// A track stopped from 90 % on counts as played (Jellyfin's default).
const PLAYED_MIN: f64 = 0.90;

pub use crate::sources::retry_delay;

/// A video plays in its own mpv window, not the engine: playing it must not switch
/// or pause the active source. The followed window makes own music yield instead.
pub fn is_hand_off(item: &str) -> bool {
    matches!(Playable::parse(item), Ok(Playable::Video(_)))
}

pub async fn run(ctx: SourceContext<JellyfinConfig>) -> anyhow::Result<()> {
    Actor::new(ctx).run().await
}

enum Connection {
    Connecting,
    Ready { client: Client, groups: Vec<Group> },
    Unavailable(Failure),
}

/// One engine load of the current occurrence.
struct Loaded {
    token: u64,
    occurrence: String,
    report: PlaybackReport,
    reported: bool,
}

struct Pending {
    attempt: u32,
    at: Instant,
    next_at_ms: u64,
}

/// Reports go out in order on their own task, so the actor never waits on them.
enum Report {
    Playback(ReportKind, PlaybackReport),
    Played(String),
}

struct Actor {
    ctx: SourceContext<JellyfinConfig>,
    connection: Connection,
    queue: Queue,
    loaded: Option<Loaded>,
    preload: Option<(u64, String)>,
    next_token: u64,
    /// Start the current occurrence when Jellyfin becomes active.
    want_start: bool,
    resume_ms: u64,
    transport: Transport,
    position_ms: Option<u64>,
    volume: Option<f64>,
    measured: Option<Track>,
    /// The pending retry timer, shown as `own.retry`.
    retry: Option<Pending>,
    /// Failures of the current occurrence since it last started.
    attempts: u32,
    video: Option<VideoHandle>,
    video_events: (mpsc::Sender<VideoEvent>, mpsc::Receiver<VideoEvent>),
    reports: mpsc::UnboundedSender<(Client, Report)>,
}

fn unsupported(what: &str) -> Failure {
    Failure::new("unsupported", format!("Jellyfin does not support {what}"))
}

fn now_ms() -> u64 {
    video::now_ms()
}

async fn reporter(mut jobs: mpsc::UnboundedReceiver<(Client, Report)>) {
    while let Some((client, job)) = jobs.recv().await {
        match job {
            Report::Playback(kind, report) => video::send_report(&client, kind, &report).await,
            Report::Played(id) => {
                if let Err(error) = client.record_play(&id, now_ms()).await {
                    tracing::warn!(code = %error.code, message = %error.message, "Jellyfin play not recorded");
                }
            }
        }
    }
}

impl Actor {
    fn new(ctx: SourceContext<JellyfinConfig>) -> Self {
        let (reports, jobs) = mpsc::unbounded_channel();
        tokio::spawn(reporter(jobs));
        Self {
            ctx,
            connection: Connection::Connecting,
            queue: Queue::default(),
            loaded: None,
            preload: None,
            next_token: 0,
            want_start: false,
            resume_ms: 0,
            transport: Transport::Stopped,
            position_ms: None,
            volume: None,
            measured: None,
            retry: None,
            attempts: 0,
            video: None,
            video_events: mpsc::channel(4),
            reports,
        }
    }

    fn client(&self) -> Result<&Client, Failure> {
        match &self.connection {
            Connection::Ready { client, .. } => Ok(client),
            Connection::Connecting => Err(Failure::new("connecting", "Jellyfin is connecting")),
            Connection::Unavailable(failure) => Err(failure.clone()),
        }
    }

    fn active(&self) -> bool {
        *self.ctx.active.borrow() == Some(ME)
    }

    async fn run(mut self) -> anyhow::Result<()> {
        let mut events = self.ctx.engine.events.subscribe();
        let mut engine_state = self.ctx.engine.state.clone();
        let mut active = self.ctx.active.clone();
        let mut reconnect_at = Instant::now();
        let mut progress = tokio::time::interval(PROGRESS_EVERY);
        loop {
            self.publish();
            let retry_at = self.retry.as_ref().map(|r| r.at);
            let connected = matches!(self.connection, Connection::Ready { .. });
            tokio::select! {
                request = self.ctx.requests.recv() => {
                    let Some(request) = request else { break };
                    self.request(request).await;
                }
                event = events.recv() => match event {
                    Ok(event) => self.engine_event(event).await,
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                changed = engine_state.changed() => {
                    if changed.is_err() { break }
                    self.engine_state();
                }
                changed = active.changed() => {
                    if changed.is_err() { break }
                    self.activation().await;
                }
                Some(event) = self.video_events.1.recv() => {
                    let VideoEvent::Ended { id, .. } = &event;
                    if self.video.as_ref().is_some_and(|v| &v.id == id) {
                        self.video = None;
                    }
                }
                _ = tokio::time::sleep_until(retry_at.unwrap_or_else(Instant::now)), if retry_at.is_some() => {
                    self.retry_now().await;
                }
                _ = tokio::time::sleep_until(reconnect_at), if !connected => {
                    self.connect().await;
                    reconnect_at = Instant::now() + RECONNECT_EVERY;
                }
                _ = progress.tick() => {
                    if self.transport == Transport::Playing {
                        self.report_progress();
                    }
                }
            }
        }
        if let Some(video) = self.video.take() {
            video.close().await;
        }
        Ok(())
    }

    async fn connect(&mut self) {
        match Client::connect(&self.ctx.config).await {
            Ok(client) => {
                let mut counts = [None; 4];
                for (slot, group) in counts.iter_mut().zip([
                    GroupId::Artists,
                    GroupId::Albums,
                    GroupId::Playlists,
                    GroupId::Videos,
                ]) {
                    *slot = client
                        .get::<ItemsPage>(&library::count(group, &client.user_id))
                        .await
                        .ok()
                        .and_then(|p| p.total_record_count);
                }
                tracing::info!(user = %client.user_id, "Jellyfin ready");
                self.connection = Connection::Ready {
                    client,
                    groups: library::groups(counts),
                };
            }
            Err(failure) => {
                tracing::warn!(code = %failure.code, message = %failure.message, "Jellyfin unavailable");
                self.connection = Connection::Unavailable(failure);
            }
        }
    }

    // ---------- requests ----------

    async fn request(&mut self, request: SourceRequest) {
        let SourceRequest { id, action, reply } = request;
        match action {
            // Library reads run on their own task so playback never waits on them.
            SourceAction::Open { node, cursor, .. } => match self.client().cloned() {
                Ok(client) => {
                    let groups = match &self.connection {
                        Connection::Ready { groups, .. } => groups.clone(),
                        _ => vec![],
                    };
                    tokio::spawn(async move {
                        let _ =
                            reply.send(open(&client, &groups, id, node, cursor).await.map(Some));
                    });
                }
                Err(failure) => {
                    let _ = reply.send(Err(failure));
                }
            },
            SourceAction::Search { query } => match self.client().cloned() {
                Ok(client) => {
                    tokio::spawn(async move {
                        let _ = reply.send(search(&client, id, query).await.map(Some));
                    });
                }
                Err(failure) => {
                    let _ = reply.send(Err(failure));
                }
            },
            SourceAction::MarkPlayed { item, played, .. } => match self.client().cloned() {
                Ok(client) => {
                    tokio::spawn(async move {
                        let result = match Playable::parse(&item) {
                            Ok(playable) => client.mark_played(playable.id(), played).await,
                            Err(failure) => Err(failure),
                        };
                        let _ = reply.send(result.map(|()| None));
                    });
                }
                Err(failure) => {
                    let _ = reply.send(Err(failure));
                }
            },
            action => {
                let result = self.act(&id, action).await;
                let _ = reply.send(result.map(|()| None));
            }
        }
    }

    async fn act(&mut self, id: &str, action: SourceAction) -> Result<(), Failure> {
        match action {
            SourceAction::Play { item, mode, .. } => self.play(Playable::parse(&item)?, mode).await,
            SourceAction::Jump { occurrence } => {
                self.leave(false);
                let index = self.queue.jump(&occurrence)?;
                self.load(index, 0).await
            }
            SourceAction::Remove { occurrence } => {
                let leaving = self
                    .queue
                    .current()
                    .is_some_and(|e| e.occurrence == occurrence);
                if leaving {
                    self.leave(false);
                }
                let was_current = self.queue.remove(&occurrence)?;
                if was_current {
                    match self.queue.current_index() {
                        Some(index) if self.transport == Transport::Playing => {
                            self.load(index, 0).await?
                        }
                        _ => self.stop_engine().await,
                    }
                } else {
                    self.refresh_preload().await;
                }
                Ok(())
            }
            SourceAction::Transport { action } => self.transport(id, action).await,
            SourceAction::Seek {
                position_ms,
                occurrence,
            } => {
                let current = self.loaded.as_ref().map(|l| l.occurrence.as_str());
                if current != Some(occurrence.as_str()) {
                    return Err(Failure::new("stale", "the seek names another occurrence"));
                }
                self.engine(EngineCommand::Seek { position_ms }).await?;
                self.position_ms = Some(position_ms);
                self.report_progress();
                Ok(())
            }
            SourceAction::Volume { value } => self.engine(EngineCommand::Volume { value }).await,
            SourceAction::Repeat { mode } => {
                self.queue.repeat = mode;
                self.refresh_preload().await;
                Ok(())
            }
            SourceAction::Shuffle { enabled } => {
                let seed = uuid::Uuid::new_v4().as_u64_pair().0;
                self.queue.set_shuffle(enabled, seed);
                self.refresh_preload().await;
                Ok(())
            }
            SourceAction::Speed { .. } => Err(unsupported("speed for music")),
            SourceAction::SelectOutput { .. } => Err(unsupported("other outputs")),
            SourceAction::SavePlaylist { .. } | SourceAction::AddToPlaylist { .. } => {
                Err(unsupported("playlist edits in Pyxamp"))
            }
            SourceAction::StartStation { .. } => Err(unsupported("stations")),
            SourceAction::ResolveConflict { .. } => Err(unsupported("position conflicts")),
            SourceAction::SetEq(_) | SourceAction::Balance { .. } => {
                Err(unsupported("EQ or balance (the engine owns them)"))
            }
            SourceAction::Open { .. }
            | SourceAction::Search { .. }
            | SourceAction::MarkPlayed { .. } => unreachable!("handled in request"),
        }
    }

    async fn play(&mut self, playable: Playable, mode: PlayMode) -> Result<(), Failure> {
        let client = self.client()?.clone();
        if let Playable::Video(id) = playable {
            if mode != PlayMode::Now {
                return Err(Failure::new(
                    "videoHandOff",
                    "a video opens in its own window and cannot be queued",
                ));
            }
            return self.hand_off(client, id).await;
        }
        let items: Vec<BaseItem> = match library::expansion(&playable, &client.user_id) {
            None => match &playable {
                Playable::Track(id) => vec![client.item(id).await?],
                _ => unreachable!("only tracks and videos have no expansion"),
            },
            Some(query) => client.get::<ItemsPage>(&query).await?.items,
        };
        let entries: Vec<NewEntry> = items.iter().filter_map(NewEntry::from_item).collect();
        if entries.is_empty() {
            return Err(Failure::new("empty", "nothing playable in that item"));
        }
        match mode {
            PlayMode::Now => {
                self.leave(false);
                self.queue.replace(entries);
                self.resume_ms = 0;
                self.want_start = true;
            }
            PlayMode::Next | PlayMode::Last => {
                let at = if mode == PlayMode::Next {
                    self.queue.insert_next(entries)
                } else {
                    self.queue.append(entries)
                };
                if self.queue.current().is_none() {
                    self.queue.set_current(at, false);
                    self.resume_ms = 0;
                    self.want_start = true;
                } else if !self.active() {
                    // Becoming active resumes the current occurrence in place.
                    self.want_start = true;
                } else {
                    self.refresh_preload().await;
                }
            }
        }
        if self.want_start && self.active() {
            self.start().await?;
        }
        Ok(())
    }

    async fn hand_off(&mut self, client: Client, id: String) -> Result<(), Failure> {
        video::mpris_script()?;
        let item = client.item(&id).await?;
        let start_ms = item
            .user_data
            .as_ref()
            .filter(|d| !d.played)
            .map_or(0, |d| d.playback_position_ticks / TICKS_PER_MS);
        if let Some(old) = self.video.take() {
            old.close().await;
        }
        let request = VideoRequest {
            title: item.name.clone().filter(|n| !n.is_empty()),
            duration_ms: library::duration_ms(&item),
            id,
            start_ms,
        };
        self.video = Some(video::open(client, request, self.video_events.0.clone()).await?);
        Ok(())
    }

    async fn transport(&mut self, id: &str, action: OwnTransport) -> Result<(), Failure> {
        match action {
            OwnTransport::Pause if id.starts_with("internal:") => {
                // The session switches away: pause here and keep the place.
                self.retry = None;
                self.want_start = false;
                if let Some(video) = &self.video {
                    video.pause().await;
                }
                if self.ours_in_engine() {
                    self.engine(EngineCommand::Pause).await?;
                    self.transport = Transport::Paused;
                    self.report_progress();
                }
                Ok(())
            }
            OwnTransport::Pause => {
                self.engine(EngineCommand::Pause).await?;
                self.transport = Transport::Paused;
                self.report_progress();
                Ok(())
            }
            OwnTransport::Play => self.start().await,
            OwnTransport::PlayPause if self.transport == Transport::Playing => {
                Box::pin(self.transport(id, OwnTransport::Pause)).await
            }
            OwnTransport::PlayPause => Box::pin(self.transport(id, OwnTransport::Play)).await,
            OwnTransport::Stop => {
                self.leave(false);
                self.stop_engine().await;
                self.resume_ms = 0;
                Ok(())
            }
            OwnTransport::Next => {
                let index = self
                    .queue
                    .peek_next(false)
                    .ok_or_else(|| Failure::new("endOfQueue", "no next item"))?;
                self.leave(false);
                self.queue.set_current(index, false);
                self.load(index, 0).await
            }
            OwnTransport::Previous => match self.queue.peek_previous() {
                Some(index) => {
                    self.leave(false);
                    self.queue.set_current(index, false);
                    self.load(index, 0).await
                }
                None if self.loaded.is_some() => {
                    self.engine(EngineCommand::Seek { position_ms: 0 }).await
                }
                None => Err(Failure::new("startOfQueue", "no previous item")),
            },
        }
    }

    // ---------- engine ----------

    async fn engine(&self, command: EngineCommand) -> Result<(), Failure> {
        if !self.active() {
            return Err(Failure::new(
                "inactive",
                "Jellyfin is not the active source",
            ));
        }
        self.ctx.engine.send(Some(ME), command).await
    }

    fn ours_in_engine(&self) -> bool {
        let state = self.ctx.engine.state.borrow();
        state.owner == Some(ME)
            && self.loaded.as_ref().map(|l| l.token) == state.token
            && state.token.is_some()
    }

    fn token(&mut self) -> u64 {
        self.next_token += 1;
        self.next_token
    }

    fn media(&self, index: usize) -> Result<Media, Failure> {
        let client = self.client()?;
        let entry = self
            .queue
            .entry(index)
            .ok_or_else(|| Failure::new("notFound", "no such queue entry"))?;
        Ok(Media {
            url: client.audio_url(&entry.id)?,
            headers: client.stream_headers(),
            track: entry.track.clone(),
        })
    }

    /// Start or resume the current occurrence (Jellyfin is active).
    async fn start(&mut self) -> Result<(), Failure> {
        self.want_start = false;
        if self.ours_in_engine() {
            self.engine(EngineCommand::Play).await
        } else {
            let index = match self.queue.current_index() {
                Some(index) => Some(index),
                None if !self.queue.is_empty() => {
                    self.queue.set_current(0, false);
                    Some(0)
                }
                None => None,
            };
            match index {
                Some(index) => self.load(index, self.resume_ms).await,
                None => Err(Failure::new("empty", "the Jellyfin queue is empty")),
            }
        }
    }

    async fn load(&mut self, index: usize, start_ms: u64) -> Result<(), Failure> {
        self.retry = None;
        let media = self.media(index)?;
        let token = self.token();
        let entry = self.queue.entry(index).expect("media checked the index");
        let loaded = Loaded {
            token,
            occurrence: entry.occurrence.clone(),
            report: PlaybackReport {
                item_id: entry.id.clone(),
                play_session_id: uuid::Uuid::new_v4().simple().to_string(),
                position_ticks: start_ms * TICKS_PER_MS,
                is_paused: false,
                can_seek: true,
                play_method: "DirectStream",
            },
            reported: false,
        };
        self.loaded = Some(loaded);
        self.preload = None;
        self.measured = None;
        self.position_ms = Some(start_ms);
        self.resume_ms = start_ms;
        self.engine(EngineCommand::Load {
            token,
            media,
            start_ms,
            paused: false,
        })
        .await?;
        self.transport = Transport::Playing;
        self.refresh_preload().await;
        Ok(())
    }

    /// Tell the engine what follows the current item (gapless).
    async fn refresh_preload(&mut self) {
        if self.loaded.is_none() || !self.active() {
            return;
        }
        let next = self.queue.peek_next(true);
        let media = next.and_then(|i| self.media(i).ok());
        let token = media.as_ref().map(|_| self.token());
        let preload = match (token, next) {
            (Some(token), Some(index)) => self
                .queue
                .entry(index)
                .map(|e| (token, e.occurrence.clone())),
            _ => None,
        };
        let command = EngineCommand::Preload {
            token: token.unwrap_or_else(|| self.loaded.as_ref().map_or(0, |l| l.token)),
            media,
        };
        match self.engine(command).await {
            Ok(()) => self.preload = preload,
            Err(error) => {
                self.preload = None;
                tracing::debug!(code = %error.code, "Jellyfin preload refused");
            }
        }
    }

    async fn stop_engine(&mut self) {
        if self.ours_in_engine() {
            let _ = self.engine(EngineCommand::Stop).await;
        }
        self.loaded = None;
        self.preload = None;
        self.retry = None;
        self.transport = Transport::Stopped;
        self.position_ms = None;
    }

    /// Report the end of the loaded item; `finished` is a natural end.
    fn leave(&mut self, finished: bool) {
        self.retry = None;
        self.attempts = 0;
        let Some(loaded) = self.loaded.take() else {
            return;
        };
        self.preload = None;
        let duration = self
            .queue
            .index_of(&loaded.occurrence)
            .and_then(|i| self.queue.entry(i))
            .and_then(|e| e.track.duration_ms);
        let position = if finished {
            duration.or(self.position_ms)
        } else {
            self.position_ms
        };
        let played = finished
            || matches!((position, duration), (Some(p), Some(d)) if d > 0 && p as f64 >= PLAYED_MIN * d as f64);
        if loaded.reported {
            let mut report = loaded.report;
            report.position_ticks = position.unwrap_or(0) * TICKS_PER_MS;
            let item = report.item_id.clone();
            self.send(Report::Playback(ReportKind::Stopped, report));
            if played {
                self.send(Report::Played(item));
            }
        }
    }

    fn send(&self, report: Report) {
        if let Ok(client) = self.client() {
            let _ = self.reports.send((client.clone(), report));
        }
    }

    fn report_progress(&mut self) {
        let position = self.position_ms.unwrap_or(0);
        let paused = self.transport != Transport::Playing;
        let Some(loaded) = self.loaded.as_mut().filter(|l| l.reported) else {
            return;
        };
        loaded.report.position_ticks = position * TICKS_PER_MS;
        loaded.report.is_paused = paused;
        let report = loaded.report.clone();
        self.send(Report::Playback(ReportKind::Progress, report));
    }

    fn report_start(&mut self) {
        let Some(loaded) = self.loaded.as_mut().filter(|l| !l.reported) else {
            return;
        };
        loaded.reported = true;
        let report = loaded.report.clone();
        self.send(Report::Playback(ReportKind::Start, report));
    }

    async fn engine_event(&mut self, event: EngineEvent) {
        if event.owner != ME {
            return;
        }
        let current = self.loaded.as_ref().map(|l| l.token);
        let preloaded = self.preload.as_ref().map(|p| p.0);
        match event.kind {
            EngineEventKind::Started if Some(event.token) == current => {
                self.retry = None;
                self.attempts = 0;
                self.queue.set_current_status(QueueStatus::Current);
                self.report_start();
            }
            EngineEventKind::Started if Some(event.token) == preloaded => {
                self.advance_to_preload().await;
                self.report_start();
            }
            EngineEventKind::Ended if Some(event.token) == current => {
                if self.preload.is_some() {
                    self.advance_to_preload().await;
                } else {
                    self.leave(true);
                    self.queue.finish();
                    self.transport = Transport::Stopped;
                    self.position_ms = None;
                    self.resume_ms = 0;
                }
            }
            EngineEventKind::Failed(failure) if Some(event.token) == current => {
                self.failed(failure).await;
            }
            _ => {}
        }
    }

    /// The engine moved on to the preloaded item.
    async fn advance_to_preload(&mut self) {
        let Some((token, occurrence)) = self.preload.take() else {
            return;
        };
        self.leave(true);
        let Some(index) = self.queue.index_of(&occurrence) else {
            return;
        };
        self.queue.set_current(index, true);
        let entry = self.queue.entry(index).expect("index found");
        self.loaded = Some(Loaded {
            token,
            occurrence,
            report: PlaybackReport {
                item_id: entry.id.clone(),
                play_session_id: uuid::Uuid::new_v4().simple().to_string(),
                position_ticks: 0,
                is_paused: false,
                can_seek: true,
                play_method: "DirectStream",
            },
            reported: false,
        });
        self.position_ms = Some(0);
        self.resume_ms = 0;
        self.measured = None;
        self.report_start();
        self.refresh_preload().await;
    }

    async fn failed(&mut self, failure: Failure) {
        tracing::warn!(code = %failure.code, message = %failure.message, "Jellyfin track failed");
        self.attempts += 1;
        let attempt = self.attempts;
        match retry_delay(attempt) {
            Some(delay) => {
                self.queue.set_current_status(QueueStatus::Retrying);
                self.retry = Some(Pending {
                    attempt,
                    at: Instant::now() + delay,
                    next_at_ms: now_ms() + delay.as_millis() as u64,
                });
                self.transport = Transport::Stopped;
            }
            None => {
                self.retry = None;
                self.attempts = 0;
                self.queue.set_current_status(QueueStatus::Failed);
                self.loaded = None;
                self.preload = None;
                match self.queue.peek_next(false) {
                    Some(index) => {
                        self.queue.set_current(index, false);
                        if let Err(error) = self.load(index, 0).await {
                            tracing::warn!(code = %error.code, "Jellyfin could not load the next track");
                        }
                    }
                    None => {
                        self.queue.finish();
                        self.transport = Transport::Stopped;
                    }
                }
            }
        }
    }

    async fn retry_now(&mut self) {
        if self.retry.take().is_none() {
            return;
        }
        let Some(index) = self.queue.current_index() else {
            return;
        };
        let start = self.resume_ms;
        if let Err(error) = self.load(index, start).await {
            // A refused load counts as one more failure.
            self.failed(error).await;
        }
    }

    fn engine_state(&mut self) {
        let state = self.ctx.engine.state.borrow().clone();
        if state.owner != Some(ME) || state.token.is_none() {
            return;
        }
        if state.token == self.loaded.as_ref().map(|l| l.token) {
            let was = self.transport;
            self.transport = state.transport;
            if let Some(p) = state.position_ms {
                self.position_ms = Some(p);
                self.resume_ms = p;
            }
            self.volume = state.volume.or(self.volume);
            self.measured = state.track;
            if was != self.transport
                && matches!(self.transport, Transport::Playing | Transport::Paused)
            {
                self.report_progress();
            }
        }
    }

    async fn activation(&mut self) {
        if self.active() {
            if self.want_start
                && let Err(error) = self.start().await
            {
                tracing::warn!(code = %error.code, message = %error.message, "Jellyfin could not start playback");
            }
        } else {
            self.retry = None;
            self.attempts = 0;
            if self.transport == Transport::Playing {
                self.transport = Transport::Paused;
                self.report_progress();
            }
        }
    }

    // ---------- state ----------

    fn publish(&self) {
        let status = match &self.connection {
            Connection::Connecting => SourceStatus::Connecting(Empty {}),
            Connection::Ready { .. } => SourceStatus::Ready(Empty {}),
            Connection::Unavailable(failure) => SourceStatus::Unavailable(failure.clone()),
        };
        let mut state = SourceState::new(ME, status);
        if let Connection::Ready { groups, .. } = &self.connection {
            state.summary.groups = groups.clone();
        }
        state.session = self.session();
        self.ctx.state.send_if_modified(|old| {
            let changed = *old != state;
            if changed {
                *old = state;
            }
            changed
        });
    }

    fn session(&self) -> Option<SourceSession> {
        if self.queue.is_empty() {
            return None;
        }
        let current = self.queue.current();
        let mut track = current.map(|e| e.track.clone()).unwrap_or_default();
        if let Some(measured) = &self.measured {
            track.bitrate_kbps = measured.bitrate_kbps;
            track.sample_rate_hz = measured.sample_rate_hz;
            track.bit_depth = measured.bit_depth;
            track.channels = measured.channels;
            track.duration_ms = track.duration_ms.or(measured.duration_ms);
        }
        let loaded = self.loaded.is_some();
        Some(SourceSession {
            own: OwnPlayback {
                source: ME,
                output: Output {
                    id: "local".into(),
                    name: "This computer".into(),
                    local: true,
                    active: true,
                },
                capabilities: Capabilities {
                    play: true,
                    pause: loaded,
                    stop: loaded,
                    previous: current.is_some(),
                    next: self.queue.peek_next(false).is_some(),
                    seek: loaded && track.duration_ms.is_some(),
                    volume: true,
                    repeat: true,
                    shuffle: true,
                },
                track,
                transport: self.transport,
                occurrence: current.map(|e| e.occurrence.clone()),
                position_ms: current.and(self.position_ms),
                volume: self.volume.filter(|v| (0.0..=1.0).contains(v)),
                repeat: Some(self.queue.repeat),
                shuffle: Some(self.queue.shuffle()),
                speed: None,
                skip: None,
                retry: self.retry.as_ref().and_then(|r| {
                    (r.next_at_ms > now_ms()).then_some(Retry {
                        attempt: r.attempt,
                        next_at_ms: r.next_at_ms,
                    })
                }),
                yielded: None,
                note: None,
            },
            queue: self.queue.wire(),
            outputs: vec![],
            conflicts: vec![],
        })
    }
}

async fn open(
    client: &Client,
    groups: &[Group],
    id: String,
    node: Option<String>,
    cursor: Option<String>,
) -> Result<Page, Failure> {
    let parsed = Node::parse(node.as_deref())?;
    let start = library::parse_cursor(cursor.as_deref())?;
    let page = |items, next_cursor, total| Page {
        id: id.clone(),
        source: Some(ME),
        node: node.clone(),
        query: None,
        items,
        next_cursor,
        total,
    };
    match &parsed {
        Node::Root => {
            let items = library::group_items(groups);
            let total = items.len() as u64;
            Ok(page(items, None, Some(total)))
        }
        Node::Group(GroupId::Videos) => {
            let query = library::listing(&parsed, &client.user_id).expect("a group lists");
            let views: ItemsPage = client.get(&query).await?;
            let items: Vec<_> = library::videos_from_views(&views.items)
                .iter()
                .filter_map(library::to_item)
                .collect();
            let total = items.len() as u64;
            Ok(page(items, None, Some(total)))
        }
        _ => {
            let query = library::listing(&parsed, &client.user_id).expect("not the root");
            let answer: ItemsPage = client.get(&library::paged(query, start, PAGE)).await?;
            let fetched = answer.items.len() as u64;
            let next = answer
                .total_record_count
                .is_some_and(|total| start + fetched < total)
                .then(|| (start + fetched).to_string())
                .filter(|_| fetched > 0);
            Ok(page(
                answer.items.iter().filter_map(library::to_item).collect(),
                next,
                answer.total_record_count,
            ))
        }
    }
}

async fn search(client: &Client, id: String, query: String) -> Result<Page, Failure> {
    let answer: ItemsPage = client
        .get(&library::search(&query, &client.user_id))
        .await?;
    Ok(Page {
        id,
        source: Some(ME),
        node: None,
        query: Some(query),
        items: answer.items.iter().filter_map(library::to_item).collect(),
        next_cursor: None,
        total: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_waits_2_s_then_10_s_then_gives_up() {
        assert_eq!(retry_delay(1), Some(Duration::from_secs(2)));
        assert_eq!(retry_delay(2), Some(Duration::from_secs(10)));
        assert_eq!(retry_delay(3), None);
    }
}

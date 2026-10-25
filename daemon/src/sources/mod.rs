//! Sources: Pyxis, Ossicle and Jellyfin, each an actor in its own module.
//!
//! Each source module exposes `pub async fn run(ctx: SourceContext<Config>) ->
//! anyhow::Result<()>`. It publishes [`SourceState`] on `ctx.state` and answers
//! every [`SourceRequest`] exactly once. `session.rs` routes requests: actions
//! that name a source go to it; own-playback and queue actions go to the active
//! source. Only the active source may drive the engine (`ctx.active`). When a
//! source stops being active, the session first sends it
//! `own.transport pause` with an id starting `internal:`; the source pauses at
//! its own end and keeps its place.
use crate::{
    engine::EngineHandle,
    protocol::{
        Conflict, Failure, Output, OwnPlayback, Page, QueueItem, SourceAction, SourceId,
        SourceStatus, SourceSummary,
    },
};
use tokio::sync::{mpsc, oneshot, watch};

pub mod jellyfin;
pub mod ossicle;
pub mod pyxis;

pub struct SourceRequest {
    pub id: String,
    pub action: SourceAction,
    /// `Ok(Some(page))` answers `library.open` and `search`; other actions reply `Ok(None)`.
    pub reply: oneshot::Sender<Result<Option<Page>, Failure>>,
}

/// What the active source contributes to the snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceSession {
    pub own: OwnPlayback,
    pub queue: Vec<QueueItem>,
    pub outputs: Vec<Output>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceState {
    pub summary: SourceSummary,
    /// The source's current session, kept while inactive so it resumes in place.
    pub session: Option<SourceSession>,
}
impl SourceState {
    pub fn new(source: SourceId, status: SourceStatus) -> Self {
        Self {
            summary: SourceSummary {
                source,
                label: source.label().into(),
                status,
                groups: vec![],
            },
            session: None,
        }
    }
}

pub struct SourceContext<C> {
    pub source: SourceId,
    pub config: C,
    pub engine: EngineHandle,
    pub active: watch::Receiver<Option<SourceId>>,
    pub state: watch::Sender<SourceState>,
    pub requests: mpsc::Receiver<SourceRequest>,
}

/// The session's side of one source.
pub struct SourceHandle {
    pub source: SourceId,
    pub requests: mpsc::Sender<SourceRequest>,
    pub state: watch::Receiver<SourceState>,
}
impl SourceHandle {
    pub async fn request(&self, id: String, action: SourceAction) -> Result<Option<Page>, Failure> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(SourceRequest { id, action, reply })
            .await
            .map_err(|_| Failure::new("source", format!("{} stopped", self.source.label())))?;
        response.await.map_err(|_| {
            Failure::new(
                "source",
                format!("{} dropped the request", self.source.label()),
            )
        })?
    }
}

pub fn channels<C>(
    source: SourceId,
    config: C,
    engine: EngineHandle,
    active: watch::Receiver<Option<SourceId>>,
) -> (SourceHandle, SourceContext<C>) {
    let (requests_tx, requests) = mpsc::channel(32);
    let (state_tx, state) = watch::channel(SourceState::new(
        source,
        SourceStatus::Connecting(Default::default()),
    ));
    (
        SourceHandle {
            source,
            requests: requests_tx,
            state,
        },
        SourceContext {
            source,
            config,
            engine,
            active,
            state: state_tx,
            requests,
        },
    )
}

/// Shared stub body until a source worker replaces its module's `run`.
pub(crate) async fn unavailable<C>(mut ctx: SourceContext<C>, reason: &str) -> anyhow::Result<()> {
    ctx.state.send_replace(SourceState::new(
        ctx.source,
        SourceStatus::Unavailable(Failure::new("notBuilt", reason)),
    ));
    while let Some(request) = ctx.requests.recv().await {
        let _ = request
            .reply
            .send(Err(Failure::new("notBuilt", reason.to_owned())));
    }
    Ok(())
}

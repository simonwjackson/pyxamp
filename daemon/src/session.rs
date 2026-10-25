//! Routes requests to sources and the engine, and merges their state into the snapshot.
//!
//! The session also yields own playback to foreign players (policy.rs): a
//! foreign player that starts while own audio plays pauses it once, and it
//! resumes only if all foreign playback ends within the resume limit.
use crate::{
    engine::{EngineCommand, EngineHandle, EngineState},
    policy::{self, YieldEffect, YieldPolicy},
    protocol::{
        Failure, Library, OwnTransport, Page, PlayerState, Request, Snapshot, SourceAction,
        SourceId, SourceStatus, Transport, Yielded,
    },
    sources::{SourceHandle, SourceState},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc, oneshot, watch};

/// The longest foreign playback after which own playback still resumes.
pub const RESUME_LIMIT_MS: u64 = 120_000;

/// Wall-clock milliseconds since the Unix epoch. Tests supply their own.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub fn wall_clock() -> Clock {
    Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    })
}

/// Yield state around the pure [`YieldPolicy`]: it also knows the resume
/// deadline that the snapshot shows as `own.yielded.resumeByMs`.
#[derive(Default)]
pub struct Yield {
    policy: YieldPolicy,
    /// Set while own playback is paused because of a foreign player.
    paused: Option<Paused>,
}
struct Paused {
    session: String,
    started_ms: u64,
    /// Auto-resume is off: expired or user action.
    stays: bool,
    /// The engine has reported the pause; until then own may still read as playing.
    observed: bool,
}

impl Yield {
    /// Feeds one observation; the caller applies the returned effect to the engine.
    pub fn update(
        &mut self,
        now_ms: u64,
        foreign_playing: bool,
        own: &policy::OwnPlayback,
    ) -> Option<YieldEffect> {
        let effect = self
            .policy
            .update(now_ms, usize::from(foreign_playing), own);
        match effect {
            Some(YieldEffect::Pause) => {
                if let policy::OwnPlayback::Playing { session } = own {
                    self.paused = Some(Paused {
                        session: session.clone(),
                        started_ms: now_ms,
                        stays: false,
                        observed: false,
                    });
                }
            }
            Some(YieldEffect::Resume) => self.paused = None,
            None if !foreign_playing => self.paused = None,
            None => {}
        }
        if let Some(paused) = &mut self.paused {
            let (paused_here, playing_here) = match own {
                policy::OwnPlayback::Paused { session } => (*session == paused.session, false),
                policy::OwnPlayback::Playing { session } => (false, *session == paused.session),
                policy::OwnPlayback::Idle => (false, false),
            };
            paused.observed |= paused_here;
            let waiting = playing_here && !paused.observed;
            if !paused_here && !waiting {
                // Own resumed by hand, or the item changed: it is no longer yielded.
                self.policy.cancel();
                self.paused = None;
            } else if now_ms.saturating_sub(paused.started_ms) > RESUME_LIMIT_MS {
                paused.stays = true;
            }
        }
        effect
    }

    /// A user action on own playback cancels the automatic resume.
    pub fn cancel(&mut self) {
        self.policy.cancel();
        if let Some(paused) = &mut self.paused {
            paused.stays = true;
        }
    }

    /// The snapshot's `own.yielded`: a deadline, or absent for "stays paused".
    pub fn yielded(&self) -> Option<Yielded> {
        self.paused.as_ref().map(|paused| Yielded {
            resume_by_ms: (!paused.stays).then(|| paused.started_ms + RESUME_LIMIT_MS),
        })
    }

    /// When the resume deadline passes, the snapshot must change to "stays paused".
    pub fn deadline_ms(&self) -> Option<u64> {
        self.paused
            .as_ref()
            .filter(|paused| !paused.stays)
            .map(|paused| paused.started_ms + RESUME_LIMIT_MS + 1)
    }
}

/// Own playback as the yield policy sees it: audio that the engine plays for
/// the active source. A remote output (console mode) does not use the engine,
/// so it never yields.
pub fn own_for_yield(engine: &EngineState, active: Option<SourceId>) -> policy::OwnPlayback {
    let (Some(owner), Some(token)) = (engine.owner, engine.token) else {
        return policy::OwnPlayback::Idle;
    };
    if Some(owner) != active {
        return policy::OwnPlayback::Idle;
    }
    let session = format!("{}:{token}", owner.label());
    match engine.transport {
        Transport::Playing => policy::OwnPlayback::Playing { session },
        Transport::Paused => policy::OwnPlayback::Paused { session },
        _ => policy::OwnPlayback::Idle,
    }
}

pub fn foreign_playing(player: &PlayerState) -> bool {
    matches!(player, PlayerState::Following(p) if p.transport == Transport::Playing)
}

/// Requests that the user made (face or MPRIS), not the session's own `internal:` ones.
fn user_action(id: &str, action: &SourceAction) -> bool {
    !id.starts_with("internal:")
        && matches!(
            action,
            SourceAction::Transport { .. }
                | SourceAction::Seek { .. }
                | SourceAction::Play { .. }
                | SourceAction::StartStation { .. }
                | SourceAction::Jump { .. }
        )
}

pub struct RequestEnvelope {
    pub request: Request,
    pub reply: oneshot::Sender<Result<Option<Page>, Failure>>,
}

const SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

struct Router {
    sources: Vec<SourceHandle>,
    engine: EngineHandle,
    active: watch::Sender<Option<SourceId>>,
    /// Serialises activation so two plays cannot interleave their pause/activate steps.
    switching: Mutex<()>,
    yielding: Arc<std::sync::Mutex<Yield>>,
    /// Wakes the merge, so a cancelled resume shows at once.
    changed: mpsc::Sender<()>,
}

impl Router {
    fn source(&self, id: SourceId) -> Result<&SourceHandle, Failure> {
        self.sources
            .iter()
            .find(|handle| handle.source == id)
            .ok_or_else(|| Failure::new("noSource", format!("{} is not configured", id.label())))
    }

    async fn handle(&self, request: Request) -> Result<Option<Page>, Failure> {
        let Request { id, action } = request;
        if user_action(&id, &action)
            && let Ok(mut yielding) = self.yielding.lock()
        {
            yielding.cancel();
            let _ = self.changed.try_send(());
        }
        match &action {
            SourceAction::SetEq(eq) => self
                .engine
                .send(None, EngineCommand::Eq(eq.clone()))
                .await
                .map(|()| None),
            SourceAction::Balance { value } => self
                .engine
                .send(None, EngineCommand::Balance { value: *value })
                .await
                .map(|()| None),
            SourceAction::Search { query } => self.search(id, query.clone()).await.map(Some),
            SourceAction::Play { source, .. } | SourceAction::StartStation { source, .. } => {
                let _guard = self.switching.lock().await;
                self.pause_previous(*source, &id).await?;
                let result = self.source(*source)?.request(id, action.clone()).await;
                if result.is_ok() {
                    self.active.send_replace(Some(*source));
                }
                result
            }
            _ => {
                let target = match action.source() {
                    Some(source) => source,
                    None => (*self.active.borrow())
                        .ok_or_else(|| Failure::new("idle", "no source is active"))?,
                };
                self.source(target)?.request(id, action).await
            }
        }
    }

    /// Pause the active source at its own end, unless it is `next`; it keeps its place.
    async fn pause_previous(&self, next: SourceId, id: &str) -> Result<(), Failure> {
        let previous = *self.active.borrow();
        if previous != Some(next)
            && let Some(previous) = previous
        {
            let pause = SourceAction::Transport {
                action: OwnTransport::Pause,
            };
            let _ = self
                .source(previous)?
                .request(format!("internal:deactivate:{id}"), pause)
                .await;
        }
        Ok(())
    }

    /// A source asked to become active (`SourceState::activation`), for example a
    /// remote console told this computer to play. It counts as a user action.
    async fn activate(&self, source: SourceId, generation: u64) {
        let _guard = self.switching.lock().await;
        if *self.active.borrow() == Some(source) {
            return;
        }
        if let Ok(mut yielding) = self.yielding.lock() {
            yielding.cancel();
        }
        let id = format!("{}-activation-{generation}", source.label());
        if let Err(failure) = self.pause_previous(source, &id).await {
            tracing::warn!(message = %failure.message, "Activation could not pause the old source");
        }
        self.active.send_replace(Some(source));
    }

    async fn search(&self, id: String, query: String) -> Result<Page, Failure> {
        let ready: Vec<&SourceHandle> = self
            .sources
            .iter()
            .filter(|handle| matches!(handle.state.borrow().summary.status, SourceStatus::Ready(_)))
            .collect();
        if ready.is_empty() {
            return Err(Failure::new("noSource", "no source is ready to search"));
        }
        let searches = ready.iter().map(|handle| {
            let action = SourceAction::Search {
                query: query.clone(),
            };
            tokio::time::timeout(SEARCH_TIMEOUT, handle.request(id.clone(), action))
        });
        let mut items = vec![];
        for result in futures_util::future::join_all(searches).await {
            if let Ok(Ok(Some(page))) = result {
                items.extend(page.items);
            }
        }
        Ok(Page {
            id,
            source: None,
            node: None,
            query: Some(query),
            items,
            next_cursor: None,
            total: None,
        })
    }
}

/// Pure merge of source and engine state into the snapshot fields step 2 adds.
pub fn merge(
    snapshot: &mut Snapshot,
    sources: &[SourceState],
    active: Option<SourceId>,
    engine: &EngineState,
    yielded: Option<Yielded>,
) {
    snapshot.library = if sources.is_empty() {
        Library::NoSources {}
    } else {
        Library::Sources {
            sources: sources.iter().map(|s| s.summary.clone()).collect(),
        }
    };
    let session = active.and_then(|active| {
        sources
            .iter()
            .find(|s| s.summary.source == active)
            .and_then(|s| s.session.clone())
    });
    // "Play on" must work before the first play: without an active session that
    // lists outputs, show every source's outputs (only Pyxis has any).
    let idle_outputs = || sources.iter().flat_map(|s| s.outputs.clone()).collect();
    match session {
        Some(session) => {
            let mut own = session.own;
            own.yielded = yielded;
            snapshot.own = Some(own);
            snapshot.queue = session.queue;
            snapshot.outputs = if session.outputs.is_empty() {
                idle_outputs()
            } else {
                session.outputs
            };
            snapshot.conflicts = session.conflicts;
        }
        None => {
            snapshot.own = None;
            snapshot.queue = vec![];
            snapshot.outputs = idle_outputs();
            snapshot.conflicts = vec![];
        }
    }
    snapshot.engine = Some(engine.status.clone());
}

pub async fn run(
    sources: Vec<SourceHandle>,
    engine: EngineHandle,
    active: watch::Sender<Option<SourceId>>,
    state: watch::Sender<Snapshot>,
    requests: mpsc::Receiver<RequestEnvelope>,
) -> anyhow::Result<()> {
    run_with_clock(sources, engine, active, state, requests, wall_clock()).await
}

pub async fn run_with_clock(
    sources: Vec<SourceHandle>,
    engine: EngineHandle,
    active: watch::Sender<Option<SourceId>>,
    state: watch::Sender<Snapshot>,
    mut requests: mpsc::Receiver<RequestEnvelope>,
    clock: Clock,
) -> anyhow::Result<()> {
    // Any source, engine or activation change wakes the merge.
    let (changed_tx, mut changed) = mpsc::channel::<()>(1);
    let mut watchers = vec![];
    for handle in &sources {
        let mut rx = handle.state.clone();
        let tx = changed_tx.clone();
        watchers.push(tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let _ = tx.try_send(());
            }
        }));
    }
    {
        let mut rx = engine.state.clone();
        let tx = changed_tx.clone();
        watchers.push(tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let _ = tx.try_send(());
            }
        }));
        let mut rx = active.subscribe();
        let tx = changed_tx.clone();
        watchers.push(tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let _ = tx.try_send(());
            }
        }));
        // The followed foreign player (mpris.rs) drives yield.
        let mut rx = state.subscribe();
        let tx = changed_tx.clone();
        watchers.push(tokio::spawn(async move {
            let mut last = rx.borrow_and_update().player.clone();
            while rx.changed().await.is_ok() {
                let player = rx.borrow_and_update().player.clone();
                if player != last {
                    last = player;
                    let _ = tx.try_send(());
                }
            }
        }));
    }
    let yielding = Arc::new(std::sync::Mutex::new(Yield::default()));
    let _ = changed_tx.try_send(());
    let router = Arc::new(Router {
        sources,
        engine,
        active,
        switching: Mutex::new(()),
        yielding: yielding.clone(),
        changed: changed_tx.clone(),
    });
    let mut deadline: Option<u64> = None;
    // Last activation generation seen per source; only an increase asks for activation.
    let mut activations: std::collections::HashMap<SourceId, u64> = router
        .sources
        .iter()
        .map(|h| (h.source, h.state.borrow().activation))
        .collect();
    let result = loop {
        let wait = deadline.map(|at| Duration::from_millis(at.saturating_sub(clock())));
        tokio::select! {
            () = async { tokio::time::sleep(wait.unwrap_or_default()).await }, if wait.is_some() => {
                let _ = changed_tx.try_send(());
                deadline = None;
            }
            request = requests.recv() => {
                let Some(RequestEnvelope { request, reply }) = request else { break Ok(()) };
                let router = router.clone();
                tokio::spawn(async move { let _ = reply.send(router.handle(request).await); });
            }
            Some(()) = changed.recv() => {
                let states: Vec<SourceState> = router.sources.iter().map(|h| h.state.borrow().clone()).collect();
                for state in &states {
                    let source = state.summary.source;
                    let seen = activations.entry(source).or_insert(state.activation);
                    if state.activation > *seen {
                        *seen = state.activation;
                        let (router, generation) = (router.clone(), state.activation);
                        tokio::spawn(async move { router.activate(source, generation).await });
                    }
                }
                let active = *router.active.borrow();
                let engine = router.engine.state.borrow().clone();
                let foreign = foreign_playing(&state.borrow().player);
                let (effect, yielded) = {
                    let mut yielding = yielding.lock().unwrap_or_else(|e| e.into_inner());
                    let effect = yielding.update(clock(), foreign, &own_for_yield(&engine, active));
                    deadline = yielding.deadline_ms();
                    (effect, yielding.yielded())
                };
                if let Some(effect) = effect {
                    let command = match effect {
                        YieldEffect::Pause => EngineCommand::Pause,
                        YieldEffect::Resume => EngineCommand::Play,
                    };
                    let handle = router.engine.clone();
                    tokio::spawn(async move {
                        if let Err(error) = handle.send(None, command).await {
                            tracing::warn!(error = %error.message, "Yield could not reach the engine");
                        }
                    });
                }
                state.send_if_modified(|snapshot| {
                    let before = snapshot.clone();
                    merge(snapshot, &states, active, &engine, yielded.clone());
                    let modified = *snapshot != before;
                    if modified { snapshot.revision += 1; }
                    modified
                });
            }
        }
    };
    for watcher in watchers {
        watcher.abort();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;

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
            transport: Transport::Paused,
            capabilities: Capabilities::default(),
            occurrence: None,
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

    #[test]
    fn merge_shows_every_source_and_only_the_active_session() {
        let mut pyxis = SourceState::new(SourceId::Pyxis, SourceStatus::Ready(Empty {}));
        pyxis.session = Some(crate::sources::SourceSession {
            own: own(SourceId::Pyxis),
            queue: vec![],
            outputs: vec![],
            conflicts: vec![],
        });
        let ossicle = SourceState::new(
            SourceId::Ossicle,
            SourceStatus::Unavailable(Failure::new("x", "y")),
        );
        let engine = EngineState::unavailable("not built");
        let mut snapshot = Snapshot::default();
        merge(
            &mut snapshot,
            &[pyxis.clone(), ossicle.clone()],
            None,
            &engine,
            None,
        );
        assert!(matches!(&snapshot.library, Library::Sources { sources } if sources.len() == 2));
        assert!(snapshot.own.is_none());
        merge(
            &mut snapshot,
            &[pyxis, ossicle],
            Some(SourceId::Pyxis),
            &engine,
            None,
        );
        assert_eq!(snapshot.own.as_ref().unwrap().source, SourceId::Pyxis);
        assert!(snapshot.validate().is_ok());
        merge(&mut snapshot, &[], None, &engine, None);
        assert_eq!(snapshot.library, Library::NoSources {});
    }

    fn playing() -> policy::OwnPlayback {
        policy::OwnPlayback::Playing {
            session: "Pyxis:1".into(),
        }
    }
    fn paused() -> policy::OwnPlayback {
        policy::OwnPlayback::Paused {
            session: "Pyxis:1".into(),
        }
    }

    #[test]
    fn yield_pauses_once_shows_deadline_and_resumes_inside_the_limit() {
        let mut y = Yield::default();
        let t0 = 1_000_000;
        assert_eq!(y.update(t0, true, &playing()), Some(YieldEffect::Pause));
        // The engine has not paused yet: no second pause, still yielded.
        assert_eq!(y.update(t0 + 10, true, &playing()), None);
        assert_eq!(y.update(t0 + 20, true, &paused()), None);
        assert_eq!(
            y.yielded(),
            Some(Yielded {
                resume_by_ms: Some(t0 + RESUME_LIMIT_MS)
            })
        );
        assert_eq!(y.deadline_ms(), Some(t0 + RESUME_LIMIT_MS + 1));
        assert_eq!(
            y.update(t0 + RESUME_LIMIT_MS, false, &paused()),
            Some(YieldEffect::Resume)
        );
        assert_eq!(y.yielded(), None);
        assert_eq!(y.update(t0 + RESUME_LIMIT_MS + 5, false, &playing()), None);
    }

    #[test]
    fn yield_stays_paused_after_the_limit_and_clears_when_foreign_ends() {
        let mut y = Yield::default();
        assert_eq!(y.update(0, true, &playing()), Some(YieldEffect::Pause));
        y.update(5, true, &paused());
        y.update(RESUME_LIMIT_MS + 1, true, &paused());
        assert_eq!(y.yielded(), Some(Yielded { resume_by_ms: None }));
        assert_eq!(y.deadline_ms(), None);
        assert_eq!(y.update(RESUME_LIMIT_MS + 2, false, &paused()), None);
        assert_eq!(y.yielded(), None);
    }

    #[test]
    fn user_action_cancels_and_paused_own_is_never_restarted() {
        let mut y = Yield::default();
        y.update(0, true, &playing());
        y.update(5, true, &paused());
        y.cancel();
        assert_eq!(y.yielded(), Some(Yielded { resume_by_ms: None }));
        assert_eq!(y.update(100, false, &paused()), None);
        // Already paused when the foreign player starts: no pause, no resume.
        let mut y = Yield::default();
        assert_eq!(y.update(0, true, &paused()), None);
        assert_eq!(y.yielded(), None);
        assert_eq!(y.update(100, false, &paused()), None);
        // The user resumes during the interruption: not yielded any more, no second pause.
        let mut y = Yield::default();
        y.update(0, true, &playing());
        y.update(5, true, &paused());
        assert_eq!(y.update(10, true, &playing()), None);
        assert_eq!(y.yielded(), None);
        assert_eq!(y.update(20, false, &playing()), None);
    }

    #[test]
    fn only_engine_playback_of_the_active_source_yields() {
        let mut engine = EngineState::unavailable("x");
        engine.owner = Some(SourceId::Pyxis);
        engine.token = Some(7);
        engine.transport = Transport::Playing;
        assert_eq!(
            own_for_yield(&engine, Some(SourceId::Pyxis)),
            policy::OwnPlayback::Playing {
                session: "Pyxis:7".into()
            }
        );
        assert_eq!(
            own_for_yield(&engine, Some(SourceId::Ossicle)),
            policy::OwnPlayback::Idle
        );
        engine.token = None;
        assert_eq!(
            own_for_yield(&engine, Some(SourceId::Pyxis)),
            policy::OwnPlayback::Idle
        );
        assert!(user_action(
            "face-1",
            &SourceAction::Transport {
                action: OwnTransport::Play
            }
        ));
        assert!(!user_action(
            "internal:deactivate:1",
            &SourceAction::Transport {
                action: OwnTransport::Pause
            }
        ));
    }
}

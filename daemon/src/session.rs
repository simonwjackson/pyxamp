//! Routes requests to sources and the engine, and merges their state into the snapshot.
//!
//! Seam version: routing, one active source, and the snapshot merge. The
//! `engine` worker owns this file and adds yield (policy.rs) and own MPRIS.
use crate::{
    engine::{EngineCommand, EngineHandle, EngineState},
    protocol::{
        Failure, Library, OwnTransport, Page, Request, Snapshot, SourceAction, SourceId,
        SourceStatus,
    },
    sources::{SourceHandle, SourceState},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc, oneshot, watch};

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
                let previous = *self.active.borrow();
                if previous != Some(*source)
                    && let Some(previous) = previous
                {
                    // Pause at the old source's end; it keeps its place.
                    let pause = SourceAction::Transport {
                        action: OwnTransport::Pause,
                    };
                    let _ = self
                        .source(previous)?
                        .request(format!("internal:deactivate:{id}"), pause)
                        .await;
                }
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
    match session {
        Some(session) => {
            snapshot.own = Some(session.own);
            snapshot.queue = session.queue;
            snapshot.outputs = session.outputs;
            snapshot.conflicts = session.conflicts;
        }
        None => {
            snapshot.own = None;
            snapshot.queue = vec![];
            snapshot.outputs = vec![];
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
    mut requests: mpsc::Receiver<RequestEnvelope>,
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
    }
    let _ = changed_tx.try_send(());
    let router = Arc::new(Router {
        sources,
        engine,
        active,
        switching: Mutex::new(()),
    });
    let result = loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(RequestEnvelope { request, reply }) = request else { break Ok(()) };
                let router = router.clone();
                tokio::spawn(async move { let _ = reply.send(router.handle(request).await); });
            }
            Some(()) = changed.recv() => {
                let states: Vec<SourceState> = router.sources.iter().map(|h| h.state.borrow().clone()).collect();
                let active = *router.active.borrow();
                let engine = router.engine.state.borrow().clone();
                state.send_if_modified(|snapshot| {
                    let before = snapshot.clone();
                    merge(snapshot, &states, active, &engine);
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
        );
        assert!(matches!(&snapshot.library, Library::Sources { sources } if sources.len() == 2));
        assert!(snapshot.own.is_none());
        merge(
            &mut snapshot,
            &[pyxis, ossicle],
            Some(SourceId::Pyxis),
            &engine,
        );
        assert_eq!(snapshot.own.as_ref().unwrap().source, SourceId::Pyxis);
        assert!(snapshot.validate().is_ok());
        merge(&mut snapshot, &[], None, &engine);
        assert_eq!(snapshot.library, Library::NoSources {});
    }
}

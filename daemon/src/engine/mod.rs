//! Own audio: one engine that the active source drives. Owned by the `engine` worker.
//!
//! The engine is an actor. Sources send [`EngineRequest`]s through an
//! [`EngineHandle`], read [`EngineState`] from a watch channel, and receive
//! [`EngineEvent`]s (started, ended, failed) on a broadcast channel. A `token`
//! names one load, so late events for an old item are ignored by the caller.
use crate::protocol::{EngineStatus, EqSettings, Failure, SourceId, Track, Transport};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

pub mod mpv;

#[derive(Clone, Debug, PartialEq)]
pub struct Media {
    /// A URL mpv can open: http(s) stream or local file.
    pub url: String,
    /// HTTP headers for the stream (for example an authorization header).
    pub headers: Vec<(String, String)>,
    /// What the source knows; the engine adds the format it measures.
    pub track: Track,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineCommand {
    /// Replace the current item and start at `start_ms`. `paused` loads without playing.
    Load {
        token: u64,
        media: Media,
        start_ms: u64,
        paused: bool,
    },
    /// The item to play after the current one, for gapless playback. `None` clears it.
    Preload {
        token: u64,
        media: Option<Media>,
    },
    Play,
    Pause,
    Stop,
    Seek {
        position_ms: u64,
    },
    Volume {
        value: f64,
    },
    Speed {
        rate: f64,
    },
    Eq(EqSettings),
    Balance {
        value: f64,
    },
}

pub struct EngineRequest {
    /// The source driving playback; `None` is the session (EQ, balance, yield).
    pub owner: Option<SourceId>,
    pub command: EngineCommand,
    pub reply: oneshot::Sender<Result<(), Failure>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineState {
    pub status: EngineStatus,
    pub owner: Option<SourceId>,
    pub token: Option<u64>,
    pub transport: Transport,
    pub position_ms: Option<u64>,
    /// Source metadata plus the measured format (bitrate, sample rate, bit depth, channels).
    pub track: Option<Track>,
    pub volume: Option<f64>,
    pub speed: Option<f64>,
}
impl EngineState {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: EngineStatus::Unavailable(Failure::new("engine", reason)),
            owner: None,
            token: None,
            transport: Transport::Stopped,
            position_ms: None,
            track: None,
            volume: None,
            speed: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineEventKind {
    Started,
    /// The item finished normally; a preloaded item, if any, now has its token.
    Ended,
    Failed(Failure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineEvent {
    pub owner: SourceId,
    pub token: u64,
    pub kind: EngineEventKind,
}

#[derive(Clone)]
pub struct EngineHandle {
    pub requests: mpsc::Sender<EngineRequest>,
    pub state: watch::Receiver<EngineState>,
    pub events: broadcast::Sender<EngineEvent>,
}
impl EngineHandle {
    pub async fn send(
        &self,
        owner: Option<SourceId>,
        command: EngineCommand,
    ) -> Result<(), Failure> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(EngineRequest {
                owner,
                command,
                reply,
            })
            .await
            .map_err(|_| Failure::new("engine", "the engine stopped"))?;
        response
            .await
            .map_err(|_| Failure::new("engine", "the engine dropped the request"))?
    }
}

/// The engine's side of the channels. `mpv::run` consumes it.
pub struct EngineContext {
    pub requests: mpsc::Receiver<EngineRequest>,
    pub state: watch::Sender<EngineState>,
    pub events: broadcast::Sender<EngineEvent>,
}

pub fn channels() -> (EngineHandle, EngineContext) {
    let (requests_tx, requests) = mpsc::channel(32);
    let (state_tx, state) = watch::channel(EngineState::unavailable("starting"));
    let (events, _) = broadcast::channel(64);
    (
        EngineHandle {
            requests: requests_tx,
            state,
            events: events.clone(),
        },
        EngineContext {
            requests,
            state: state_tx,
            events,
        },
    )
}

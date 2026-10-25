use crate::{CommandRequest, mpris, protocol::*, session::RequestEnvelope};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{
    fdo,
    zvariant::{OwnedObjectPath, Value},
};

pub struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "Use Quickshell IPC pyxamp.toggle".into(),
        ))
    }
    fn quit(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "Daemon is managed separately".into(),
        ))
    }
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Pyxamp"
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "pyxamp"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec![]
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }
}

/// Pyxamp's MPRIS player. While a foreign player is followed it forwards to
/// that player (step 1). Otherwise it reflects and controls own playback
/// through the session, like a request from the face.
pub struct Player {
    pub state: watch::Receiver<Snapshot>,
    pub commands: mpsc::Sender<CommandRequest>,
    pub requests: mpsc::Sender<RequestEnvelope>,
}

/// What Pyxamp's MPRIS reflects.
#[derive(Clone, Debug, PartialEq)]
pub enum Reflected {
    Foreign(Box<Following>),
    Own(Box<OwnPlayback>),
    Nothing,
}

/// Pure choice: the followed foreign player wins, else own playback.
pub fn reflected(snapshot: &Snapshot) -> Reflected {
    match (&snapshot.player, &snapshot.own) {
        (PlayerState::Following(p), _) => Reflected::Foreign(p.clone()),
        (_, Some(own)) => Reflected::Own(Box::new(own.clone())),
        _ => Reflected::Nothing,
    }
}

/// An MPRIS track id for one queue occurrence. Occurrence ids are arbitrary
/// text, so they are hex-encoded into one valid object path element.
pub fn own_track_id(occurrence: &str) -> String {
    let hex: String = occurrence.bytes().map(|b| format!("{b:02x}")).collect();
    format!("/org/pyxamp/track/o{hex}")
}

/// MPRIS-shaped view of own playback, so metadata and properties share step 1's code.
fn as_following(own: &OwnPlayback) -> Following {
    let mut track = own.track.clone();
    track.id = own.occurrence.as_deref().map(own_track_id);
    Following {
        target: Target {
            bus_name: "org.mpris.MediaPlayer2.pyxamp".into(),
            owner: String::new(),
        },
        identity: "Pyxamp".into(),
        track,
        transport: own.transport,
        capabilities: own.capabilities.clone(),
        position_ms: own.position_ms,
        volume: own.volume,
        repeat: own.repeat,
        shuffle: own.shuffle,
        rate: own.speed,
    }
}

impl Player {
    fn reflected(&self) -> Reflected {
        reflected(&self.state.borrow())
    }
    /// The player whose properties MPRIS shows: foreign, or own in the same shape.
    fn shown(&self) -> Option<Following> {
        match self.reflected() {
            Reflected::Foreign(p) => Some(*p),
            Reflected::Own(own) => Some(as_following(&own)),
            Reflected::Nothing => None,
        }
    }
    fn own(&self) -> Option<OwnPlayback> {
        match self.reflected() {
            Reflected::Own(own) => Some(*own),
            _ => None,
        }
    }
    async fn forward(&self, action: Action) -> fdo::Result<()> {
        let target = match self.reflected() {
            Reflected::Foreign(p) => p.target,
            _ => return Err(fdo::Error::NotSupported("noTarget".into())),
        };
        let (reply, response) = oneshot::channel();
        self.commands
            .try_send(CommandRequest {
                command: Command {
                    id: uuid::Uuid::new_v4().to_string(),
                    target,
                    action,
                },
                reply,
            })
            .map_err(|_| fdo::Error::Failed("Command queue full".into()))?;
        match tokio::time::timeout(std::time::Duration::from_secs(8), response).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(e))) if e.code == "unsupported" => Err(fdo::Error::NotSupported(e.message)),
            Ok(Ok(Err(e))) => Err(fdo::Error::Failed(format!("{}: {}", e.code, e.message))),
            _ => Err(fdo::Error::Failed(
                "Native operation timed out; outcome unknown".into(),
            )),
        }
    }
    /// Sends an own-playback request to the session. It counts as a user action.
    async fn request(&self, action: SourceAction) -> fdo::Result<()> {
        let request = Request {
            id: format!("mpris:{}", uuid::Uuid::new_v4()),
            action,
        };
        request
            .validate()
            .map_err(|e| fdo::Error::InvalidArgs(e.message))?;
        let (reply, response) = oneshot::channel();
        self.requests
            .try_send(RequestEnvelope { request, reply })
            .map_err(|_| fdo::Error::Failed("Request queue full".into()))?;
        match tokio::time::timeout(std::time::Duration::from_secs(8), response).await {
            Ok(Ok(Ok(_))) => Ok(()),
            Ok(Ok(Err(e))) => Err(fdo::Error::Failed(format!("{}: {}", e.code, e.message))),
            _ => Err(fdo::Error::Failed(
                "Own playback did not answer; outcome unknown".into(),
            )),
        }
    }
    /// Foreign player: forward the MPRIS action. Own playback: the matching request.
    async fn transport(&self, action: Action, own: OwnTransport) -> fdo::Result<()> {
        match self.reflected() {
            Reflected::Foreign(_) => self.forward(action).await,
            Reflected::Own(_) => self.request(SourceAction::Transport { action: own }).await,
            Reflected::Nothing => Err(fdo::Error::NotSupported("noTarget".into())),
        }
    }
    async fn own_seek(&self, own: OwnPlayback, position_ms: u64) -> fdo::Result<()> {
        if !own.capabilities.seek {
            return Err(fdo::Error::NotSupported("Own playback cannot seek".into()));
        }
        let occurrence = own
            .occurrence
            .ok_or_else(|| fdo::Error::NotSupported("No current item".into()))?;
        let position_ms = position_ms.min(own.track.duration_ms.unwrap_or(u64::MAX));
        self.request(SourceAction::Seek {
            position_ms,
            occurrence,
        })
        .await
    }
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn play(&self) -> fdo::Result<()> {
        self.transport(Action::Play(Empty {}), OwnTransport::Play)
            .await
    }
    async fn pause(&self) -> fdo::Result<()> {
        self.transport(Action::Pause(Empty {}), OwnTransport::Pause)
            .await
    }
    async fn play_pause(&self) -> fdo::Result<()> {
        self.transport(Action::PlayPause(Empty {}), OwnTransport::PlayPause)
            .await
    }
    async fn stop(&self) -> fdo::Result<()> {
        self.transport(Action::Stop(Empty {}), OwnTransport::Stop)
            .await
    }
    async fn next(&self) -> fdo::Result<()> {
        self.transport(Action::Next(Empty {}), OwnTransport::Next)
            .await
    }
    async fn previous(&self) -> fdo::Result<()> {
        self.transport(Action::Previous(Empty {}), OwnTransport::Previous)
            .await
    }
    async fn seek(&self, offset: i64) -> fdo::Result<()> {
        let player = self
            .shown()
            .ok_or_else(|| fdo::Error::NotSupported("noTarget".into()))?;
        let position = player
            .position_ms
            .ok_or_else(|| fdo::Error::NotSupported("No native position".into()))?;
        let position_ms = position
            .saturating_add_signed(offset / 1000)
            .min(player.track.duration_ms.unwrap_or(u64::MAX));
        if let Some(own) = self.own() {
            return self.own_seek(own, position_ms).await;
        }
        let track_id = player
            .track
            .id
            .ok_or_else(|| fdo::Error::NotSupported("No track ID".into()))?;
        self.forward(Action::Seek(Seek {
            position_ms,
            track_id,
        }))
        .await
    }
    async fn set_position(&self, track_id: OwnedObjectPath, position: i64) -> fdo::Result<()> {
        if position < 0 {
            return Err(fdo::Error::InvalidArgs("Negative position".into()));
        }
        let position_ms = position as u64 / 1000;
        if let Some(own) = self.own() {
            // MPRIS ignores a SetPosition for a track that is no longer current.
            if own.occurrence.as_deref().map(own_track_id).as_deref() != Some(track_id.as_str()) {
                return Ok(());
            }
            return self.own_seek(own, position_ms).await;
        }
        self.forward(Action::Seek(Seek {
            position_ms,
            track_id: track_id.to_string(),
        }))
        .await
    }
    fn open_uri(&self, _uri: &str) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "Pyxamp plays from its sources, not URIs".into(),
        ))
    }
    #[zbus(property)]
    fn playback_status(&self) -> &str {
        match self.shown().map(|p| p.transport) {
            Some(Transport::Playing) => "Playing",
            Some(Transport::Paused) => "Paused",
            _ => "Stopped",
        }
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<&'static str, Value<'static>> {
        mpris::metadata(self.shown().as_ref())
    }
    #[zbus(property)]
    fn position(&self) -> i64 {
        self.shown()
            .and_then(|p| p.position_ms)
            .unwrap_or(0)
            .saturating_mul(1000)
            .min(i64::MAX as u64) as i64
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        self.shown().is_some_and(|p| {
            let c = p.capabilities;
            c.play
                || c.pause
                || c.stop
                || c.seek
                || c.previous
                || c.next
                || c.volume
                || c.repeat
                || c.shuffle
        })
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.shown().is_some_and(|p| p.capabilities.play)
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.shown().is_some_and(|p| p.capabilities.pause)
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.shown().is_some_and(|p| p.capabilities.seek)
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.shown().is_some_and(|p| p.capabilities.next)
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.shown().is_some_and(|p| p.capabilities.previous)
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.shown().and_then(|p| p.volume).unwrap_or(0.0)
    }
    #[zbus(property)]
    async fn set_volume(&self, value: f64) -> fdo::Result<()> {
        if self.own().is_some() {
            let value = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                return Err(fdo::Error::InvalidArgs("Volume must be a number".into()));
            };
            return self.request(SourceAction::Volume { value }).await;
        }
        self.forward(Action::Volume { value }).await
    }
    #[zbus(property)]
    fn loop_status(&self) -> &str {
        match self.shown().and_then(|p| p.repeat) {
            Some(Repeat::All) => "Playlist",
            Some(Repeat::One) => "Track",
            _ => "None",
        }
    }
    #[zbus(property)]
    async fn set_loop_status(&self, value: &str) -> fdo::Result<()> {
        let mode = match value {
            "None" => Repeat::Off,
            "Playlist" => Repeat::All,
            "Track" => Repeat::One,
            _ => return Err(fdo::Error::InvalidArgs("Unknown loop mode".into())),
        };
        if self.own().is_some() {
            return self.request(SourceAction::Repeat { mode }).await;
        }
        self.forward(Action::Repeat { mode }).await
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.shown().and_then(|p| p.shuffle).unwrap_or(false)
    }
    #[zbus(property)]
    async fn set_shuffle(&self, enabled: bool) -> fdo::Result<()> {
        if self.own().is_some() {
            return self.request(SourceAction::Shuffle { enabled }).await;
        }
        self.forward(Action::Shuffle { enabled }).await
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.shown().and_then(|p| p.rate).unwrap_or(1.0)
    }
    #[zbus(property)]
    async fn set_rate(&self, rate: f64) -> fdo::Result<()> {
        match self.own() {
            Some(own) if own.speed.is_some() => {
                if !(rate.is_finite() && (0.5..=3.0).contains(&rate)) {
                    return Err(fdo::Error::InvalidArgs("Rate must be in 0.5..3.0".into()));
                }
                self.request(SourceAction::Speed { rate }).await
            }
            _ => Err(fdo::Error::NotSupported(
                "This player has no rate control".into(),
            )),
        }
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        match self.own() {
            Some(own) if own.speed.is_some() => 0.5,
            _ => self.rate(),
        }
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        match self.own() {
            Some(own) if own.speed.is_some() => 3.0,
            _ => self.rate(),
        }
    }
}

pub async fn publish(
    connection: zbus::Connection,
    mut state: watch::Receiver<Snapshot>,
) -> anyhow::Result<()> {
    let context = zbus::object_server::SignalEmitter::new(&connection, mpris::PATH)?;
    while state.changed().await.is_ok() {
        // Invalidate supported properties so MPRIS clients re-read native truth. Position is intentionally not PropertiesChanged per MPRIS.
        let invalidated = [
            "PlaybackStatus",
            "Metadata",
            "CanControl",
            "CanPlay",
            "CanPause",
            "CanSeek",
            "CanGoNext",
            "CanGoPrevious",
            "Volume",
            "LoopStatus",
            "Shuffle",
            "Rate",
            "MinimumRate",
            "MaximumRate",
        ];
        fdo::Properties::properties_changed(
            &context,
            mpris::PLAYER.try_into()?,
            HashMap::new(),
            invalidated.as_slice().into(),
        )
        .await?;
    }
    Ok(())
}

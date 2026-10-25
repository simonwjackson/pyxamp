use crate::{CommandRequest, mpris, protocol::*};
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

pub struct Player {
    pub state: watch::Receiver<Snapshot>,
    pub commands: mpsc::Sender<CommandRequest>,
}
impl Player {
    fn followed(&self) -> Option<Following> {
        match &self.state.borrow().player {
            PlayerState::Following(p) => Some(*p.clone()),
            _ => None,
        }
    }
    async fn forward(&self, action: Action) -> fdo::Result<()> {
        let target = self
            .followed()
            .ok_or_else(|| fdo::Error::NotSupported("noTarget".into()))?
            .target;
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
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn play(&self) -> fdo::Result<()> {
        self.forward(Action::Play(Empty {})).await
    }
    async fn pause(&self) -> fdo::Result<()> {
        self.forward(Action::Pause(Empty {})).await
    }
    async fn play_pause(&self) -> fdo::Result<()> {
        self.forward(Action::PlayPause(Empty {})).await
    }
    async fn stop(&self) -> fdo::Result<()> {
        self.forward(Action::Stop(Empty {})).await
    }
    async fn next(&self) -> fdo::Result<()> {
        self.forward(Action::Next(Empty {})).await
    }
    async fn previous(&self) -> fdo::Result<()> {
        self.forward(Action::Previous(Empty {})).await
    }
    async fn seek(&self, offset: i64) -> fdo::Result<()> {
        let player = self
            .followed()
            .ok_or_else(|| fdo::Error::NotSupported("noTarget".into()))?;
        let position = player
            .position_ms
            .ok_or_else(|| fdo::Error::NotSupported("No native position".into()))?;
        let position_ms = position
            .saturating_add_signed(offset / 1000)
            .min(player.track.duration_ms.unwrap_or(u64::MAX));
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
        self.forward(Action::Seek(Seek {
            position_ms: position as u64 / 1000,
            track_id: track_id.to_string(),
        }))
        .await
    }
    fn open_uri(&self, _uri: &str) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "Step 1 has no own playback".into(),
        ))
    }
    #[zbus(property)]
    fn playback_status(&self) -> &str {
        match self.followed().map(|p| p.transport) {
            Some(Transport::Playing) => "Playing",
            Some(Transport::Paused) => "Paused",
            _ => "Stopped",
        }
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<&'static str, Value<'static>> {
        mpris::metadata(self.followed().as_ref())
    }
    #[zbus(property)]
    fn position(&self) -> i64 {
        self.followed()
            .and_then(|p| p.position_ms)
            .unwrap_or(0)
            .saturating_mul(1000)
            .min(i64::MAX as u64) as i64
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        self.followed().is_some_and(|p| {
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
        self.followed().is_some_and(|p| p.capabilities.play)
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.followed().is_some_and(|p| p.capabilities.pause)
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.followed().is_some_and(|p| p.capabilities.seek)
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.followed().is_some_and(|p| p.capabilities.next)
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.followed().is_some_and(|p| p.capabilities.previous)
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.followed().and_then(|p| p.volume).unwrap_or(0.0)
    }
    #[zbus(property)]
    async fn set_volume(&self, value: f64) -> fdo::Result<()> {
        self.forward(Action::Volume { value }).await
    }
    #[zbus(property)]
    fn loop_status(&self) -> &str {
        match self.followed().and_then(|p| p.repeat) {
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
        self.forward(Action::Repeat { mode }).await
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.followed().and_then(|p| p.shuffle).unwrap_or(false)
    }
    #[zbus(property)]
    async fn set_shuffle(&self, enabled: bool) -> fdo::Result<()> {
        self.forward(Action::Shuffle { enabled }).await
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.followed().and_then(|p| p.rate).unwrap_or(1.0)
    }
    #[zbus(property)]
    async fn set_rate(&self, _rate: f64) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "Rate control is not in step 1".into(),
        ))
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        self.rate()
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        self.rate()
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

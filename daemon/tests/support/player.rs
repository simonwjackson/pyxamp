use pyxamp_daemon::{mpris, protocol::Target};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use zbus::{
    Connection, fdo,
    zvariant::{OwnedObjectPath, Value},
};

#[derive(Clone)]
pub struct PlayerConfig {
    pub status: String,
    pub track: String,
    pub duration: Option<i64>,
    pub can_control: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_seek: bool,
    pub can_next: bool,
    pub failure: bool,
    pub volume: f64,
    pub repeat: String,
    pub shuffle: bool,
    pub calls: Vec<String>,
    pub metadata_delay_ms: u64,
    pub metadata_reads: Arc<std::sync::atomic::AtomicUsize>,
}
impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            status: "Playing".into(),
            track: "/track/one".into(),
            duration: Some(60_000_000),
            can_control: true,
            can_play: true,
            can_pause: false,
            can_seek: true,
            can_next: true,
            failure: false,
            volume: 0.5,
            repeat: "None".into(),
            shuffle: false,
            calls: vec![],
            metadata_delay_ms: 0,
            metadata_reads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}
struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Session audio player"
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "session-audio"
    }
}
struct SessionPlayer {
    pub config: Arc<Mutex<PlayerConfig>>,
}
impl SessionPlayer {
    fn call(&self, name: &str) -> fdo::Result<()> {
        let mut c = self.config.lock().unwrap();
        if c.failure {
            return Err(fdo::Error::Failed("Configured native refusal".into()));
        }
        c.calls.push(name.into());
        Ok(())
    }
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl SessionPlayer {
    fn play(&self) -> fdo::Result<()> {
        self.call("Play")
    }
    fn pause(&self) -> fdo::Result<()> {
        self.call("Pause")
    }
    fn play_pause(&self) -> fdo::Result<()> {
        self.call("PlayPause")
    }
    fn stop(&self) -> fdo::Result<()> {
        self.call("Stop")
    }
    fn previous(&self) -> fdo::Result<()> {
        self.call("Previous")
    }
    fn next(&self) -> fdo::Result<()> {
        self.call("Next")
    }
    fn set_position(&self, track_id: OwnedObjectPath, position: i64) -> fdo::Result<()> {
        let c = self.config.lock().unwrap();
        if track_id.as_str() != c.track {
            return Err(fdo::Error::Failed("Track changed".into()));
        }
        drop(c);
        self.call(&format!("SetPosition:{track_id}:{position}"))
    }
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.config.lock().unwrap().status.clone()
    }
    #[zbus(property)]
    async fn metadata(&self) -> HashMap<&'static str, Value<'static>> {
        let delay = {
            let config = self.config.lock().unwrap();
            config
                .metadata_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            config.metadata_delay_ms
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let c = self.config.lock().unwrap();
        let mut m = HashMap::from([
            (
                "mpris:trackid",
                Value::from(OwnedObjectPath::try_from(c.track.clone()).unwrap()),
            ),
            ("xesam:title", Value::from("Observed title")),
            ("xesam:artist", Value::from(vec!["Artist"])),
        ]);
        if let Some(d) = c.duration {
            m.insert("mpris:length", Value::from(d));
        }
        m
    }
    #[zbus(property)]
    fn position(&self) -> i64 {
        1_000_000
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        self.config.lock().unwrap().can_control
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.config.lock().unwrap().can_play
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.config.lock().unwrap().can_pause
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.config.lock().unwrap().can_seek
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.config.lock().unwrap().can_next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.config.lock().unwrap().volume
    }
    #[zbus(property)]
    fn set_volume(&self, value: f64) {
        self.config.lock().unwrap().volume = value;
    }
    #[zbus(property)]
    fn loop_status(&self) -> String {
        self.config.lock().unwrap().repeat.clone()
    }
    #[zbus(property)]
    fn set_loop_status(&self, value: &str) {
        self.config.lock().unwrap().repeat = value.into();
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.config.lock().unwrap().shuffle
    }
    #[zbus(property)]
    fn set_shuffle(&self, value: bool) {
        self.config.lock().unwrap().shuffle = value;
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }
}
pub struct BusPlayer {
    pub connection: Connection,
    pub config: Arc<Mutex<PlayerConfig>>,
    pub target: Target,
}
impl BusPlayer {
    pub async fn start(name: &str, status: &str) -> Self {
        let bus_name = format!("org.mpris.MediaPlayer2.{name}");
        let config = Arc::new(Mutex::new(PlayerConfig {
            status: status.into(),
            ..PlayerConfig::default()
        }));
        let connection = zbus::connection::Builder::session()
            .unwrap()
            .name(bus_name.as_str())
            .unwrap()
            .serve_at(mpris::PATH, Root)
            .unwrap()
            .serve_at(
                mpris::PATH,
                SessionPlayer {
                    config: config.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let target = Target {
            bus_name,
            owner: connection.unique_name().unwrap().to_string(),
        };
        Self {
            connection,
            config,
            target,
        }
    }
    pub async fn status(&self, status: &str) {
        self.config.lock().unwrap().status = status.into();
        let properties = HashMap::from([("PlaybackStatus", Value::from(status))]);
        self.connection
            .emit_signal(
                None::<&str>,
                mpris::PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(mpris::PLAYER, properties, Vec::<String>::new()),
            )
            .await
            .unwrap();
    }
}

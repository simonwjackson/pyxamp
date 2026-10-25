//! Version 1 wire contract. Generate JSON Schema with `pyxamp-schema`.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_LINE: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub bus_name: String,
    pub owner: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum ClientMessage {
    #[serde(rename = "hello")]
    Hello { version: u32 },
    #[serde(rename = "command")]
    Command(Command),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    #[schemars(length(min = 1, max = 128))]
    pub id: String,
    pub target: Target,
    pub action: Action,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum Action {
    #[serde(rename = "transport.play")]
    Play(Empty),
    #[serde(rename = "transport.pause")]
    Pause(Empty),
    #[serde(rename = "transport.playPause")]
    PlayPause(Empty),
    #[serde(rename = "transport.stop")]
    Stop(Empty),
    #[serde(rename = "transport.previous")]
    Previous(Empty),
    #[serde(rename = "transport.next")]
    Next(Empty),
    #[serde(rename = "transport.seek")]
    Seek(Seek),
    #[serde(rename = "volume.set")]
    Volume {
        #[schemars(range(min = 0, max = 1))]
        value: f64,
    },
    #[serde(rename = "repeat.set")]
    Repeat { mode: Repeat },
    #[serde(rename = "shuffle.set")]
    Shuffle { enabled: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Seek {
    pub position_ms: u64,
    pub track_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Playing,
    Paused,
    Stopped,
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Track {
    pub title: String,
    pub artist: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub play: bool,
    pub pause: bool,
    pub stop: bool,
    pub previous: bool,
    pub next: bool,
    pub seek: bool,
    pub volume: bool,
    pub repeat: bool,
    pub shuffle: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Following {
    pub target: Target,
    pub identity: String,
    pub track: Track,
    pub transport: Transport,
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shuffle: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum PlayerState {
    #[serde(rename = "idle")]
    Idle(Empty),
    #[serde(rename = "unavailable")]
    Unavailable(Failure),
    #[serde(rename = "following")]
    Following(Box<Following>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Failure {
    pub code: String,
    pub message: String,
}
impl Failure {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", deny_unknown_fields)]
pub enum Library {
    #[serde(rename = "noSources")]
    NoSources,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    pub player: PlayerState,
    pub library: Library,
    #[schemars(length(max = 0))]
    pub queue: Vec<Empty>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            player: PlayerState::Idle(Empty {}),
            library: Library::NoSources,
            queue: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visualizer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    pub sequence: u64,
    pub status: VisualizerStatus,
    #[schemars(length(min = 75, max = 75))]
    pub columns: Vec<u8>,
    #[schemars(length(min = 75, max = 75))]
    pub waveform: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VisualizerStatus {
    Ready,
    Idle,
    Unavailable,
}
impl Visualizer {
    pub fn silent(
        target: Option<Target>,
        sequence: u64,
        status: VisualizerStatus,
        reason: Option<String>,
    ) -> Self {
        Self {
            target,
            sequence,
            status,
            columns: vec![0; 75],
            waveform: vec![128; 75],
            reason,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum Outcome {
    #[serde(rename = "accepted")]
    Accepted(Empty),
    #[serde(rename = "rejected")]
    Rejected(Failure),
}
impl From<Result<(), Failure>> for Outcome {
    fn from(result: Result<(), Failure>) -> Self {
        match result {
            Ok(()) => Self::Accepted(Empty {}),
            Err(e) => Self::Rejected(e),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum ServerMessage {
    #[serde(rename = "ready")]
    Ready {
        version: u32,
        #[serde(rename = "instanceId")]
        instance_id: String,
    },
    #[serde(rename = "state")]
    State(Snapshot),
    #[serde(rename = "visualizer")]
    Visualizer(Visualizer),
    #[serde(rename = "result")]
    Result { id: String, outcome: Outcome },
    #[serde(rename = "error")]
    Error(Failure),
}

#[derive(JsonSchema)]
#[serde(untagged)]
pub enum WireMessage {
    Client(ClientMessage),
    Server(ServerMessage),
}

pub fn schema() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(WireMessage)).expect("schema serialization")
        + "\n"
}

impl Command {
    pub fn validate(&self) -> Result<(), Failure> {
        if self.id.is_empty() || self.id.chars().count() > 128 {
            return Err(Failure::new("invalid", "id must contain 1..128 characters"));
        }
        if !self.target.bus_name.starts_with("org.mpris.MediaPlayer2.")
            || zbus::names::WellKnownName::try_from(self.target.bus_name.as_str()).is_err()
            || !self.target.owner.starts_with(':')
            || zbus::names::UniqueName::try_from(self.target.owner.as_str()).is_err()
        {
            return Err(Failure::new(
                "invalid",
                "Expected MPRIS service and unique owner",
            ));
        }
        match &self.action {
            Action::Volume { value } if !value.is_finite() || !(0.0..=1.0).contains(value) => {
                Err(Failure::new("invalid", "volume must be in 0..1"))
            }
            Action::Seek(s)
                if s.position_ms > i64::MAX as u64 / 1000
                    || zbus::zvariant::ObjectPath::try_from(s.track_id.as_str()).is_err() =>
            {
                Err(Failure::new(
                    "invalid",
                    "Invalid seek position or track path",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_boundary_validates_native_names_numbers_and_track_paths() {
        let mut command = Command {
            id: "gesture".into(),
            target: Target {
                bus_name: "org.mpris.MediaPlayer2.player".into(),
                owner: ":1.23".into(),
            },
            action: Action::Volume { value: 0.5 },
        };
        assert!(command.validate().is_ok());
        for value in [-1.0, 1.01, f64::NAN, f64::INFINITY] {
            command.action = Action::Volume { value };
            assert_eq!(command.validate().unwrap_err().code, "invalid");
        }
        command.action = Action::Seek(Seek {
            position_ms: 1,
            track_id: "/track/invalid path".into(),
        });
        assert!(command.validate().is_err());
        command.action = Action::Next(Empty {});
        command.target.owner = "org.mpris.MediaPlayer2.player".into();
        assert!(command.validate().is_err());
        command.target.owner = ":1.23".into();
        command.target.bus_name.push('\n');
        assert!(command.validate().is_err());
    }
    #[test]
    fn schema_and_serde_keep_approved_discriminants_and_object_payloads() {
        let message = ServerMessage::State(Snapshot::default());
        assert_eq!(
            serde_json::to_value(message).unwrap(),
            serde_json::json!({"_tag":"state", "payload":{"revision":0, "player":{"_tag":"idle","payload":{}}, "library":{"_tag":"noSources"}, "queue":[]}})
        );
        assert!(
            serde_json::from_value::<Action>(
                serde_json::json!({"_tag":"transport.next","payload":{"extra":true}})
            )
            .is_err()
        );
        let schema: serde_json::Value = serde_json::from_str(&super::schema()).unwrap();
        assert_eq!(
            schema["$defs"]["Visualizer"]["properties"]["columns"]["minItems"],
            75
        );
        assert_eq!(
            schema["$defs"]["Visualizer"]["properties"]["waveform"]["maxItems"],
            75
        );
    }
}

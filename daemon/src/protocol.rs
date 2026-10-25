//! Version 1 wire contract. Generate JSON Schema with `pyxamp-schema`.
//! Step 2 adds sources, own playback and requests. The face and daemon ship in
//! one package, so the additions keep version 1 (docs/seam-step2.md).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_LINE: usize = 1024 * 1024;
pub const VISUALIZER_POINTS: usize = 75;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub bus_name: String,
    pub owner: String,
}
impl Target {
    pub fn validate(&self) -> Result<(), Failure> {
        if !self.bus_name.starts_with("org.mpris.MediaPlayer2.")
            || zbus::names::WellKnownName::try_from(self.bus_name.as_str()).is_err()
            || !self.owner.starts_with(':')
            || zbus::names::UniqueName::try_from(self.owner.as_str()).is_err()
        {
            return Err(Failure::new(
                "invalid",
                "Expected MPRIS service and unique owner",
            ));
        }
        Ok(())
    }
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
    /// Source, library, own-playback and engine requests (step 2).
    #[serde(rename = "request")]
    Request(Request),
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
    /// No source is configured.
    #[serde(rename = "noSources")]
    NoSources {},
    /// One summary per configured source, in the order Pyxis, Ossicle, Jellyfin.
    #[serde(rename = "sources")]
    Sources { sources: Vec<SourceSummary> },
}

/// The three sources. Wire tags are the jump-box source tags in lower case.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum SourceId {
    Pyxis,
    Ossicle,
    Jellyfin,
}
impl SourceId {
    pub const ALL: [SourceId; 3] = [SourceId::Pyxis, SourceId::Ossicle, SourceId::Jellyfin];
    pub fn label(self) -> &'static str {
        match self {
            SourceId::Pyxis => "Pyxis",
            SourceId::Ossicle => "Ossicle",
            SourceId::Jellyfin => "Jellyfin",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum SourceStatus {
    #[serde(rename = "connecting")]
    Connecting(Empty),
    #[serde(rename = "ready")]
    Ready(Empty),
    #[serde(rename = "unavailable")]
    Unavailable(Failure),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSummary {
    pub source: SourceId,
    pub label: String,
    pub status: SourceStatus,
    /// First-level groups exactly as the source reports them, in its order.
    pub groups: Vec<Group>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
    Albums,
    Artists,
    Bookmarks,
    Stations,
    Playlists,
    Videos,
    UpNext,
    Shows,
    Books,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Group {
    /// Opaque to the face; pass it back as `library.open` node.
    pub id: String,
    pub label: String,
    pub kind: GroupKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Track,
    Album,
    Artist,
    Playlist,
    Station,
    Bookmark,
    Show,
    Episode,
    Book,
    Video,
    Group,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Progress {
    pub played: bool,
    /// Resume position of a part-played item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
}

/// One library, search or queue entry. Unknown fields stay absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    pub source: SourceId,
    /// Opaque, source-owned identity.
    pub id: String,
    pub kind: ItemKind,
    pub title: String,
    /// Artist, show or author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Child count of an album, playlist, show or group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// `library.open` with this id lists children.
    pub openable: bool,
    /// `item.play` accepts this id.
    pub playable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum QueueStatus {
    Pending,
    Current,
    Played,
    Retrying,
    Failed,
}

/// One queue occurrence. The same item can occur twice with different occurrences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueueItem {
    pub occurrence: String,
    pub item: Item,
    /// Equal keys on consecutive rows fold into one album-mode row (Ossicle: the show).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_key: Option<String>,
    pub status: QueueStatus,
}

/// A Pyxis output ("Play on"): this computer or a remote device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub id: String,
    pub name: String,
    pub local: bool,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictPosition {
    pub id: String,
    pub device: String,
    pub position_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<u64>,
}

/// An Ossicle position conflict. `conflict.resolve` keeps one position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Conflict {
    pub id: String,
    pub item: Item,
    pub positions: Vec<ConflictPosition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkipIntervals {
    pub back_s: u32,
    pub forward_s: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retry {
    /// 1 after the first failure, 2 after the second.
    pub attempt: u32,
    /// Wall-clock milliseconds since the Unix epoch.
    pub next_at_ms: u64,
}

/// Own playback paused by a foreign player.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Yielded {
    /// Wall-clock deadline for automatic resume; absent means it stays paused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_by_ms: Option<u64>,
}

/// The active source's session: Pyxamp playing here, or driving a remote output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnPlayback {
    pub source: SourceId,
    pub output: Output,
    pub track: Track,
    pub transport: Transport,
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shuffle: Option<bool>,
    /// Spoken-word speed, 0.5..3.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<SkipIntervals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yielded: Option<Yielded>,
    /// A short display note such as "SKIPPED SPONSOR".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum EngineStatus {
    #[serde(rename = "ready")]
    Ready(Empty),
    #[serde(rename = "unavailable")]
    Unavailable(Failure),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    /// The followed foreign MPRIS player (step 1 meaning, unchanged).
    pub player: PlayerState,
    pub library: Library,
    /// The active source's queue.
    pub queue: Vec<QueueItem>,
    /// The active source's session, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own: Option<OwnPlayback>,
    /// The active source's outputs (Pyxis only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<Output>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<Conflict>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineStatus>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), Failure> {
        if let PlayerState::Following(player) = &self.player {
            player.target.validate()?;
        }
        if let Some(own) = &self.own {
            if own.speed.is_some_and(|s| !(0.5..=3.0).contains(&s)) {
                return Err(Failure::new("invalid", "speed must be in 0.5..3.0"));
            }
            if own.volume.is_some_and(|v| !(0.0..=1.0).contains(&v)) {
                return Err(Failure::new("invalid", "volume must be in 0..1"));
            }
        }
        let active = self.own.as_ref().map(|own| own.source);
        if self
            .queue
            .iter()
            .any(|entry| Some(entry.item.source) != active)
        {
            return Err(Failure::new(
                "invalid",
                "queue entries must belong to the active source",
            ));
        }
        Ok(())
    }
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            player: PlayerState::Idle(Empty {}),
            library: Library::NoSources {},
            queue: vec![],
            own: None,
            outputs: vec![],
            conflicts: vec![],
            engine: None,
        }
    }
}

// ---------- step 2 requests ----------

pub const BANDS: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[schemars(length(min = 1, max = 128))]
    pub id: String,
    pub action: SourceAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlayMode {
    /// Replace the queue and play.
    Now,
    /// Insert after the current occurrence.
    Next,
    /// Append.
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum OwnTransport {
    Play,
    Pause,
    PlayPause,
    Stop,
    Previous,
    Next,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EqSettings {
    pub on: bool,
    #[schemars(range(min = -12, max = 12))]
    pub preamp_db: f64,
    #[schemars(length(min = BANDS, max = BANDS))]
    pub bands_db: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_tag", content = "payload", deny_unknown_fields)]
pub enum SourceAction {
    /// List a source's root groups (node absent) or a group's or item's children.
    #[serde(rename = "library.open", rename_all = "camelCase")]
    Open {
        source: SourceId,
        #[serde(skip_serializing_if = "Option::is_none")]
        node: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
    },
    /// Search every ready source; one merged page.
    #[serde(rename = "search")]
    Search {
        #[schemars(length(min = 1, max = 200))]
        query: String,
    },
    /// Play or enqueue. Playing from another source pauses the old one, keeping its place.
    #[serde(rename = "item.play")]
    Play {
        source: SourceId,
        item: String,
        mode: PlayMode,
    },
    #[serde(rename = "queue.jump")]
    Jump { occurrence: String },
    #[serde(rename = "queue.remove")]
    Remove { occurrence: String },
    /// Pyxis "Play on".
    #[serde(rename = "output.select")]
    SelectOutput { source: SourceId, output: String },
    /// Save the active queue as a new playlist.
    #[serde(rename = "playlist.save")]
    SavePlaylist {
        source: SourceId,
        #[schemars(length(min = 1, max = 200))]
        name: String,
    },
    #[serde(rename = "playlist.add")]
    AddToPlaylist {
        source: SourceId,
        playlist: String,
        item: String,
    },
    #[serde(rename = "station.start")]
    StartStation { source: SourceId, seed: String },
    #[serde(rename = "item.markPlayed")]
    MarkPlayed {
        source: SourceId,
        item: String,
        played: bool,
    },
    #[serde(rename = "conflict.resolve")]
    ResolveConflict {
        source: SourceId,
        conflict: String,
        keep: String,
    },
    #[serde(rename = "own.transport")]
    Transport { action: OwnTransport },
    #[serde(rename = "own.seek", rename_all = "camelCase")]
    Seek {
        position_ms: u64,
        occurrence: String,
    },
    #[serde(rename = "own.volume")]
    Volume {
        #[schemars(range(min = 0, max = 1))]
        value: f64,
    },
    #[serde(rename = "own.repeat")]
    Repeat { mode: Repeat },
    #[serde(rename = "own.shuffle")]
    Shuffle { enabled: bool },
    #[serde(rename = "own.speed")]
    Speed {
        #[schemars(range(min = 0.5, max = 3.0))]
        rate: f64,
    },
    /// EQ and balance change Pyxamp's own audio only.
    #[serde(rename = "eq.set")]
    SetEq(EqSettings),
    #[serde(rename = "balance.set")]
    Balance {
        #[schemars(range(min = -1, max = 1))]
        value: f64,
    },
}
impl SourceAction {
    /// The source an action names; `None` routes to the active source or the engine.
    pub fn source(&self) -> Option<SourceId> {
        match self {
            Self::Open { source, .. }
            | Self::Play { source, .. }
            | Self::SelectOutput { source, .. }
            | Self::SavePlaylist { source, .. }
            | Self::AddToPlaylist { source, .. }
            | Self::StartStation { source, .. }
            | Self::MarkPlayed { source, .. }
            | Self::ResolveConflict { source, .. } => Some(*source),
            _ => None,
        }
    }
}

fn bounded(text: &str, max: usize) -> bool {
    !text.is_empty() && text.chars().count() <= max
}

impl Request {
    pub fn validate(&self) -> Result<(), Failure> {
        let invalid = |message: &str| Err(Failure::new("invalid", message));
        if !bounded(&self.id, 128) {
            return invalid("id must contain 1..128 characters");
        }
        let ids_ok = match &self.action {
            SourceAction::Open { node, cursor, .. } => node
                .iter()
                .chain(cursor.iter())
                .all(|text| bounded(text, 1024)),
            SourceAction::Search { query } | SourceAction::SavePlaylist { name: query, .. } => {
                bounded(query, 200)
            }
            SourceAction::Play { item: a, .. }
            | SourceAction::Jump { occurrence: a }
            | SourceAction::Remove { occurrence: a }
            | SourceAction::SelectOutput { output: a, .. }
            | SourceAction::StartStation { seed: a, .. }
            | SourceAction::MarkPlayed { item: a, .. }
            | SourceAction::Seek { occurrence: a, .. } => bounded(a, 1024),
            SourceAction::AddToPlaylist {
                playlist: a,
                item: b,
                ..
            }
            | SourceAction::ResolveConflict {
                conflict: a,
                keep: b,
                ..
            } => bounded(a, 1024) && bounded(b, 1024),
            _ => true,
        };
        if !ids_ok {
            return invalid("text fields must be non-empty and bounded");
        }
        let in_range = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        match &self.action {
            SourceAction::Volume { value } if !in_range(*value, 0.0, 1.0) => {
                invalid("volume must be in 0..1")
            }
            SourceAction::Balance { value } if !in_range(*value, -1.0, 1.0) => {
                invalid("balance must be in -1..1")
            }
            SourceAction::Speed { rate } if !in_range(*rate, 0.5, 3.0) => {
                invalid("speed must be in 0.5..3.0")
            }
            SourceAction::Seek { position_ms, .. } if *position_ms > i64::MAX as u64 / 1000 => {
                invalid("seek position out of range")
            }
            SourceAction::SetEq(eq)
                if eq.bands_db.len() != BANDS
                    || !std::iter::once(eq.preamp_db)
                        .chain(eq.bands_db.iter().copied())
                        .all(|db| in_range(db, -12.0, 12.0)) =>
            {
                invalid("EQ needs 10 bands and preamp in -12..12 dB")
            }
            _ => Ok(()),
        }
    }
}

/// The answer to `library.open` or `search`, sent before the request's result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Page {
    /// The request id.
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visualizer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    pub sequence: u64,
    pub status: VisualizerStatus,
    #[schemars(length(min = VISUALIZER_POINTS, max = VISUALIZER_POINTS))]
    pub columns: Vec<u8>,
    #[schemars(length(min = VISUALIZER_POINTS, max = VISUALIZER_POINTS))]
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
    /// Serde enforces value/integer bounds; these checks enforce cross-field and length invariants.
    pub fn validate(&self) -> Result<(), Failure> {
        if self.columns.len() != VISUALIZER_POINTS || self.waveform.len() != VISUALIZER_POINTS {
            return Err(Failure::new(
                "invalid",
                "Visualizer arrays must contain exactly 75 values",
            ));
        }
        if let Some(target) = &self.target {
            target.validate()?;
        }
        if matches!(
            self.status,
            VisualizerStatus::Idle | VisualizerStatus::Unavailable
        ) && (self.columns.iter().any(|v| *v != 0) || self.waveform.iter().any(|v| *v != 128))
        {
            return Err(Failure::new(
                "invalid",
                "Idle/unavailable capture must have zero columns and centered waveform",
            ));
        }
        Ok(())
    }

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
            columns: vec![0; VISUALIZER_POINTS],
            waveform: vec![128; VISUALIZER_POINTS],
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

// Messages are built once per send; boxing the snapshot buys nothing here.
#[allow(clippy::large_enum_variant)]
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
    #[serde(rename = "page")]
    Page(Page),
    #[serde(rename = "error")]
    Error(Failure),
}

#[allow(clippy::large_enum_variant)]
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
        self.target.validate()?;
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

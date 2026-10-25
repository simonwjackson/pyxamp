//! The subset of the Pyxis wire (`pyxis-rpc-v3`) that Pyxamp uses.
//!
//! Canonical source: `services/pyxis/src/rpc/contract.rs` in the Pyxis repository
//! (generated: `contracts/generated/pyxis.schema.json`). These are client-side mirrors.
//! Requests are exact (the core denies unknown fields). Responses ignore fields this
//! client does not read, so an additive core change does not break Pyxamp.
use serde::{Deserialize, Serialize};

pub const CONTRACT_ID: &str = "pyxis-rpc-v3";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RpcFailure {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionTransport {
    Stopped,
    Playing,
    Paused,
    Ended,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputBinding {
    pub plugin_id: String,
    pub target_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub repeat: RepeatMode,
    pub shuffle: bool,
    #[serde(default)]
    pub queue_order: Option<Vec<u32>>,
    pub id: String,
    pub name: String,
    pub host_device_id: String,
    pub queue: Vec<String>,
    #[serde(default)]
    pub cursor: Option<u32>,
    #[serde(default)]
    pub current_track_id: Option<String>,
    pub transport: SessionTransport,
    pub position_ms: u32,
    #[serde(default)]
    pub duration_ms: Option<u32>,
    pub volume: u8,
    #[serde(default)]
    pub output: Option<OutputBinding>,
    pub reachable: bool,
    pub revision: u32,
}
impl Session {
    /// One ordinal per effective queue occurrence; absent means `0..queue.len()`.
    pub fn ordinals(&self) -> Vec<u32> {
        match &self.queue_order {
            Some(order) if order.len() == self.queue.len() => order.clone(),
            _ => (0..self.queue.len() as u32).collect(),
        }
    }
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
            .map(|cursor| cursor as usize)
            .filter(|cursor| *cursor < self.queue.len())
    }
}

/// Session commands (`RpcSessionCommand`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "_tag", content = "payload")]
pub enum Command {
    #[serde(rename = "queue.add", rename_all = "camelCase")]
    QueueAdd {
        track_ids: Vec<String>,
        /// Insert position in the effective queue. Not in d4d1b0aa: that core rejects it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
    },
    #[serde(rename = "queue.remove")]
    QueueRemove { index: u32 },
    #[serde(rename = "queue.clear")]
    QueueClear {},
    #[serde(rename = "repeat.set")]
    RepeatSet { mode: RepeatMode },
    #[serde(rename = "shuffle.set")]
    ShuffleSet { enabled: bool },
    #[serde(rename = "cursor.jump")]
    CursorJump { index: u32 },
    #[serde(rename = "transport.seek", rename_all = "camelCase")]
    Seek { position_ms: u32 },
    #[serde(rename = "transport.play")]
    Play {},
    #[serde(rename = "transport.pause")]
    Pause {},
    #[serde(rename = "transport.stop")]
    Stop {},
    #[serde(rename = "transport.trackEnded")]
    TrackEnded {},
    #[serde(rename = "position.report", rename_all = "camelCase")]
    PositionReport {
        position_ms: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u32>,
    },
    #[serde(rename = "volume.set")]
    VolumeSet { volume: u8 },
}

/// A console command routed to this host (`RpcSessionDirective`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Directive {
    #[serde(default)]
    pub expected_revision: Option<u32>,
    pub session_id: String,
    pub command: Command,
    pub issued_by: String,
    pub directive_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
    Albums,
    Bookmarks,
    Stations,
    Playlists,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryGroup {
    pub id: String,
    pub label: String,
    pub kind: GroupKind,
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub duration_ms: Option<u32>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub artwork_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAlbum {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    pub tracks: Vec<LibraryTrack>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub track_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkMember {
    pub bookmark: Bookmark,
    #[serde(default)]
    pub track: Option<LibraryTrack>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    pub external_id: String,
    pub name: String,
    pub source_plugin_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub track_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "_tag", content = "payload", rename_all = "camelCase")]
pub enum GroupMember {
    Album(LibraryAlbum),
    Bookmark(BookmarkMember),
    Station(Station),
    Playlist(Playlist),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupPage {
    pub items: Vec<GroupMember>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchTrack {
    pub id: String,
    pub external_id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u32>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    pub source_plugin_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceAlbumSummary {
    pub external_id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub year: Option<u32>,
    pub source_plugin_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceArtistSummary {
    pub external_id: String,
    pub name: String,
    pub source_plugin_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub tracks: Vec<SearchTrack>,
    pub albums: Vec<SourceAlbumSummary>,
    pub artists: Vec<SourceArtistSummary>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceAlbum {
    pub external_id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub year: Option<u32>,
    pub source_plugin_id: String,
    pub tracks: Vec<SearchTrack>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StationBatch {
    pub tracks: Vec<SearchTrack>,
    #[serde(default)]
    pub cursor: Option<String>,
    pub exhausted: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SeedKind {
    Track,
    Album,
    Artist,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StationSeed {
    pub kind: SeedKind,
    pub external_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub id: String,
    pub capabilities: Vec<String>,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputRoom {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroup {
    pub id: String,
    pub coordinator_id: String,
    pub coordinator_name: String,
    pub rooms: Vec<OutputRoom>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputTopology {
    pub plugin_id: String,
    pub groups: Vec<OutputGroup>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListenEvent {
    pub id: String,
    pub track_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    pub device_id: String,
    pub listened_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub played_ms: Option<u32>,
    pub completed: bool,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

/// The RPC requests Pyxamp sends (`RpcRequest`).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "_tag", content = "payload")]
pub enum Request {
    #[serde(rename = "system.status.get")]
    SystemStatus {},
    #[serde(rename = "plugin.list")]
    PluginList {},
    #[serde(rename = "output.targets.list", rename_all = "camelCase")]
    OutputTargets { plugin_id: String },
    #[serde(rename = "output.session.create", rename_all = "camelCase")]
    OutputSessionCreate {
        plugin_id: String,
        target_id: String,
        name: String,
    },
    #[serde(rename = "session.create")]
    SessionCreate { name: String },
    #[serde(rename = "session.list", rename_all = "camelCase")]
    SessionList { include_unreachable: bool },
    #[serde(rename = "session.state.get", rename_all = "camelCase")]
    SessionState { session_id: String },
    #[serde(rename = "session.command.run", rename_all = "camelCase")]
    CommandRun {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        command_id: Option<String>,
        command: Command,
    },
    #[serde(rename = "session.command.send", rename_all = "camelCase")]
    CommandSend {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        command_id: Option<String>,
        command: Command,
    },
    #[serde(rename = "source.search.run")]
    Search { query: String, limit: u32 },
    #[serde(rename = "library.groups.list")]
    Groups {},
    #[serde(rename = "library.group.items.list", rename_all = "camelCase")]
    GroupItems {
        group_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
        limit: u32,
    },
    #[serde(rename = "library.albums.list")]
    Albums {},
    #[serde(rename = "library.playlists.list")]
    Playlists {},
    #[serde(rename = "library.playlist.create", rename_all = "camelCase")]
    PlaylistCreate {
        title: String,
        track_ids: Vec<String>,
    },
    #[serde(rename = "library.playlist.tracks.add", rename_all = "camelCase")]
    PlaylistAdd {
        playlist_id: String,
        track_ids: Vec<String>,
        command_id: String,
    },
    #[serde(rename = "listen.events.append")]
    ListenAppend { events: Vec<ListenEvent> },
    #[serde(rename = "source.album.get", rename_all = "camelCase")]
    SourceAlbum {
        plugin_id: String,
        external_id: String,
    },
    #[serde(rename = "source.station.create", rename_all = "camelCase")]
    StationCreate {
        plugin_id: String,
        seed: StationSeed,
    },
    #[serde(rename = "source.station.next", rename_all = "camelCase")]
    StationNext {
        plugin_id: String,
        station_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Topic {
    Sessions,
    Library,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "_tag", content = "payload")]
pub enum RealtimeClient {
    #[serde(rename = "realtime.hello", rename_all = "camelCase")]
    Hello {
        contract_id: String,
        bearer_token: String,
        topics: Vec<Topic>,
        #[serde(skip_serializing_if = "Option::is_none")]
        resume_token: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Welcome {
    pub contract_id: String,
    pub resume_token: String,
    pub resumed: bool,
    pub missed_events_dropped: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "_tag", content = "payload")]
pub enum RealtimeState {
    #[serde(rename = "session.state")]
    Session(Session),
    #[serde(rename = "library.album.state")]
    Album(LibraryAlbum),
    #[serde(rename = "library.album.removed")]
    AlbumRemoved(Removal),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeEvent {
    pub resume_token: String,
    pub state: RealtimeState,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "_tag", content = "payload")]
pub enum RealtimeServer {
    #[serde(rename = "realtime.welcome")]
    Welcome(Welcome),
    #[serde(rename = "realtime.event")]
    Event(Box<RealtimeEvent>),
    #[serde(rename = "realtime.subscribed")]
    Subscribed(serde_json::Value),
    #[serde(rename = "realtime.command")]
    Command(Box<Directive>),
    #[serde(rename = "realtime.failure")]
    Failure(RpcFailure),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_commands_use_the_core_tag_payload_shape() {
        let run = Request::CommandRun {
            session_id: "s".into(),
            command_id: Some("c".into()),
            command: Command::QueueAdd {
                track_ids: vec!["t".into()],
                index: None,
            },
        };
        assert_eq!(
            serde_json::to_value(run).unwrap(),
            serde_json::json!({"_tag":"session.command.run","payload":{"sessionId":"s","commandId":"c","command":{"_tag":"queue.add","payload":{"trackIds":["t"]}}}})
        );
        assert_eq!(
            serde_json::to_value(Command::Pause {}).unwrap(),
            serde_json::json!({"_tag":"transport.pause","payload":{}})
        );
        let directive: Directive = serde_json::from_value(serde_json::json!({
            "sessionId":"s","command":{"_tag":"transport.seek","payload":{"positionMs":5}},
            "issuedBy":"d","directiveId":"x","expectedRevision":3
        }))
        .unwrap();
        assert_eq!(directive.command, Command::Seek { position_ms: 5 });
    }
}

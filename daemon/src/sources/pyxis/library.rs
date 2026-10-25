//! Library ids, the track metadata Pyxamp has seen, and Pyxis → Pyxamp item mapping.
//!
//! Pyxis session queues hold core track ids only. The core has no call that returns one
//! track's metadata, so titles come from what this client has read: library albums,
//! search results, source albums and station batches. A track nobody has listed here
//! stays unknown (empty title, no artist), never guessed.
use super::wire::{
    GroupKind as WireGroupKind, GroupMember, LibraryAlbum, LibraryGroup, SearchTrack, SeedKind,
};
use crate::protocol::{Group, GroupKind, Item, ItemKind, SourceId};
use std::collections::HashMap;

/// An opaque `library.open`, `item.play`, `playlist.add` or `station.start` id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Group(String),
    Album(String),
    Track(String),
    Playlist(String),
    Station { plugin: String, id: String },
    SourceAlbum { plugin: String, id: String },
    Artist { plugin: String, id: String },
}

fn enc(part: &str) -> String {
    url::form_urlencoded::byte_serialize(part.as_bytes()).collect()
}
fn dec(part: &str) -> Option<String> {
    if part.is_empty() {
        return None;
    }
    url::form_urlencoded::parse(part.as_bytes())
        .next()
        .map(|(key, _)| key.into_owned())
        .filter(|key| !key.is_empty())
}

impl Node {
    pub fn encode(&self) -> String {
        match self {
            Node::Group(id) => format!("group:{}", enc(id)),
            Node::Album(id) => format!("album:{}", enc(id)),
            Node::Track(id) => format!("track:{}", enc(id)),
            Node::Playlist(id) => format!("playlist:{}", enc(id)),
            Node::Station { plugin, id } => format!("station:{}/{}", enc(plugin), enc(id)),
            Node::SourceAlbum { plugin, id } => format!("salbum:{}/{}", enc(plugin), enc(id)),
            Node::Artist { plugin, id } => format!("artist:{}/{}", enc(plugin), enc(id)),
        }
    }

    pub fn parse(text: &str) -> Option<Node> {
        let (kind, rest) = text.split_once(':')?;
        let pair = || {
            let (plugin, id) = rest.split_once('/')?;
            Some((dec(plugin)?, dec(id)?))
        };
        Some(match kind {
            "group" => Node::Group(dec(rest)?),
            "album" => Node::Album(dec(rest)?),
            "track" => Node::Track(dec(rest)?),
            "playlist" => Node::Playlist(dec(rest)?),
            "station" => pair().map(|(plugin, id)| Node::Station { plugin, id })?,
            "salbum" => pair().map(|(plugin, id)| Node::SourceAlbum { plugin, id })?,
            "artist" => pair().map(|(plugin, id)| Node::Artist { plugin, id })?,
            _ => return None,
        })
    }
}

/// What Pyxamp knows about one core track.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackInfo {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Library album id, when the track belongs to a saved album.
    pub album_id: Option<String>,
    pub year: Option<u32>,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<String>,
    /// Source plugin and the source's own id: the seed for a track station.
    pub source: Option<(String, String)>,
}

#[derive(Default)]
pub struct Catalog {
    tracks: HashMap<String, TrackInfo>,
    albums: HashMap<String, LibraryAlbum>,
    /// Library albums in the core's list order.
    album_order: Vec<String>,
}

impl Catalog {
    pub fn replace_albums(&mut self, albums: Vec<LibraryAlbum>) {
        self.albums.clear();
        self.album_order.clear();
        for album in albums {
            self.put_album(album);
        }
    }

    pub fn put_album(&mut self, album: LibraryAlbum) {
        for track in &album.tracks {
            let entry = self.tracks.entry(track.id.clone()).or_default();
            entry.title = track.title.clone();
            entry.artist = Some(track.artist.clone());
            entry.album = Some(album.title.clone());
            entry.album_id = Some(album.id.clone());
            entry.year = album.year;
            entry.duration_ms = track.duration_ms.map(u64::from).or(entry.duration_ms);
            entry.artwork_url = track
                .artwork_url
                .clone()
                .or_else(|| album.artwork_url.clone());
        }
        if !self.albums.contains_key(&album.id) {
            self.album_order.push(album.id.clone());
        }
        self.albums.insert(album.id.clone(), album);
    }

    pub fn remove_album(&mut self, id: &str) {
        self.albums.remove(id);
        self.album_order.retain(|candidate| candidate != id);
        for track in self.tracks.values_mut() {
            if track.album_id.as_deref() == Some(id) {
                track.album_id = None;
            }
        }
    }

    pub fn put_search_track(&mut self, track: &SearchTrack) {
        let entry = self.tracks.entry(track.id.clone()).or_default();
        if entry.album_id.is_none() {
            entry.title = track.title.clone();
            entry.artist = Some(track.artist.clone());
            entry.album = track.album.clone();
            entry.duration_ms = track.duration_ms.map(u64::from);
            entry.artwork_url = track.artwork_url.clone();
        }
        entry.source = Some((track.source_plugin_id.clone(), track.external_id.clone()));
    }

    pub fn track(&self, id: &str) -> Option<&TrackInfo> {
        self.tracks.get(id)
    }

    pub fn album(&self, id: &str) -> Option<&LibraryAlbum> {
        self.albums.get(id)
    }

    pub fn is_library_track(&self, id: &str) -> bool {
        self.tracks
            .get(id)
            .is_some_and(|track| track.album_id.is_some())
    }

    /// Library albums whose title or artist contains `query`, case-insensitively.
    pub fn search_albums(&self, query: &str) -> Vec<&LibraryAlbum> {
        let query = query.to_lowercase();
        self.album_order
            .iter()
            .filter_map(|id| self.albums.get(id))
            .filter(|album| {
                album.title.to_lowercase().contains(&query)
                    || album.artist.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn track_item(&self, id: &str) -> Item {
        let info = self.tracks.get(id).cloned().unwrap_or_default();
        Item {
            source: SourceId::Pyxis,
            id: Node::Track(id.into()).encode(),
            kind: ItemKind::Track,
            title: info.title,
            subtitle: info.artist,
            album: info.album,
            year: info.year,
            duration_ms: info.duration_ms,
            count: None,
            openable: false,
            playable: true,
            progress: None,
        }
    }

    /// Album-mode key: the library album, else the album name the source gave.
    pub fn album_key(&self, id: &str) -> Option<String> {
        let info = self.tracks.get(id)?;
        if let Some(album) = &info.album_id {
            return Some(Node::Album(album.clone()).encode());
        }
        let album = info.album.as_ref()?;
        Some(format!(
            "name:{}/{}",
            enc(info.artist.as_deref().unwrap_or_default()),
            enc(album)
        ))
    }

    pub fn member_item(&mut self, member: GroupMember) -> Item {
        match member {
            GroupMember::Album(album) => {
                let item = album_item(&album);
                self.put_album(album);
                item
            }
            GroupMember::Bookmark(bookmark) => match bookmark.track {
                Some(track) => {
                    let entry = self.tracks.entry(track.id.clone()).or_default();
                    if entry.album_id.is_none() {
                        entry.title = track.title.clone();
                        entry.artist = Some(track.artist.clone());
                        entry.duration_ms = track.duration_ms.map(u64::from);
                    }
                    self.track_item(&track.id)
                }
                // The bookmarked track is gone from this account: listed, not playable.
                None => Item {
                    source: SourceId::Pyxis,
                    id: Node::Track(bookmark.bookmark.track_id).encode(),
                    kind: ItemKind::Bookmark,
                    title: String::new(),
                    subtitle: None,
                    album: None,
                    year: None,
                    duration_ms: None,
                    count: None,
                    openable: false,
                    playable: false,
                    progress: None,
                },
            },
            GroupMember::Station(station) => Item {
                source: SourceId::Pyxis,
                id: Node::Station {
                    plugin: station.source_plugin_id,
                    id: station.external_id,
                }
                .encode(),
                kind: ItemKind::Station,
                title: station.name,
                subtitle: None,
                album: None,
                year: None,
                duration_ms: None,
                count: None,
                openable: false,
                playable: true,
                progress: None,
            },
            GroupMember::Playlist(playlist) => Item {
                source: SourceId::Pyxis,
                id: Node::Playlist(playlist.id).encode(),
                kind: ItemKind::Playlist,
                title: playlist.title,
                subtitle: None,
                album: None,
                year: None,
                duration_ms: None,
                count: Some(playlist.track_ids.len() as u64),
                openable: true,
                playable: !playlist.track_ids.is_empty(),
                progress: None,
            },
        }
    }
}

pub fn album_item(album: &LibraryAlbum) -> Item {
    let durations: Option<u64> = album
        .tracks
        .iter()
        .map(|track| track.duration_ms.map(u64::from))
        .sum();
    Item {
        source: SourceId::Pyxis,
        id: Node::Album(album.id.clone()).encode(),
        kind: ItemKind::Album,
        title: album.title.clone(),
        subtitle: Some(album.artist.clone()),
        album: None,
        year: album.year,
        duration_ms: durations.filter(|_| !album.tracks.is_empty()),
        count: Some(album.tracks.len() as u64),
        openable: true,
        playable: !album.tracks.is_empty(),
        progress: None,
    }
}

pub fn group_kind(kind: WireGroupKind) -> GroupKind {
    match kind {
        WireGroupKind::Albums => GroupKind::Albums,
        WireGroupKind::Bookmarks => GroupKind::Bookmarks,
        WireGroupKind::Stations => GroupKind::Stations,
        WireGroupKind::Playlists => GroupKind::Playlists,
    }
}

/// First-level groups exactly as the core reports them, in its order.
pub fn groups(groups: &[LibraryGroup]) -> Vec<Group> {
    groups
        .iter()
        .map(|group| Group {
            id: Node::Group(group.id.clone()).encode(),
            label: group.label.clone(),
            kind: group_kind(group.kind),
            count: group.count.map(u64::from),
        })
        .collect()
}

pub fn group_item(group: &Group) -> Item {
    Item {
        source: SourceId::Pyxis,
        id: group.id.clone(),
        kind: ItemKind::Group,
        title: group.label.clone(),
        subtitle: None,
        album: None,
        year: None,
        duration_ms: None,
        count: group.count,
        openable: true,
        playable: false,
        progress: None,
    }
}

/// What a `station.start` seed names.
pub enum Seed {
    Existing {
        plugin: String,
        station: String,
    },
    New {
        plugin: String,
        kind: SeedKind,
        external: String,
    },
}

pub fn seed(catalog: &Catalog, text: &str) -> Result<Seed, String> {
    match Node::parse(text) {
        Some(Node::Station { plugin, id }) => Ok(Seed::Existing {
            plugin,
            station: id,
        }),
        Some(Node::Artist { plugin, id }) => Ok(Seed::New {
            plugin,
            kind: SeedKind::Artist,
            external: id,
        }),
        Some(Node::SourceAlbum { plugin, id }) => Ok(Seed::New {
            plugin,
            kind: SeedKind::Album,
            external: id,
        }),
        Some(Node::Track(id)) => match catalog.track(&id).and_then(|t| t.source.clone()) {
            Some((plugin, external)) => Ok(Seed::New {
                plugin,
                kind: SeedKind::Track,
                external,
            }),
            None => Err(
                "this track has no source reference to start a station from; \
                 start it from a search result"
                    .into(),
            ),
        },
        _ => Err("a station starts from a song, an artist, an album or a station".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_ids_round_trip_with_separators_inside_parts() {
        for node in [
            Node::Group("a/b c".into()),
            Node::Track("01J".into()),
            Node::Station {
                plugin: "pandora/x".into(),
                id: "st:1&2=3+4".into(),
            },
            Node::Artist {
                plugin: "yt".into(),
                id: "ü".into(),
            },
        ] {
            assert_eq!(Node::parse(&node.encode()), Some(node.clone()));
        }
        assert_eq!(Node::parse("track:"), None);
        assert_eq!(Node::parse("bogus:x"), None);
        assert_eq!(Node::parse("station:only"), None);
    }

    #[test]
    fn unknown_tracks_stay_unknown() {
        let catalog = Catalog::default();
        let item = catalog.track_item("t1");
        assert_eq!(item.title, "");
        assert_eq!(item.subtitle, None);
        assert_eq!(catalog.album_key("t1"), None);
    }
}

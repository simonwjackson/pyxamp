//! Library model: node and item ids, the Jellyfin query for each node, and the
//! mapping from Jellyfin items to wire items. Pure; the actor does the I/O.
use super::api::{BaseItem, Query, TICKS_PER_MS};
use crate::protocol::{Failure, Group, GroupKind, Item, ItemKind, Progress, SourceId};

pub const PAGE: u64 = 100;
/// The most tracks one `item.play` expands to.
pub const EXPAND_LIMIT: u64 = 2000;
pub const SEARCH_LIMIT: u64 = 40;

/// The first-level groups, in the brief's order.
pub const GROUPS: [(&str, &str, GroupKind); 4] = [
    ("artists", "Artists", GroupKind::Artists),
    ("albums", "Albums", GroupKind::Albums),
    ("playlists", "Playlists", GroupKind::Playlists),
    ("videos", "Videos", GroupKind::Videos),
];

/// Video libraries listed under Videos. Music and playlist views are not.
const VIDEO_COLLECTIONS: [&str; 5] = ["movies", "tvshows", "homevideos", "musicvideos", "mixed"];
const VIDEO_TYPES: [&str; 4] = ["Movie", "Episode", "Video", "MusicVideo"];
const FOLDER_TYPES: [&str; 6] = [
    "Series",
    "Season",
    "Folder",
    "BoxSet",
    "CollectionFolder",
    "UserView",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupId {
    Artists,
    Albums,
    Playlists,
    Videos,
}

/// What `library.open` lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Root,
    Group(GroupId),
    Artist(String),
    Album(String),
    Playlist(String),
    Folder(String),
}

/// What `item.play` accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Playable {
    Track(String),
    Album(String),
    Artist(String),
    Playlist(String),
    Video(String),
}

fn invalid(message: &str) -> Failure {
    Failure::new("invalid", message)
}

/// Jellyfin ids are GUIDs; anything else never reaches a URL path.
fn jellyfin_id(text: &str) -> Result<String, Failure> {
    if !text.is_empty()
        && text.len() <= 64
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        Ok(text.to_owned())
    } else {
        Err(invalid("not a Jellyfin id"))
    }
}

fn split(id: &str) -> Result<(&str, String), Failure> {
    let (kind, rest) = id
        .split_once(':')
        .ok_or_else(|| invalid("unknown Jellyfin node"))?;
    Ok((kind, jellyfin_id(rest)?))
}

impl Node {
    pub fn parse(node: Option<&str>) -> Result<Self, Failure> {
        let Some(node) = node else {
            return Ok(Self::Root);
        };
        Ok(match node {
            "artists" => Self::Group(GroupId::Artists),
            "albums" => Self::Group(GroupId::Albums),
            "playlists" => Self::Group(GroupId::Playlists),
            "videos" => Self::Group(GroupId::Videos),
            _ => match split(node)? {
                ("artist", id) => Self::Artist(id),
                ("album", id) => Self::Album(id),
                ("playlist", id) => Self::Playlist(id),
                ("folder", id) => Self::Folder(id),
                _ => return Err(invalid("not an openable Jellyfin node")),
            },
        })
    }
}

impl Playable {
    pub fn parse(item: &str) -> Result<Self, Failure> {
        Ok(match split(item)? {
            ("track", id) => Self::Track(id),
            ("album", id) => Self::Album(id),
            ("artist", id) => Self::Artist(id),
            ("playlist", id) => Self::Playlist(id),
            ("video", id) => Self::Video(id),
            _ => return Err(invalid("not a playable Jellyfin item")),
        })
    }

    /// The Jellyfin item id.
    pub fn id(&self) -> &str {
        match self {
            Self::Track(id)
            | Self::Album(id)
            | Self::Artist(id)
            | Self::Playlist(id)
            | Self::Video(id) => id,
        }
    }
}

pub fn parse_cursor(cursor: Option<&str>) -> Result<u64, Failure> {
    cursor.map_or(Ok(0), |c| {
        c.parse().map_err(|_| invalid("not a Jellyfin cursor"))
    })
}

fn items(user: &str) -> Query {
    Query::new("Items").param("userId", user)
}

fn albums(user: &str) -> Query {
    items(user)
        .param("Recursive", "true")
        .param("IncludeItemTypes", "MusicAlbum")
        .param("Fields", "ChildCount")
}

fn album_tracks(user: &str, album: &str) -> Query {
    items(user)
        .param("ParentId", album)
        .param("IncludeItemTypes", "Audio")
        .param("SortBy", "ParentIndexNumber,IndexNumber,SortName")
}

fn artist_tracks(user: &str, artist: &str) -> Query {
    items(user)
        .param("Recursive", "true")
        .param("IncludeItemTypes", "Audio")
        .param("AlbumArtistIds", artist)
        .param(
            "SortBy",
            "ProductionYear,Album,ParentIndexNumber,IndexNumber,SortName",
        )
}

fn playlist_items(user: &str, playlist: &str) -> Query {
    Query::new(format!("Playlists/{playlist}/Items")).param("userId", user)
}

/// The listing query for a node, without paging; `None` for the root.
pub fn listing(node: &Node, user: &str) -> Option<Query> {
    Some(match node {
        Node::Root => return None,
        Node::Group(GroupId::Artists) => Query::new("Artists/AlbumArtists")
            .param("userId", user)
            .param("SortBy", "SortName"),
        Node::Group(GroupId::Albums) => {
            albums(user).param("SortBy", "AlbumArtist,ProductionYear,SortName")
        }
        Node::Group(GroupId::Playlists) => items(user)
            .param("Recursive", "true")
            .param("IncludeItemTypes", "Playlist")
            .param("MediaTypes", "Audio")
            .param("SortBy", "SortName")
            .param("Fields", "ChildCount"),
        // Views do not page; `videos_from_views` filters them.
        Node::Group(GroupId::Videos) => Query::new("UserViews").param("userId", user),
        Node::Artist(id) => albums(user)
            .param("AlbumArtistIds", id)
            .param("SortBy", "ProductionYear,SortName"),
        Node::Album(id) => album_tracks(user, id),
        Node::Playlist(id) => playlist_items(user, id),
        Node::Folder(id) => items(user)
            .param("ParentId", id)
            .param("SortBy", "ParentIndexNumber,IndexNumber,SortName")
            .param("Fields", "ChildCount"),
    })
}

pub fn paged(query: Query, start: u64, limit: u64) -> Query {
    query
        .param("StartIndex", start.to_string())
        .param("Limit", limit.to_string())
}

/// The ordered tracks one playable expands to; `None` for a single track or a video.
pub fn expansion(playable: &Playable, user: &str) -> Option<Query> {
    let query = match playable {
        Playable::Track(_) | Playable::Video(_) => return None,
        Playable::Album(id) => album_tracks(user, id),
        Playable::Artist(id) => artist_tracks(user, id),
        Playable::Playlist(id) => playlist_items(user, id),
    };
    Some(paged(query, 0, EXPAND_LIMIT))
}

/// The count query for one group (`Limit=0` returns only `TotalRecordCount`).
pub fn count(group: GroupId, user: &str) -> Query {
    let query = match group {
        GroupId::Videos => items(user)
            .param("Recursive", "true")
            .param("IncludeItemTypes", VIDEO_TYPES.join(",")),
        other => listing(&Node::Group(other), user).expect("a group lists"),
    };
    query.param("Limit", "0")
}

pub fn search(query: &str, user: &str) -> Query {
    items(user)
        .param("Recursive", "true")
        .param("searchTerm", query)
        .param(
            "IncludeItemTypes",
            "MusicArtist,MusicAlbum,Audio,Playlist,Movie,Episode,Video,MusicVideo,Series",
        )
        .param("Fields", "ChildCount")
        .param("Limit", SEARCH_LIMIT.to_string())
}

pub fn groups(counts: [Option<u64>; 4]) -> Vec<Group> {
    GROUPS
        .iter()
        .zip(counts)
        .map(|((id, label, kind), count)| Group {
            id: (*id).into(),
            label: (*label).into(),
            kind: *kind,
            count,
        })
        .collect()
}

pub fn group_items(groups: &[Group]) -> Vec<Item> {
    groups
        .iter()
        .map(|g| Item {
            source: SourceId::Jellyfin,
            id: g.id.clone(),
            kind: ItemKind::Group,
            title: g.label.clone(),
            subtitle: None,
            album: None,
            year: None,
            duration_ms: None,
            count: g.count,
            openable: true,
            playable: false,
            progress: None,
        })
        .collect()
}

pub fn videos_from_views(views: &[BaseItem]) -> Vec<BaseItem> {
    views
        .iter()
        .filter(|v| {
            v.collection_type
                .as_deref()
                .is_some_and(|c| VIDEO_COLLECTIONS.contains(&c))
        })
        .cloned()
        .collect()
}

pub fn duration_ms(item: &BaseItem) -> Option<u64> {
    item.run_time_ticks
        .map(|t| t / TICKS_PER_MS)
        .filter(|ms| *ms > 0)
}

/// The track artist: the item's artists, else its album artist.
pub fn artist(item: &BaseItem) -> Option<String> {
    let artists: Vec<&str> = item
        .artists
        .iter()
        .map(String::as_str)
        .filter(|a| !a.is_empty())
        .collect();
    if artists.is_empty() {
        item.album_artist.clone().filter(|a| !a.is_empty())
    } else {
        Some(artists.join(", "))
    }
}

pub fn is_audio(item: &BaseItem) -> bool {
    item.kind.as_deref() == Some("Audio")
}

/// A wire item, or `None` for a kind Pyxamp does not list (photos, books).
pub fn to_item(item: &BaseItem) -> Option<Item> {
    let title = item.name.clone().filter(|n| !n.is_empty())?;
    let id = jellyfin_id(&item.id).ok()?;
    let kind_name = item.kind.as_deref()?;
    let (prefix, kind, openable, playable) = match kind_name {
        "MusicArtist" => ("artist", ItemKind::Artist, true, true),
        "MusicAlbum" => ("album", ItemKind::Album, true, true),
        "Playlist" => ("playlist", ItemKind::Playlist, true, true),
        "Audio" => ("track", ItemKind::Track, false, true),
        k if VIDEO_TYPES.contains(&k) => ("video", ItemKind::Video, false, true),
        k if FOLDER_TYPES.contains(&k) => ("folder", ItemKind::Group, true, false),
        _ => return None,
    };
    let subtitle = match kind {
        ItemKind::Album => item.album_artist.clone().filter(|a| !a.is_empty()),
        ItemKind::Track => artist(item),
        ItemKind::Video => item.series_name.clone().filter(|s| !s.is_empty()),
        _ => None,
    };
    let progress = (kind == ItemKind::Video)
        .then_some(item.user_data.as_ref())
        .flatten()
        .map(|data| Progress {
            played: data.played,
            position_ms: Some(data.playback_position_ticks / TICKS_PER_MS).filter(|ms| *ms > 0),
        });
    Some(Item {
        source: SourceId::Jellyfin,
        id: format!("{prefix}:{id}"),
        kind,
        title,
        subtitle,
        album: (kind == ItemKind::Track)
            .then(|| item.album.clone())
            .flatten()
            .filter(|a| !a.is_empty()),
        year: item.production_year,
        duration_ms: duration_ms(item),
        count: item.child_count.filter(|_| openable),
        openable,
        playable,
        progress,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::jellyfin::api::{ItemsPage, UserData};

    #[test]
    fn ids_round_trip_and_refuse_path_injection() {
        assert_eq!(Node::parse(None).unwrap(), Node::Root);
        assert_eq!(
            Node::parse(Some("videos")).unwrap(),
            Node::Group(GroupId::Videos)
        );
        assert_eq!(
            Node::parse(Some("album:d1bedbcf")).unwrap(),
            Node::Album("d1bedbcf".into())
        );
        for bad in [
            "album:../Users",
            "album:",
            "track:abc",
            "x",
            "album:a/b",
            "album:a?b",
        ] {
            assert!(Node::parse(Some(bad)).is_err(), "{bad}");
        }
        assert_eq!(
            Playable::parse("video:5ffe").unwrap(),
            Playable::Video("5ffe".into())
        );
        assert!(Playable::parse("folder:5ffe").is_err());
        assert_eq!(parse_cursor(Some("200")).unwrap(), 200);
        assert!(parse_cursor(Some("-1")).is_err());
    }

    #[test]
    fn items_map_from_a_recorded_answer_and_unknowns_stay_absent() {
        // Recorded from Jellyfin 10.11.6 and trimmed.
        let page: ItemsPage = serde_json::from_str(r#"{"Items":[
            {"Name":"Sines","Id":"d1bedbcfd892f97d007a2d5191362f38","Type":"MusicAlbum","AlbumArtist":"Test Tones","ProductionYear":2001,"RunTimeTicks":900000000,"ChildCount":3},
            {"Name":"Low","Id":"1833e0ae401150b0dcbaa36803a42d95","Type":"Audio","Album":"Sines","Artists":["Test Tones"],"AlbumArtist":"Test Tones","RunTimeTicks":300000000},
            {"Name":"Pattern","Id":"5ffea56d7b32f2e1bb15cab5067a9359","Type":"Video","RunTimeTicks":200230000,"UserData":{"PlaybackPositionTicks":50000000,"PlayCount":0,"Played":false}},
            {"Name":"Picture","Id":"aa","Type":"Photo"},
            {"Id":"bb","Type":"Audio"}
        ],"TotalRecordCount":5}"#).unwrap();
        let items: Vec<Item> = page.items.iter().filter_map(to_item).collect();
        assert_eq!(items.len(), 3, "photos and nameless items are not listed");
        assert_eq!(items[0].id, "album:d1bedbcfd892f97d007a2d5191362f38");
        assert_eq!(items[0].count, Some(3));
        assert_eq!(items[0].subtitle.as_deref(), Some("Test Tones"));
        assert!(items[0].openable && items[0].playable);
        assert_eq!(items[1].kind, ItemKind::Track);
        assert_eq!(items[1].duration_ms, Some(30_000));
        assert_eq!(items[1].album.as_deref(), Some("Sines"));
        assert_eq!(items[1].year, None, "unknown year stays absent");
        assert_eq!(items[2].kind, ItemKind::Video);
        assert_eq!(
            items[2].progress,
            Some(Progress {
                played: false,
                position_ms: Some(5_000)
            })
        );
        let mut played = page.items[2].clone();
        played.user_data = Some(UserData {
            played: true,
            ..Default::default()
        });
        assert_eq!(
            to_item(&played).unwrap().progress.unwrap().position_ms,
            None
        );
    }

    #[test]
    fn groups_follow_the_brief_and_videos_list_only_video_libraries() {
        let groups = groups([Some(2), Some(2), None, Some(1)]);
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["Artists", "Albums", "Playlists", "Videos"]);
        assert_eq!(groups[2].count, None);
        let views: Vec<BaseItem> = serde_json::from_str(
            r#"[
            {"Name":"Music","Id":"7e64","Type":"CollectionFolder","CollectionType":"music"},
            {"Name":"Playlists","Id":"daa9","Type":"UserView","CollectionType":"playlists"},
            {"Name":"Videos","Id":"0979","Type":"CollectionFolder","CollectionType":"homevideos"}
        ]"#,
        )
        .unwrap();
        let videos = videos_from_views(&views);
        assert_eq!(videos.len(), 1);
        assert_eq!(to_item(&videos[0]).unwrap().id, "folder:0979");
    }
}

//! Ossicle's wire (`POST /rpc`, contract `podcasts-rpc-v1`) and the books
//! bridge (`POST /comics/rpc`), hand-carried from
//! `services/ossicle/src/rpc/contract.rs` and the comics contract. Only the
//! calls and fields Pyxamp uses are here. Responses tolerate fields Pyxamp
//! does not read; every field read is required or explicitly optional.
use crate::{config::OssicleConfig, protocol::Failure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::time::Duration;

pub const CONTRACT_ID: &str = "podcasts-rpc-v1";
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Podcast {
    pub id: String,
    pub title: String,
    pub episode_count: u32,
    #[serde(default)]
    pub cover_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub episode_id: String,
    pub seconds: u32,
    pub revision: u64,
    pub device_id: String,
    pub updated_at: String,
    pub played: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: String,
    pub podcast_id: String,
    pub title: String,
    #[serde(default)]
    pub duration_seconds: Option<u32>,
    #[serde(default)]
    pub published_at: Option<String>,
    pub audio_path: String,
    /// Present only for YouTube episodes, the only ones with sponsor segments.
    #[serde(default)]
    pub video_path: Option<String>,
    #[serde(default)]
    pub position: Option<Position>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastDetail {
    pub podcast: Podcast,
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub episode: Episode,
    pub podcast_title: String,
    #[serde(default)]
    pub cover_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub skip_back_seconds: u32,
    pub skip_forward_seconds: u32,
    pub playback_speed: f64,
    pub auto_advance_queue: bool,
    pub refresh_minutes: u32,
    pub sponsor_skip: bool,
    pub continuous_playback: bool,
    pub books_upstream: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PodcastSettings {
    pub podcast_id: String,
    #[serde(default)]
    pub playback_speed: Option<f64>,
    #[serde(default)]
    pub skip_back_seconds: Option<u32>,
    #[serde(default)]
    pub skip_forward_seconds: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub settings: Settings,
    pub podcasts: Vec<PodcastSettings>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub contract_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcFailure {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEpisodes {
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionConflict {
    pub theirs: Position,
    pub yours: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "camelCase")]
pub enum PositionWriteOutcome {
    Accepted(Position),
    Conflict(PositionConflict),
    UnknownEpisode,
    Unavailable(RpcFailure),
}

/// Outcomes with one ready value; other statuses fail with their message.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "camelCase")]
enum ReadyOutcome<T> {
    Ready(T),
    Accepted(T),
    UnknownPodcast,
    UnknownEpisode,
    Rejected(RpcFailure),
    Unavailable(RpcFailure),
    Scanning,
    Disabled(RpcFailure),
    Invalid(RpcFailure),
    UnsupportedLanguage(RpcFailure),
    StaleGeneration(serde::de::IgnoredAny),
}

// ---------- books bridge ----------

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    #[serde(default)]
    pub year: Option<u32>,
    pub generation: String,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Books {
    pub publications: Vec<Book>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookLocator {
    pub generation: String,
    pub resource: String,
    pub selector: String,
    pub character_offset: u32,
    pub text_context: String,
    pub resource_progression: f32,
    pub whole_book_progression: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookPosition {
    pub book_id: String,
    pub locator: BookLocator,
    pub revision: u64,
    pub device_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookPositionConflict {
    pub theirs: BookPosition,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "camelCase")]
pub enum BookPositionRead {
    Found(BookPosition),
    #[serde(other)]
    NotFound,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "camelCase")]
pub enum BookPositionWrite {
    Accepted(BookPosition),
    Conflict(BookPositionConflict),
    #[serde(other)]
    Refused,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSegment {
    pub ordinal: u32,
    pub resource: String,
    pub selector: String,
    pub start_offset: u32,
    pub end_offset: u32,
    pub estimated_start_ms: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub estimated_duration_ms: u32,
    pub segments: Vec<TimelineSegment>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NarrationSegment {
    pub ordinal: u32,
    pub resource: String,
    pub selector: String,
    pub start_offset: u32,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NarrationWindow {
    pub audio_url: String,
    pub duration_ms: u32,
    pub starts_ms: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NarrationBatch {
    pub segments: Vec<NarrationSegment>,
    #[serde(default)]
    pub window: Option<NarrationWindow>,
}

/// Where narration starts: a text anchor in the book.
#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    pub resource: String,
    pub selector: String,
    pub character_offset: u32,
    pub text_context: String,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "_tag")]
    tag: String,
    outcome: Value,
}

/// One Ossicle origin. Ossicle has no auth today (tailnet only); a token file,
/// when configured, is sent as a bearer token.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: url::Url,
    token: Option<String>,
}

fn protocol(message: impl Into<String>) -> Failure {
    Failure::new("protocol", message)
}

impl Client {
    pub fn new(config: &OssicleConfig) -> Result<Self, Failure> {
        let mut base = url::Url::parse(&config.url)
            .map_err(|e| Failure::new("config", format!("Ossicle url: {e}")))?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(Failure::new("config", "Ossicle url must be http(s)"));
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let token = match &config.token_file {
            Some(path) => Some(
                crate::config::read_secret(path)
                    .map_err(|e| Failure::new("config", e.to_string()))?,
            ),
            None => None,
        };
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Failure::new("config", e.to_string()))?;
        Ok(Self { http, base, token })
    }

    /// An origin-relative Ossicle path as an absolute URL.
    pub fn url(&self, path: &str) -> String {
        self.base
            .join(path.trim_start_matches('/'))
            .map(String::from)
            .unwrap_or_else(|_| format!("{}{}", self.base, path.trim_start_matches('/')))
    }

    /// A comics-server path, reached through Ossicle's `/comics` bridge.
    pub fn comics_url(&self, path: &str) -> String {
        self.url(&format!("comics/{}", path.trim_start_matches('/')))
    }

    pub fn headers(&self) -> Vec<(String, String)> {
        self.token
            .iter()
            .map(|t| ("Authorization".into(), format!("Bearer {t}")))
            .collect()
    }

    async fn post(&self, path: &str, tag: &str, payload: Value) -> Result<Value, Failure> {
        let mut request = self
            .http
            .post(self.url(path))
            .json(&json!({ "_tag": tag, "payload": payload }));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|e| Failure::new("unreachable", format!("Ossicle: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let body: String = body.chars().take(200).collect();
            return Err(Failure::new(
                "unavailable",
                format!("Ossicle {tag}: HTTP {status} {body}"),
            ));
        }
        let envelope: Envelope = response
            .json()
            .await
            .map_err(|e| protocol(format!("Ossicle {tag}: {e}")))?;
        if envelope.tag != tag {
            return Err(protocol(format!(
                "Ossicle answered {} to {tag}",
                envelope.tag
            )));
        }
        Ok(envelope.outcome)
    }

    async fn typed<T: DeserializeOwned>(
        &self,
        path: &str,
        tag: &str,
        payload: Value,
    ) -> Result<T, Failure> {
        let outcome = self.post(path, tag, payload).await?;
        serde_json::from_value(outcome).map_err(|e| protocol(format!("Ossicle {tag}: {e}")))
    }

    async fn ready<T: DeserializeOwned>(
        &self,
        path: &str,
        tag: &str,
        payload: Value,
    ) -> Result<T, Failure> {
        match self.typed::<ReadyOutcome<T>>(path, tag, payload).await? {
            ReadyOutcome::Ready(value) | ReadyOutcome::Accepted(value) => Ok(value),
            ReadyOutcome::UnknownPodcast | ReadyOutcome::UnknownEpisode => {
                Err(Failure::new("notFound", format!("{tag}: unknown item")))
            }
            ReadyOutcome::Scanning => Err(Failure::new("unavailable", format!("{tag}: scanning"))),
            ReadyOutcome::StaleGeneration(_) => {
                Err(Failure::new("stale", format!("{tag}: the book changed")))
            }
            ReadyOutcome::Rejected(f)
            | ReadyOutcome::Unavailable(f)
            | ReadyOutcome::Disabled(f)
            | ReadyOutcome::Invalid(f)
            | ReadyOutcome::UnsupportedLanguage(f) => {
                Err(Failure::new("unavailable", format!("{tag}: {}", f.message)))
            }
        }
    }

    pub async fn status(&self) -> Result<SystemStatus, Failure> {
        self.ready("rpc", "system.status", json!({})).await
    }
    pub async fn podcasts(&self) -> Result<Vec<Podcast>, Failure> {
        self.ready("rpc", "library.snapshot", json!({})).await
    }
    pub async fn podcast(&self, podcast_id: &str) -> Result<PodcastDetail, Failure> {
        self.ready(
            "rpc",
            "podcast.episodes",
            json!({ "podcastId": podcast_id }),
        )
        .await
    }
    pub async fn recent_episodes(&self, limit: u32) -> Result<Vec<Episode>, Failure> {
        self.ready::<LibraryEpisodes>("rpc", "library.episodes", json!({ "limit": limit }))
            .await
            .map(|page| page.episodes)
    }
    pub async fn queue(&self) -> Result<Vec<QueueEntry>, Failure> {
        self.ready("rpc", "queue.list", json!({})).await
    }
    pub async fn write_queue(&self, episode_ids: &[String]) -> Result<Vec<QueueEntry>, Failure> {
        self.ready("rpc", "queue.write", json!({ "episodeIds": episode_ids }))
            .await
    }
    pub async fn settings(&self) -> Result<SettingsSnapshot, Failure> {
        self.ready("rpc", "settings.snapshot", json!({})).await
    }
    pub async fn write_settings(&self, settings: &Settings) -> Result<Settings, Failure> {
        let payload = serde_json::to_value(settings).map_err(|e| protocol(e.to_string()))?;
        self.ready("rpc", "settings.write", payload).await
    }
    /// Writes one show's overrides; `None` inherits the account setting.
    pub async fn write_podcast_settings(&self, show: &PodcastSettings) -> Result<(), Failure> {
        let mut payload = json!({ "podcastId": show.podcast_id });
        if let Some(v) = show.playback_speed {
            payload["playbackSpeed"] = json!(v);
        }
        if let Some(v) = show.skip_back_seconds {
            payload["skipBackSeconds"] = json!(v);
        }
        if let Some(v) = show.skip_forward_seconds {
            payload["skipForwardSeconds"] = json!(v);
        }
        self.ready::<Value>("rpc", "podcast.settings.write", payload)
            .await
            .map(|_| ())
    }
    pub async fn sponsor_segments(&self, episode_id: &str) -> Result<Vec<SponsorSegment>, Failure> {
        self.ready(
            "rpc",
            "episode.sponsorSegments",
            json!({ "episodeId": episode_id }),
        )
        .await
    }
    pub async fn positions(&self) -> Result<Vec<Position>, Failure> {
        self.ready("rpc", "position.list", json!({})).await
    }
    pub async fn write_position(
        &self,
        episode_id: &str,
        seconds: u32,
        base_revision: Option<u64>,
        device_id: &str,
        force: bool,
    ) -> Result<PositionWriteOutcome, Failure> {
        let mut payload = json!({
            "episodeId": episode_id, "seconds": seconds, "deviceId": device_id, "force": force,
        });
        if let Some(revision) = base_revision {
            payload["baseRevision"] = json!(revision);
        }
        self.typed("rpc", "position.write", payload).await
    }
    pub async fn mark_played(
        &self,
        episode_id: &str,
        played: bool,
        device_id: &str,
    ) -> Result<Position, Failure> {
        self.ready(
            "rpc",
            "position.markPlayed",
            json!({ "episodeId": episode_id, "played": played, "deviceId": device_id }),
        )
        .await
    }

    pub async fn books(&self) -> Result<Vec<Book>, Failure> {
        self.ready::<Books>("comics/rpc", "books.list", json!({}))
            .await
            .map(|books| books.publications)
    }
    pub async fn book_position(&self, book: &Book) -> Result<Option<BookPosition>, Failure> {
        let payload = json!({ "bookId": book.id, "generation": book.generation });
        match self
            .typed::<BookPositionRead>("comics/rpc", "book.position.read", payload)
            .await?
        {
            BookPositionRead::Found(position) => Ok(Some(position)),
            BookPositionRead::NotFound => Ok(None),
        }
    }
    pub async fn write_book_position(
        &self,
        book_id: &str,
        locator: &BookLocator,
        base_revision: Option<u64>,
        device_id: &str,
        force: bool,
    ) -> Result<BookPositionWrite, Failure> {
        let mut payload = serde_json::to_value(locator).map_err(|e| protocol(e.to_string()))?;
        payload["bookId"] = json!(book_id);
        payload["deviceId"] = json!(device_id);
        payload["force"] = json!(force);
        if let Some(revision) = base_revision {
            payload["baseRevision"] = json!(revision);
        }
        self.typed("comics/rpc", "book.position.write", payload)
            .await
    }
    pub async fn timeline(&self, book: &Book) -> Result<Timeline, Failure> {
        let payload = json!({ "bookId": book.id, "generation": book.generation });
        self.ready("comics/rpc", "narration.timeline", payload)
            .await
    }
    /// One batch of at most 8 sentences; `window` is absent while synthesis runs.
    pub async fn prepare(&self, book: &Book, anchor: &Anchor) -> Result<NarrationBatch, Failure> {
        let payload = json!({
            "bookId": book.id, "generation": book.generation,
            "resource": anchor.resource, "selector": anchor.selector,
            "characterOffset": anchor.character_offset, "textContext": anchor.text_context,
            "maxSegments": 8,
        });
        self.ready("comics/rpc", "narration.prepare", payload).await
    }
}

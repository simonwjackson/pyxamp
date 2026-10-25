//! Jellyfin HTTP API: authentication, item queries, playback reports, user data.
//!
//! Verified against Jellyfin 10.11.6 (the live server's release) with
//! `daemon/tools/jellyfin-test-server`:
//! - An API key belongs to no user. `GET /Users` resolves the configured user
//!   name to its id; every item query passes `userId`.
//! - `/Sessions/Playing*` accepts an API-key session, but the session has no
//!   user, so Jellyfin records no play for the user from those reports alone.
//!   The connector therefore also writes the user's data
//!   (`/UserItems/{id}/UserData?userId=`): play count, played flag, resume.
use crate::{config::JellyfinConfig, protocol::Failure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::Duration;
use url::Url;

/// Jellyfin time unit: 100 ns.
pub const TICKS_PER_MS: u64 = 10_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CLIENT: &str = "Pyxamp";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: Url,
    key: String,
    device_id: String,
    device: String,
    pub user_id: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The key never appears in logs.
        f.debug_struct("Client")
            .field("base", &self.base.as_str())
            .field("user_id", &self.user_id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserData {
    pub played: bool,
    pub playback_position_ticks: u64,
    pub play_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct BaseItem {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "Type")]
    pub kind: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_artist: Option<String>,
    pub artists: Vec<String>,
    pub series_name: Option<String>,
    pub production_year: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub child_count: Option<u64>,
    pub index_number: Option<u32>,
    pub parent_index_number: Option<u32>,
    pub collection_type: Option<String>,
    pub user_data: Option<UserData>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct ItemsPage {
    pub items: Vec<BaseItem>,
    pub total_record_count: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct User {
    id: String,
    name: String,
}

/// One `/Sessions/Playing*` body.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackReport {
    pub item_id: String,
    pub play_session_id: String,
    pub position_ticks: u64,
    pub is_paused: bool,
    pub can_seek: bool,
    pub play_method: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportKind {
    Start,
    Progress,
    Stopped,
}

/// A GET request: an API path and its query pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub path: String,
    pub params: Vec<(String, String)>,
}
impl Query {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            params: vec![],
        }
    }
    pub fn param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.push((key.into(), value.into()));
        self
    }
}

fn failure(code: &str, error: impl std::fmt::Display) -> Failure {
    Failure::new(code, format!("Jellyfin: {error}"))
}

fn http_failure(error: reqwest::Error) -> Failure {
    match error.status() {
        Some(status) if status.as_u16() == 401 || status.as_u16() == 403 => {
            failure("auth", "the API key was refused")
        }
        Some(status) => failure("http", format!("the server answered {status}")),
        // Strip the URL: reqwest errors name it, and it can carry query data.
        None => failure("unreachable", error.without_url()),
    }
}

/// A stable device id per host, so Jellyfin shows one Pyxamp device per desktop.
fn device() -> (String, String) {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_owned())
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "desktop".into());
    (format!("pyxamp-{host}"), host)
}

/// Quote a value for the MediaBrowser authorization header.
fn quoted(value: &str) -> String {
    value.replace(['"', '\\', '\r', '\n'], "")
}

impl Client {
    /// Read the key, resolve the user name to its id. Never logs the key.
    pub async fn connect(config: &JellyfinConfig) -> Result<Self, Failure> {
        let key =
            crate::config::read_secret(&config.api_key_file).map_err(|e| failure("secret", e))?;
        let mut base = Url::parse(&config.url).map_err(|e| failure("config", e))?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(failure("config", "the URL must be http or https"));
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| failure("http", e))?;
        let (device_id, device) = device();
        let mut client = Self {
            http,
            base,
            key,
            device_id,
            device,
            user_id: String::new(),
        };
        let users: Vec<User> = client.get(&Query::new("Users")).await?;
        let user = users
            .iter()
            .find(|u| u.name == config.user)
            .or_else(|| {
                users
                    .iter()
                    .find(|u| u.name.eq_ignore_ascii_case(&config.user))
            })
            .ok_or_else(|| {
                failure(
                    "noUser",
                    format!("no user named \"{}\" on the server", config.user),
                )
            })?;
        client.user_id = user.id.clone();
        Ok(client)
    }

    /// The header for API calls. Device fields name this desktop in Jellyfin.
    fn authorization(&self) -> String {
        format!(
            "MediaBrowser Token=\"{}\", Client=\"{CLIENT}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{VERSION}\"",
            quoted(&self.key),
            quoted(&self.device),
            quoted(&self.device_id),
        )
    }

    /// Headers for a stream that mpv opens. One comma-free header, because
    /// mpv's `http-header-fields` is a comma-separated list.
    pub fn stream_headers(&self) -> Vec<(String, String)> {
        vec![("X-Emby-Token".into(), self.key.clone())]
    }

    pub fn url(&self, query: &Query) -> Result<Url, Failure> {
        let mut url = self
            .base
            .join(&query.path)
            .map_err(|e| failure("config", e))?;
        if !query.params.is_empty() {
            url.query_pairs_mut().extend_pairs(&query.params);
        }
        Ok(url)
    }

    /// The original file, unchanged; the engine measures its format.
    pub fn audio_url(&self, id: &str) -> Result<String, Failure> {
        Ok(self
            .url(&Query::new(format!("Audio/{id}/stream")).param("static", "true"))?
            .into())
    }

    pub fn video_url(&self, id: &str) -> Result<String, Failure> {
        Ok(self
            .url(&Query::new(format!("Videos/{id}/stream")).param("static", "true"))?
            .into())
    }

    pub async fn get<T: DeserializeOwned>(&self, query: &Query) -> Result<T, Failure> {
        self.http
            .get(self.url(query)?)
            .header("Authorization", self.authorization())
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http_failure)?
            .json()
            .await
            .map_err(|e| failure("decode", e.without_url()))
    }

    async fn post<B: Serialize + ?Sized>(&self, query: &Query, body: &B) -> Result<(), Failure> {
        self.http
            .post(self.url(query)?)
            .header("Authorization", self.authorization())
            .json(body)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http_failure)?;
        Ok(())
    }

    async fn delete(&self, query: &Query) -> Result<(), Failure> {
        self.http
            .delete(self.url(query)?)
            .header("Authorization", self.authorization())
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http_failure)?;
        Ok(())
    }

    pub async fn item(&self, id: &str) -> Result<BaseItem, Failure> {
        self.get(&Query::new(format!("Items/{id}")).param("userId", &self.user_id))
            .await
    }

    pub async fn report(&self, kind: ReportKind, report: &PlaybackReport) -> Result<(), Failure> {
        let path = match kind {
            ReportKind::Start => "Sessions/Playing",
            ReportKind::Progress => "Sessions/Playing/Progress",
            ReportKind::Stopped => "Sessions/Playing/Stopped",
        };
        self.post(&Query::new(path), report).await
    }

    fn user_data_query(&self, id: &str) -> Query {
        Query::new(format!("UserItems/{id}/UserData")).param("userId", &self.user_id)
    }

    /// One finished play: count it, mark played, clear the resume position.
    pub async fn record_play(&self, id: &str, now_ms: u64) -> Result<(), Failure> {
        let data: UserData = self.get(&self.user_data_query(id)).await?;
        self.post(
            &self.user_data_query(id),
            &serde_json::json!({
                "PlayCount": data.play_count + 1,
                "Played": true,
                "PlaybackPositionTicks": 0,
                "LastPlayedDate": rfc3339(now_ms),
            }),
        )
        .await
    }

    /// Keep a part-played position so Jellyfin (and the next hand-off) resumes there.
    pub async fn record_position(&self, id: &str, position_ms: u64) -> Result<(), Failure> {
        self.post(
            &self.user_data_query(id),
            &serde_json::json!({ "PlaybackPositionTicks": position_ms * TICKS_PER_MS }),
        )
        .await
    }

    pub async fn mark_played(&self, id: &str, played: bool) -> Result<(), Failure> {
        let query = Query::new(format!("UserPlayedItems/{id}")).param("userId", &self.user_id);
        if played {
            self.post(&query, &serde_json::json!({})).await
        } else {
            self.delete(&query).await
        }
    }
}

/// UTC RFC 3339 from Unix milliseconds (civil-from-days, no time zone database).
pub fn rfc3339(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_is_utc() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(1_792_929_600_123), "2026-10-25T12:00:00.123Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn items_decode_from_a_recorded_answer() {
        // Recorded from Jellyfin 10.11.6 (`/Items?IncludeItemTypes=Audio`), trimmed.
        let page: ItemsPage = serde_json::from_str(
            r#"{"Items":[{"Name":"Low","Id":"1833e0ae401150b0dcbaa36803a42d95","AlbumId":"d1bedbcfd892f97d007a2d5191362f38","Album":"Sines","Artists":["Test Tones"],"AlbumArtist":"Test Tones","IndexNumber":1,"ParentIndexNumber":null,"RunTimeTicks":300000000,"ProductionYear":2001,"Type":"Audio","MediaType":"Audio","UserData":{"PlaybackPositionTicks":0,"PlayCount":0,"IsFavorite":false,"Played":false,"Key":"x","ItemId":"1833e0ae401150b0dcbaa36803a42d95"}}],"TotalRecordCount":5,"StartIndex":0}"#,
        )
        .unwrap();
        assert_eq!(page.total_record_count, Some(5));
        let item = &page.items[0];
        assert_eq!(item.kind.as_deref(), Some("Audio"));
        assert_eq!(item.run_time_ticks, Some(300_000_000));
        assert_eq!(item.parent_index_number, None);
        assert_eq!(item.user_data.as_ref().unwrap().play_count, 0);
    }

    #[test]
    fn debug_output_hides_the_key() {
        let client = Client {
            http: reqwest::Client::new(),
            base: Url::parse("http://127.0.0.1/").unwrap(),
            key: "secret-key".into(),
            device_id: "d".into(),
            device: "d".into(),
            user_id: "u".into(),
        };
        assert!(!format!("{client:?}").contains("secret-key"));
        let url = client
            .url(&Query::new("Audio/abc/stream").param("static", "true"))
            .unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1/Audio/abc/stream?static=true"
        );
        assert!(
            !url.as_str().contains("secret-key"),
            "keys stay out of URLs"
        );
    }
}

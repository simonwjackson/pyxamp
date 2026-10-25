//! HTTP RPC client for `pyxis-rpc-v3`: one POST /rpc per call, typed outcome envelope.
//!
//! Every authenticated call carries `x-pyxis-contract` and the bearer token. The token is
//! never logged; `Client`'s `Debug` omits it.
use super::wire::{CONTRACT_ID, Request, RpcFailure};
use crate::protocol::Failure;
use serde::{Deserialize, de::DeserializeOwned};
use std::time::Duration;
use url::Url;

pub const CONTRACT_HEADER: &str = "x-pyxis-contract";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: Url,
    token: String,
}
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base.as_str())
            .finish()
    }
}

/// A decoded operation outcome: `{ status, value? }`.
#[derive(Debug, Deserialize)]
pub struct Outcome {
    pub status: String,
    #[serde(default)]
    pub value: serde_json::Value,
}
impl Outcome {
    pub fn value<T: DeserializeOwned>(self) -> Result<T, Failure> {
        serde_json::from_value(self.value)
            .map_err(|e| Failure::new("pyxis.protocol", format!("unexpected Pyxis answer: {e}")))
    }
    /// The failure carried by `unavailable`, `rejected` or `invalid`, or a named status.
    pub fn failure(&self, what: &str) -> Failure {
        match serde_json::from_value::<RpcFailure>(self.value.clone()) {
            Ok(failure) => from_rpc(failure),
            Err(_) => Failure::new(
                &format!("pyxis.{}", self.status),
                format!("Pyxis refused {what}: {}", self.status),
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "_tag")]
    tag: String,
    outcome: Outcome,
}

pub fn from_rpc(failure: RpcFailure) -> Failure {
    if failure.code == "contract.mismatch" {
        return mismatch(&failure.message);
    }
    Failure::new(&failure.code, failure.message)
}

pub fn mismatch(detail: &str) -> Failure {
    Failure::new(
        "contract.mismatch",
        format!("Pyxis does not speak {CONTRACT_ID}: {detail}"),
    )
}

impl Client {
    pub fn new(base: &str, token: String) -> Result<Self, Failure> {
        let mut base = Url::parse(base)
            .map_err(|e| Failure::new("config", format!("invalid Pyxis url {base:?}: {e}")))?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(Failure::new(
                "config",
                "the Pyxis url must be http or https",
            ));
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|e| Failure::new("pyxis.http", e.to_string()))?;
        Ok(Self { http, base, token })
    }

    /// The WebSocket URL of the realtime channel.
    pub fn realtime_url(&self) -> Url {
        let mut url = self.base.join("realtime").expect("static path joins");
        let scheme = if self.base.scheme() == "https" {
            "wss"
        } else {
            "ws"
        };
        url.set_scheme(scheme).expect("ws schemes are valid");
        url
    }

    /// The stream URL of one core track; it needs [`Client::auth_header`].
    pub fn stream_url(&self, track_id: &str) -> String {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("http urls have paths")
            .pop_if_empty()
            .push("stream")
            .push(track_id);
        url.to_string()
    }

    pub fn auth_header(&self) -> (String, String) {
        ("Authorization".into(), format!("Bearer {}", self.token))
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub async fn call(&self, request: &Request) -> Result<Outcome, Failure> {
        let body = serde_json::to_value(request).expect("requests serialize");
        let tag = body["_tag"].as_str().unwrap_or_default().to_owned();
        let response = self
            .http
            .post(self.base.join("rpc").expect("static path joins"))
            .header(CONTRACT_HEADER, CONTRACT_ID)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                Failure::new(
                    "pyxis.unreachable",
                    format!("cannot reach Pyxis at {}: {}", self.base, without_url(&e)),
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(Failure::new(
                "pyxis.http",
                format!("Pyxis answered HTTP {status} to {tag}"),
            ));
        }
        let envelope: Envelope = response.json().await.map_err(|e| {
            Failure::new(
                "pyxis.protocol",
                format!(
                    "Pyxis sent an unreadable answer to {tag}: {}",
                    without_url(&e)
                ),
            )
        })?;
        if envelope.tag == "rpc.failure" {
            return Err(envelope.outcome.failure(&tag));
        }
        if envelope.tag != tag {
            return Err(Failure::new(
                "pyxis.protocol",
                format!("Pyxis answered {} to {tag}", envelope.tag),
            ));
        }
        Ok(envelope.outcome)
    }

    /// Call and require `expected` status, decoding its value.
    pub async fn ready<T: DeserializeOwned>(
        &self,
        request: &Request,
        expected: &str,
        what: &str,
    ) -> Result<T, Failure> {
        let outcome = self.call(request).await?;
        if outcome.status == expected {
            outcome.value()
        } else {
            Err(outcome.failure(what))
        }
    }

    /// `system.status.get` is public and unversioned, so it can name a mismatched core.
    pub async fn contract_id(&self) -> Result<String, Failure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Status {
            contract_id: String,
        }
        let status: Status = self
            .ready(&Request::SystemStatus {}, "ready", "the status check")
            .await
            .map_err(|failure| {
                if failure.code == "pyxis.protocol" {
                    mismatch("its status reply has no contractId")
                } else {
                    failure
                }
            })?;
        Ok(status.contract_id)
    }
}

/// reqwest errors can carry the URL; keep messages short and free of query strings.
fn without_url(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    if let Some(url) = error.url() {
        text = text.replace(url.as_str(), "");
    }
    text.trim().trim_end_matches(':').trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_join_under_the_configured_base_and_hide_the_token() {
        let client = Client::new("https://pyxis.example/base", "pyx_dev_secret".into()).unwrap();
        assert_eq!(
            client.realtime_url().as_str(),
            "wss://pyxis.example/base/realtime"
        );
        assert_eq!(
            client.stream_url("a b/c"),
            "https://pyxis.example/base/stream/a%20b%2Fc"
        );
        assert!(!format!("{client:?}").contains("secret"));
        assert!(Client::new("ftp://x", "t".into()).is_err());
    }
}

//! The realtime WebSocket: hello with `contractId`, resume tokens, reconnect.
//!
//! One task owns the socket. It reports what it hears to the actor as [`Heard`] and
//! reconnects on its own with a bounded backoff, sending the last resume token. A
//! contract or credential refusal is terminal: the task reports it and stops.
use super::client::{from_rpc, mismatch};
use super::wire::{
    CONTRACT_ID, Directive, LibraryAlbum, RealtimeClient, RealtimeServer, RealtimeState, Session,
    Topic,
};
use crate::protocol::Failure;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const PING_EVERY: Duration = Duration::from_secs(15);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum Heard {
    /// A welcome. `complete` is false when missed events may be lost; refetch through RPC.
    Connected {
        complete: bool,
    },
    Session(Box<Session>),
    Album(Box<LibraryAlbum>),
    AlbumRemoved(String),
    Directive(Box<Directive>),
    Disconnected(Failure),
    /// The core refused this client for good (contract, credential, scope).
    Refused(Failure),
}

pub fn spawn(
    url: Url,
    token: String,
    generation: u64,
    tx: mpsc::Sender<(u64, Heard)>,
) -> tokio::task::JoinHandle<()> {
    let tx = Tagged { generation, tx };
    tokio::spawn(async move {
        let mut resume: Option<String> = None;
        let mut backoff = Duration::from_secs(1);
        loop {
            let outcome = session(&url, &token, &mut resume, &tx).await;
            let failure = match outcome {
                Ok(()) => return, // the actor went away
                Err(End::Refused(failure)) => {
                    let _ = tx.send(Heard::Refused(failure)).await;
                    return;
                }
                Err(End::Dropped { failure, welcomed }) => {
                    if welcomed {
                        backoff = Duration::from_secs(1);
                    }
                    failure
                }
            };
            if tx.send(Heard::Disconnected(failure)).await.is_err() {
                return;
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    })
}

/// Stamps each report with its socket generation, so the actor drops a replaced task's tail.
struct Tagged {
    generation: u64,
    tx: mpsc::Sender<(u64, Heard)>,
}
impl Tagged {
    async fn send(&self, heard: Heard) -> Result<(), ()> {
        self.tx.send((self.generation, heard)).await.map_err(|_| ())
    }
}

enum End {
    Refused(Failure),
    Dropped { failure: Failure, welcomed: bool },
}

fn dropped(message: impl Into<String>, welcomed: bool) -> End {
    End::Dropped {
        failure: Failure::new("pyxis.realtime", message),
        welcomed,
    }
}

/// A refusal the client cannot fix by reconnecting.
fn terminal(code: &str) -> bool {
    code == "contract.mismatch"
        || code == "auth.invalidToken"
        || code == "auth.insufficientScope"
        || code == "realtime.helloRequired"
}

async fn session(
    url: &Url,
    token: &str,
    resume: &mut Option<String>,
    tx: &Tagged,
) -> Result<(), End> {
    let (socket, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .map_err(|e| dropped(format!("cannot open the Pyxis realtime socket: {e}"), false))?;
    let (mut sink, mut stream) = socket.split();
    let hello = RealtimeClient::Hello {
        contract_id: CONTRACT_ID.into(),
        bearer_token: token.into(),
        topics: vec![Topic::Sessions, Topic::Library],
        resume_token: resume.clone(),
    };
    let text = serde_json::to_string(&hello).expect("hello serializes");
    sink.send(Message::Text(text.into()))
        .await
        .map_err(|e| dropped(format!("realtime hello failed: {e}"), false))?;

    let first = tokio::time::timeout(HELLO_TIMEOUT, next_message(&mut stream))
        .await
        .map_err(|_| dropped("Pyxis did not answer the realtime hello", false))?;
    match first {
        Some(Ok(RealtimeServer::Welcome(welcome))) => {
            if welcome.contract_id != CONTRACT_ID {
                return Err(End::Refused(mismatch(&format!(
                    "its realtime welcome names {}",
                    welcome.contract_id
                ))));
            }
            *resume = Some(welcome.resume_token);
            let complete = welcome.resumed && !welcome.missed_events_dropped;
            if tx.send(Heard::Connected { complete }).await.is_err() {
                return Ok(());
            }
        }
        Some(Ok(RealtimeServer::Failure(failure))) => {
            let failure_code = failure.code.clone();
            let failure = from_rpc(failure);
            return Err(if terminal(&failure_code) {
                End::Refused(failure)
            } else {
                End::Dropped {
                    failure,
                    welcomed: false,
                }
            });
        }
        Some(Ok(_)) => return Err(dropped("Pyxis sent a frame before its welcome", false)),
        Some(Err(failure)) => {
            return Err(End::Dropped {
                failure,
                welcomed: false,
            });
        }
        None => {
            // A pre-v3 core closes or answers in a shape this client cannot read.
            return Err(dropped(
                "Pyxis closed the realtime socket after hello",
                false,
            ));
        }
    }

    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if sink.send(Message::Ping(Default::default())).await.is_err() {
                    return Err(dropped("the Pyxis realtime socket closed", true));
                }
            }
            message = next_message(&mut stream) => {
                let heard = match message {
                    None => return Err(dropped("the Pyxis realtime socket closed", true)),
                    Some(Err(failure)) => return Err(End::Dropped { failure, welcomed: true }),
                    Some(Ok(RealtimeServer::Event(event))) => {
                        *resume = Some(event.resume_token);
                        match event.state {
                            RealtimeState::Session(session) => Heard::Session(Box::new(session)),
                            RealtimeState::Album(album) => Heard::Album(Box::new(album)),
                            RealtimeState::AlbumRemoved(removal) => Heard::AlbumRemoved(removal.id),
                        }
                    }
                    Some(Ok(RealtimeServer::Command(directive))) => Heard::Directive(directive),
                    Some(Ok(RealtimeServer::Failure(failure))) => {
                        let code = failure.code.clone();
                        let failure = from_rpc(failure);
                        return Err(if terminal(&code) {
                            End::Refused(failure)
                        } else {
                            End::Dropped { failure, welcomed: true }
                        });
                    }
                    Some(Ok(RealtimeServer::Welcome(_) | RealtimeServer::Subscribed(_))) => continue,
                };
                if tx.send(heard).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

type Stream = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

/// The next protocol frame. Pings are answered by tungstenite while reading.
async fn next_message(stream: &mut Stream) -> Option<Result<RealtimeServer, Failure>> {
    loop {
        match stream.next().await? {
            Ok(Message::Text(text)) => {
                return Some(serde_json::from_str(text.as_str()).map_err(|e| {
                    Failure::new(
                        "pyxis.protocol",
                        format!("unreadable Pyxis realtime frame: {e}"),
                    )
                }));
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => continue,
        }
    }
}

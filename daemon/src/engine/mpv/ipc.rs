//! mpv JSON IPC over a private Unix socket: one line per message.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixStream, unix::OwnedWriteHalf},
    sync::{mpsc, oneshot},
};

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// mpv answers every command in order, but a stalled mpv must not hang the engine.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Ipc {
    writer: OwnedWriteHalf,
    pending: Pending,
    next_id: u64,
    reader: tokio::task::JoinHandle<()>,
}

impl Ipc {
    /// Connects and returns the client plus the ordered stream of mpv events.
    /// Wraps our end of the socket pair that mpv inherited.
    pub fn connect(
        stream: std::os::unix::net::UnixStream,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<Value>)> {
        stream.set_nonblocking(true)?;
        let stream = UnixStream::from_std(stream)?;
        let (read, writer) = stream.into_split();
        let pending: Pending = Arc::default();
        let (events_tx, events) = mpsc::unbounded_channel();
        let routes = pending.clone();
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if message.get("event").is_some() {
                    if events_tx.send(message).is_err() {
                        break;
                    }
                } else if let Some(id) = message.get("request_id").and_then(Value::as_u64) {
                    let reply = routes.lock().ok().and_then(|mut map| map.remove(&id));
                    if let Some(reply) = reply {
                        let error = message.get("error").and_then(Value::as_str).unwrap_or("");
                        let _ = reply.send(if error == "success" {
                            Ok(message.get("data").cloned().unwrap_or(Value::Null))
                        } else {
                            Err(error.to_owned())
                        });
                    }
                }
            }
            // Dropping the pending senders fails every waiting command.
            if let Ok(mut map) = routes.lock() {
                map.clear();
            }
        });
        Ok((
            Self {
                writer,
                pending,
                next_id: 1,
                reader,
            },
            events,
        ))
    }

    /// Sends one command (array or named-argument object) and waits for mpv's answer.
    pub async fn command(&mut self, command: Value) -> Result<Value, String> {
        self.command_within(command, COMMAND_TIMEOUT).await
    }

    pub async fn command_within(
        &mut self,
        command: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let (reply, response) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| "mpv IPC state poisoned".to_owned())?
            .insert(id, reply);
        let mut line = serde_json::to_vec(&json!({ "command": command, "request_id": id }))
            .map_err(|e| e.to_string())?;
        line.push(b'\n');
        if let Err(error) = self.writer.write_all(&line).await {
            self.forget(id);
            return Err(format!("mpv IPC write failed: {error}"));
        }
        match tokio::time::timeout(timeout, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("mpv closed the IPC connection".into()),
            Err(_) => {
                self.forget(id);
                Err("mpv did not answer in time".into())
            }
        }
    }

    fn forget(&self, id: u64) {
        if let Ok(mut map) = self.pending.lock() {
            map.remove(&id);
        }
    }

    pub async fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        self.command(json!(["set_property", name, value]))
            .await
            .map(drop)
    }
}

impl Drop for Ipc {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

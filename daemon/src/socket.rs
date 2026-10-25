use crate::{CommandRequest, protocol::*, session::RequestEnvelope};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Semaphore, mpsc, oneshot, watch},
};

pub fn runtime_socket() -> anyhow::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| anyhow::anyhow!("XDG_RUNTIME_DIR is required; no /tmp fallback"))?;
    let runtime = PathBuf::from(runtime);
    anyhow::ensure!(runtime.is_absolute(), "XDG_RUNTIME_DIR must be absolute");
    let meta = fs::symlink_metadata(&runtime)?;
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    anyhow::ensure!(
        meta.is_dir() && meta.uid() == uid && meta.permissions().mode() & 0o077 == 0,
        "Runtime directory must be a private, owned directory"
    );
    Ok(runtime.join("pyxamp/control.sock"))
}

pub struct PrivateListener {
    pub listener: UnixListener,
    path: PathBuf,
    inode: u64,
    _lock: File,
}
impl PrivateListener {
    /// `path` must be under the verified user runtime directory in production.
    pub fn bind(path: &Path) -> anyhow::Result<Self> {
        let dir = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Missing socket directory"))?;
        match fs::create_dir(dir) {
            Ok(()) => fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        let meta = fs::symlink_metadata(dir)?;
        // SAFETY: getuid has no preconditions.
        anyhow::ensure!(
            meta.is_dir()
                && meta.uid() == unsafe { libc::getuid() }
                && meta.permissions().mode() & 0o077 == 0,
            "Socket directory must be private and owned"
        );
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("control.lock"))?;
        // SAFETY: lock holds a valid fd for the lifetime of the listener.
        anyhow::ensure!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "Another daemon owns the runtime socket"
        );
        if path.exists() {
            anyhow::ensure!(
                fs::symlink_metadata(path)?.file_type().is_socket(),
                "Refusing to unlink a non-socket file"
            );
            anyhow::ensure!(
                std::os::unix::net::UnixStream::connect(path)
                    .is_err_and(|e| e.kind() == io::ErrorKind::ConnectionRefused),
                "Refusing to unlink a live or unknown socket"
            );
            fs::remove_file(path)?;
        }
        let listener = UnixListener::bind(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            inode: fs::symlink_metadata(path)?.ino(),
            path: path.into(),
            listener,
            _lock: lock,
        })
    }
}
impl Drop for PrivateListener {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| m.ino() == self.inode) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub async fn read_frame<R: AsyncBufRead + Unpin>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Unterminated NDJSON frame",
                ))
            };
        }
        let end = buf.iter().position(|b| *b == b'\n');
        let count = end.unwrap_or(buf.len());
        if frame.len() + count > MAX_LINE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "NDJSON line exceeds 1 MiB",
            ));
        }
        frame.extend_from_slice(&buf[..count]);
        reader.consume(count + usize::from(end.is_some()));
        if end.is_some() {
            return Ok(Some(frame));
        }
    }
}

async fn send(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    message: &ServerMessage,
) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec(message)?;
    let oversized = bytes.len() > MAX_LINE;
    if oversized {
        bytes = serde_json::to_vec(&ServerMessage::Error(Failure::new(
            "framing",
            "Outgoing NDJSON frame exceeds 1 MiB",
        )))?;
    }
    bytes.push(b'\n');
    tokio::time::timeout(Duration::from_secs(2), writer.write_all(&bytes)).await??;
    anyhow::ensure!(!oversized, "Outgoing NDJSON frame exceeds 1 MiB");
    Ok(())
}
async fn read_client(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> Result<Option<ClientMessage>, Failure> {
    let frame = read_frame(reader)
        .await
        .map_err(|e| Failure::new("framing", e.to_string()))?;
    frame
        .map(|f| serde_json::from_slice(&f).map_err(|e| Failure::new("schema", e.to_string())))
        .transpose()
}

/// How long a source request may take before the face hears "outcome unknown".
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

async fn answer_request(
    request: Request,
    requests: &mpsc::Sender<RequestEnvelope>,
) -> (Option<Page>, Outcome) {
    if let Err(e) = request.validate() {
        return (None, Outcome::Rejected(e));
    }
    let (reply, response) = oneshot::channel();
    if requests
        .try_send(RequestEnvelope { request, reply })
        .is_err()
    {
        return (
            None,
            Outcome::Rejected(Failure::new(
                "busy",
                "Request queue is full or sources unavailable",
            )),
        );
    }
    match tokio::time::timeout(REQUEST_TIMEOUT, response).await {
        Ok(Ok(Ok(page))) => (page, Outcome::Accepted(Empty {})),
        Ok(Ok(Err(e))) => (None, Outcome::Rejected(e)),
        _ => (
            None,
            Outcome::Rejected(Failure::new(
                "source",
                "Request did not complete; outcome unknown",
            )),
        ),
    }
}

pub async fn serve_client(
    stream: UnixStream,
    instance_id: String,
    mut state: watch::Receiver<Snapshot>,
    mut frames: watch::Receiver<Visualizer>,
    commands: mpsc::Sender<CommandRequest>,
    requests: mpsc::Sender<RequestEnvelope>,
) -> anyhow::Result<()> {
    // SAFETY: getuid has no preconditions.
    anyhow::ensure!(
        stream.peer_cred()?.uid() == unsafe { libc::getuid() },
        "Different-user socket peer"
    );
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let hello = tokio::time::timeout(Duration::from_secs(5), read_client(&mut reader)).await;
    match hello {
        Ok(Ok(Some(ClientMessage::Hello { version: VERSION }))) => {}
        Ok(Ok(Some(ClientMessage::Hello { .. }))) => {
            send(
                &mut writer,
                &ServerMessage::Error(Failure::new("version", "Only version 1 is supported")),
            )
            .await?;
            return Ok(());
        }
        Ok(Err(e)) => {
            send(&mut writer, &ServerMessage::Error(e)).await?;
            return Ok(());
        }
        _ => {
            send(
                &mut writer,
                &ServerMessage::Error(Failure::new("handshake", "hello version 1 must be first")),
            )
            .await?;
            return Ok(());
        }
    }
    send(
        &mut writer,
        &ServerMessage::Ready {
            version: VERSION,
            instance_id,
        },
    )
    .await?;
    let snapshot = state.borrow_and_update().clone();
    send(&mut writer, &ServerMessage::State(snapshot)).await?;
    let visualizer = frames.borrow_and_update().clone();
    send(&mut writer, &ServerMessage::Visualizer(visualizer)).await?;
    let (replies, mut results) = mpsc::channel::<ServerMessage>(32);
    let reader_task = tokio::spawn(async move {
        loop {
            match read_client(&mut reader).await {
                Ok(Some(ClientMessage::Command(command))) => {
                    let id = command.id.clone();
                    let (reply, response) = oneshot::channel();
                    let outcome = match command.validate() {
                        Err(e) => Err(e),
                        Ok(()) => match commands.try_send(CommandRequest { command, reply }) {
                            Err(_) => Err(Failure::new(
                                "busy",
                                "Command queue is full or daemon unavailable",
                            )),
                            Ok(()) => {
                                match tokio::time::timeout(Duration::from_secs(8), response).await {
                                    Ok(Ok(result)) => result,
                                    _ => Err(Failure::new(
                                        "native",
                                        "Native operation did not complete; outcome unknown",
                                    )),
                                }
                            }
                        },
                    };
                    if replies
                        .try_send(ServerMessage::Result {
                            id,
                            outcome: outcome.into(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Some(ClientMessage::Request(request))) => {
                    // Answer in the background so a slow source does not block commands.
                    let (requests, replies) = (requests.clone(), replies.clone());
                    tokio::spawn(async move {
                        let id = request.id.clone();
                        let (page, outcome) = answer_request(request, &requests).await;
                        if let Some(page) = page {
                            let _ = replies
                                .send(ServerMessage::Page(Page {
                                    id: id.clone(),
                                    ..page
                                }))
                                .await;
                        }
                        let _ = replies.send(ServerMessage::Result { id, outcome }).await;
                    });
                }
                Ok(None) => break,
                Ok(Some(ClientMessage::Hello { .. })) => {
                    let _ = replies
                        .send(ServerMessage::Error(Failure::new(
                            "handshake",
                            "hello is only allowed once",
                        )))
                        .await;
                    break;
                }
                Err(error) => {
                    let _ = replies.send(ServerMessage::Error(error)).await;
                    break;
                }
            }
        }
    });
    let frame_period = Duration::from_millis(17);
    let mut frame_clock =
        tokio::time::interval_at(tokio::time::Instant::now() + frame_period, frame_period);
    frame_clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = async {
        loop {
            tokio::select! {
                message = results.recv() => match message { Some(message) => send(&mut writer, &message).await?, None => return Ok(()) },
                update = state.changed() => { if update.is_err() { return Ok(()); } let snapshot = state.borrow_and_update().clone(); send(&mut writer, &ServerMessage::State(snapshot)).await?; },
                _ = frame_clock.tick() => {
                    match frames.has_changed() {
                        Err(_) => return Ok(()),
                        Ok(false) => {},
                        Ok(true) => { let frame = frames.borrow_and_update().clone(); send(&mut writer, &ServerMessage::Visualizer(frame)).await?; frame_clock.reset(); }
                    }
                }
            }
        }
    }.await;
    reader_task.abort();
    result
}

pub async fn run(
    listener: &PrivateListener,
    instance_id: String,
    state: watch::Receiver<Snapshot>,
    frames: watch::Receiver<Visualizer>,
    commands: mpsc::Sender<CommandRequest>,
    requests: mpsc::Sender<RequestEnvelope>,
) -> anyhow::Result<()> {
    let clients = Arc::new(Semaphore::new(64));
    loop {
        let (stream, _) = listener.listener.accept().await?;
        let Ok(permit) = clients.clone().try_acquire_owned() else {
            continue;
        };
        let (instance, state, frames, commands, requests) = (
            instance_id.clone(),
            state.clone(),
            frames.clone(),
            commands.clone(),
            requests.clone(),
        );
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) =
                serve_client(stream, instance, state, frames, commands, requests).await
            {
                tracing::debug!(%error, "Client disconnected");
            }
        });
    }
}

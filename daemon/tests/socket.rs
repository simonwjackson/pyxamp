use pyxamp_daemon::{
    protocol::*,
    socket::{self, read_frame},
};
use std::{os::unix::fs::PermissionsExt, time::Duration};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::{mpsc, watch},
};

async fn read(reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>) -> ServerMessage {
    let bytes = tokio::time::timeout(Duration::from_secs(2), read_frame(reader))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
struct SessionServer {
    _runtime: tempfile::TempDir,
    path: std::path::PathBuf,
    state: watch::Sender<Snapshot>,
    frames: watch::Sender<Visualizer>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}
impl SessionServer {
    async fn start() -> Self {
        let runtime = tempfile::tempdir().unwrap();
        let path = runtime.path().join("pyxamp/control.sock");
        let listener = socket::PrivateListener::bind(&path).unwrap();
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(socket::PrivateListener::bind(&path).is_err());
        let (state, snapshots) = watch::channel(Snapshot::default());
        let (frames_tx, frames) =
            watch::channel(Visualizer::silent(None, 0, VisualizerStatus::Idle, None));
        let (commands, mut requests) = mpsc::channel::<pyxamp_daemon::CommandRequest>(4);
        tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let _ = request
                    .reply
                    .send(Err(Failure::new("noTarget", "No player")));
            }
        });
        let task = tokio::spawn(async move {
            socket::run(&listener, "session-1".into(), snapshots, frames, commands).await
        });
        Self {
            _runtime: runtime,
            path,
            state,
            frames: frames_tx,
            task,
        }
    }
    async fn connect(
        &self,
    ) -> (
        BufReader<tokio::net::unix::OwnedReadHalf>,
        tokio::net::unix::OwnedWriteHalf,
    ) {
        let (r, w) = UnixStream::connect(&self.path).await.unwrap().into_split();
        (BufReader::new(r), w)
    }
}
impl Drop for SessionServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
const HELLO: &[u8] = b"{\"_tag\":\"hello\",\"payload\":{\"version\":1}}\n";

#[tokio::test]
async fn fragmented_handshake_multiple_lines_and_reconnect_get_fresh_state() {
    let server = SessionServer::start().await;
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(&HELLO[..9]).await.unwrap();
    writer.write_all(&HELLO[9..]).await.unwrap();
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Ready { version: 1, .. }
    ));
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::State(Snapshot { revision: 0, .. })
    ));
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Visualizer(_)
    ));
    let command = b"{\"_tag\":\"command\",\"payload\":{\"id\":\"gesture-1\",\"target\":{\"busName\":\"org.mpris.MediaPlayer2.test\",\"owner\":\":1.3\"},\"action\":{\"_tag\":\"transport.next\",\"payload\":{}}}}\n";
    let mut batched = command.to_vec();
    batched.extend_from_slice(command);
    writer.write_all(&batched).await.unwrap();
    for _ in 0..2 {
        assert!(
            matches!(read(&mut reader).await, ServerMessage::Result { outcome: Outcome::Rejected(Failure { code, .. }), .. } if code == "noTarget")
        );
    }
    drop(reader);
    drop(writer);
    server.state.send_modify(|s| s.revision = 9);
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(HELLO).await.unwrap();
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Ready { .. }
    ));
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::State(Snapshot { revision: 9, .. })
    ));
}

#[tokio::test]
async fn malformed_unsupported_and_pre_handshake_clients_are_isolated() {
    let server = SessionServer::start().await;
    for bytes in [
        b"garbage\n".as_slice(),
        b"{\"_tag\":\"hello\",\"payload\":{\"version\":2}}\n",
        b"{\"_tag\":\"command\",\"payload\":{}}\n",
    ] {
        let (mut reader, mut writer) = server.connect().await;
        writer.write_all(bytes).await.unwrap();
        assert!(matches!(read(&mut reader).await, ServerMessage::Error(_)));
        assert!(read_frame(&mut reader).await.unwrap().is_none());
    }
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(HELLO).await.unwrap();
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Ready { .. }
    ));
}

#[tokio::test]
async fn oversized_and_unterminated_lines_fail_bounded_framing() {
    let data = vec![b'x'; MAX_LINE + 1];
    let mut reader = BufReader::new(data.as_slice());
    assert_eq!(
        read_frame(&mut reader).await.unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    let mut reader = BufReader::new(b"{}".as_slice());
    assert_eq!(
        read_frame(&mut reader).await.unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    let mut reader = BufReader::new(b"one\ntwo\n".as_slice());
    assert_eq!(read_frame(&mut reader).await.unwrap().unwrap(), b"one");
    assert_eq!(read_frame(&mut reader).await.unwrap().unwrap(), b"two");
}

#[tokio::test]
async fn invalid_command_is_rejected_without_losing_subscription() {
    let server = SessionServer::start().await;
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(HELLO).await.unwrap();
    for _ in 0..3 {
        read(&mut reader).await;
    }
    writer.write_all(b"{\"_tag\":\"command\",\"payload\":{\"id\":\"\",\"target\":{\"busName\":\"org.mpris.MediaPlayer2.a\",\"owner\":\":1.2\"},\"action\":{\"_tag\":\"volume.set\",\"payload\":{\"value\":2}}}}\n").await.unwrap();
    assert!(
        matches!(read(&mut reader).await, ServerMessage::Result { outcome: Outcome::Rejected(Failure { code, .. }), .. } if code == "invalid")
    );
    server.state.send_modify(|s| s.revision = 3);
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::State(Snapshot { revision: 3, .. })
    ));
}

#[tokio::test]
async fn latest_visualizer_replaces_unsent_old_frames() {
    let server = SessionServer::start().await;
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(HELLO).await.unwrap();
    for _ in 0..3 {
        read(&mut reader).await;
    }
    // No network polling between these updates: only the latest frame may be retained.
    for sequence in 1..=10_000 {
        server.frames.send_replace(Visualizer::silent(
            None,
            sequence,
            VisualizerStatus::Idle,
            None,
        ));
    }
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Visualizer(Visualizer {
            sequence: 10_000,
            ..
        })
    ));
    server.frames.send_replace(Visualizer::silent(
        None,
        10_001,
        VisualizerStatus::Idle,
        None,
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(5), read_frame(&mut reader))
            .await
            .is_err()
    );
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Visualizer(Visualizer {
            sequence: 10_001,
            ..
        })
    ));
}

#[tokio::test]
async fn socket_binding_does_not_remove_files_or_follow_symlink_directories() {
    let runtime = tempfile::tempdir().unwrap();
    let directory = runtime.path().join("pyxamp");
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.join("control.sock");
    std::fs::write(&path, "preserve me").unwrap();
    assert!(socket::PrivateListener::bind(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve me");
    let link = runtime.path().join("link");
    std::os::unix::fs::symlink(&directory, &link).unwrap();
    assert!(socket::PrivateListener::bind(&link.join("control.sock")).is_err());
}

#[tokio::test]
async fn oversized_native_snapshot_never_writes_an_oversized_wire_line() {
    let server = SessionServer::start().await;
    server.state.send_modify(|state| {
        state.player = PlayerState::Unavailable(Failure::new("native", "x".repeat(MAX_LINE)))
    });
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(HELLO).await.unwrap();
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Ready { .. }
    ));
    assert!(
        matches!(read(&mut reader).await, ServerMessage::Error(Failure { code, .. }) if code == "framing")
    );
    assert!(read_frame(&mut reader).await.unwrap().is_none());
    server.state.send_replace(Snapshot::default());
    let (mut reader, mut writer) = server.connect().await;
    writer.write_all(HELLO).await.unwrap();
    assert!(matches!(
        read(&mut reader).await,
        ServerMessage::Ready { .. }
    ));
    assert!(matches!(read(&mut reader).await, ServerMessage::State(_)));
}

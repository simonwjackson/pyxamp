//! Video hand-off: one separate mpv window per video, never the engine.
//!
//! The window loads the mpv-mpris plugin (`$PYXAMP_MPV_MPRIS`), so it is an
//! MPRIS player whose D-Bus owner is the mpv process itself. Pyxamp follows it
//! like any foreign player and its own music yields; the visualizer finds the
//! video's audio by that PID. The connector drives the window over mpv's JSON
//! IPC socket, so the API key never appears on a command line, and reports
//! start, progress and stop to Jellyfin from the window's real position.
use super::api::{Client, PlaybackReport, ReportKind, TICKS_PER_MS};
use crate::protocol::Failure;
use serde_json::{Value, json};
use std::{
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    process::Command,
    sync::{mpsc, oneshot},
};

pub const MPRIS_ENV: &str = "PYXAMP_MPV_MPRIS";
/// The mpv binary, as for the engine; `mpv` from PATH when unset.
pub const MPV_ENV: &str = "PYXAMP_MPV";
const PROGRESS_EVERY: Duration = Duration::from_secs(10);
const CONNECT_FOR: Duration = Duration::from_secs(10);
/// Jellyfin's defaults: below 5 % keeps no resume point, from 90 % counts as played.
const RESUME_MIN: f64 = 0.05;
const PLAYED_MIN: f64 = 0.90;

/// The mpv-mpris plugin, or the reason video cannot be handed off.
pub fn mpris_script() -> Result<PathBuf, Failure> {
    let refuse = |why: String| Err(Failure::new("videoNoMpris", why));
    match std::env::var_os(MPRIS_ENV) {
        None => refuse(format!(
            "video hand-off needs the mpv MPRIS plugin; {MPRIS_ENV} is not set"
        )),
        Some(path) if Path::new(&path).is_file() => Ok(path.into()),
        Some(path) => refuse(format!(
            "video hand-off needs the mpv MPRIS plugin; {} does not exist",
            Path::new(&path).display()
        )),
    }
}

pub struct VideoRequest {
    pub id: String,
    /// Unknown stays unknown: mpv then shows its own media title.
    pub title: Option<String>,
    pub start_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VideoEvent {
    /// The window closed or finished; `position_ms` is the last known position.
    Ended {
        id: String,
        position_ms: Option<u64>,
        finished: bool,
    },
}

pub struct VideoHandle {
    pub id: String,
    commands: mpsc::Sender<Control>,
    done: tokio::task::JoinHandle<()>,
}

enum Control {
    Pause,
    Quit(oneshot::Sender<()>),
}

impl VideoHandle {
    pub async fn pause(&self) {
        let _ = self.commands.send(Control::Pause).await;
    }

    /// Close the window and wait for its stop report.
    pub async fn close(self) {
        let (tx, rx) = oneshot::channel();
        if self.commands.send(Control::Quit(tx)).await.is_ok() {
            let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(5), self.done).await;
    }
}

fn private_dir() -> Result<PathBuf, Failure> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(std::env::temp_dir);
    // Short names: a Unix socket path must fit in 108 bytes.
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let dir = base.join(format!("pyxamp-jf-{}", &tag[..12]));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .map_err(|e| Failure::new("video", format!("cannot make the video IPC dir: {e}")))?;
    Ok(dir)
}

/// Open the window and start its reporting task.
pub async fn open(
    client: Client,
    request: VideoRequest,
    events: mpsc::Sender<VideoEvent>,
) -> Result<VideoHandle, Failure> {
    let script = mpris_script()?;
    let url = client.video_url(&request.id)?;
    let mpv = std::env::var_os(MPV_ENV)
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "mpv".into());
    let mut window = spawn_window(&mpv, &script, private_dir()?).await?;
    let (commands, control) = mpsc::channel(4);
    let id = request.id.clone();
    window.start(&client, &url, &request).await?;
    let done = tokio::spawn(async move {
        let ended = window.follow(&client, &request, control).await;
        let _ = events.send(ended).await;
    });
    Ok(VideoHandle { id, commands, done })
}

/// Start mpv with its IPC socket in `dir`. On any failure the process is
/// stopped and `dir` is removed; on success the `Window` owns both.
async fn spawn_window(
    program: &std::ffi::OsStr,
    script: &Path,
    dir: PathBuf,
) -> Result<Window, Failure> {
    let socket = dir.join("ipc");
    let mut script_arg = std::ffi::OsString::from("--script=");
    script_arg.push(script);
    let mut ipc_arg = std::ffi::OsString::from("--input-ipc-server=");
    ipc_arg.push(&socket);
    // The user's config stays (key bindings), but no auto-loaded scripts: a
    // user-installed MPRIS script would give this window a second bus name.
    let spawned = Command::new(program)
        .arg("--idle=once")
        .arg("--load-scripts=no")
        .arg("--force-window=yes")
        .arg("--keep-open=no")
        .arg("--no-terminal")
        .arg(script_arg)
        .arg(ipc_arg)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn();
    let result = match spawned {
        Ok(mut child) => connect(&socket, &mut child)
            .await
            .map(|stream| (child, stream)),
        Err(e) => Err(Failure::new("video", format!("cannot start mpv: {e}"))),
    };
    match result {
        Ok((child, stream)) => Ok(Window {
            stream: BufReader::new(stream),
            child,
            dir,
            next_request: 1,
        }),
        Err(failure) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(failure)
        }
    }
}

async fn connect(socket: &Path, child: &mut tokio::process::Child) -> Result<UnixStream, Failure> {
    let deadline = tokio::time::Instant::now() + CONNECT_FOR;
    loop {
        match UnixStream::connect(socket).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(Failure::new(
                        "video",
                        format!("mpv exited before opening its IPC socket ({status})"),
                    ));
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(Failure::new(
                        "video",
                        format!("mpv did not open its IPC socket: {e}"),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

struct Window {
    stream: BufReader<UnixStream>,
    child: tokio::process::Child,
    dir: PathBuf,
    next_request: u64,
}

impl Drop for Window {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn ms(seconds: &Value) -> Option<u64> {
    seconds
        .as_f64()
        .filter(|s| s.is_finite() && *s >= 0.0)
        .map(|s| (s * 1000.0) as u64)
}

impl Window {
    async fn send(&mut self, command: Value) -> Result<(), Failure> {
        let line =
            json!({ "command": command, "request_id": self.next_request }).to_string() + "\n";
        self.next_request += 1;
        self.stream
            .get_mut()
            .write_all(line.as_bytes())
            .await
            .map_err(|e| Failure::new("video", format!("mpv IPC: {e}")))
    }

    async fn start(
        &mut self,
        client: &Client,
        url: &str,
        request: &VideoRequest,
    ) -> Result<(), Failure> {
        let headers: Vec<String> = client
            .stream_headers()
            .into_iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        self.send(json!(["set_property", "http-header-fields", headers]))
            .await?;
        if let Some(title) = &request.title {
            self.send(json!(["set_property", "force-media-title", title]))
                .await?;
        }
        if request.start_ms > 0 {
            let start = format!("{:.3}", request.start_ms as f64 / 1000.0);
            self.send(json!(["set_property", "start", start])).await?;
        }
        self.send(json!(["observe_property", 1, "time-pos"]))
            .await?;
        self.send(json!(["observe_property", 2, "pause"])).await?;
        self.send(json!(["loadfile", url, "replace"])).await
    }

    /// Follow the window until it closes; report to Jellyfin on the way.
    async fn follow(
        &mut self,
        client: &Client,
        request: &VideoRequest,
        mut control: mpsc::Receiver<Control>,
    ) -> VideoEvent {
        let mut report = PlaybackReport {
            item_id: request.id.clone(),
            play_session_id: uuid::Uuid::new_v4().simple().to_string(),
            position_ticks: request.start_ms * TICKS_PER_MS,
            is_paused: false,
            can_seek: true,
            play_method: "DirectStream",
        };
        let mut position = None;
        let mut started = false;
        let mut eof = false;
        let mut quit_waiters = vec![];
        let mut tick = tokio::time::interval(PROGRESS_EVERY);
        tick.tick().await;
        let mut line = String::new();
        loop {
            line.clear();
            tokio::select! {
                read = self.stream.read_line(&mut line) => {
                    if !matches!(read, Ok(n) if n > 0) { break; }
                    let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                    match message["event"].as_str() {
                        Some("file-loaded") if !started => {
                            started = true;
                            send_report(client, ReportKind::Start, &report).await;
                        }
                        Some("property-change") => match message["name"].as_str() {
                            Some("time-pos") => {
                                if let Some(p) = ms(&message["data"]) {
                                    position = Some(p);
                                    report.position_ticks = p * TICKS_PER_MS;
                                }
                            }
                            Some("pause") => {
                                let paused = message["data"].as_bool().unwrap_or(false);
                                if started && paused != report.is_paused {
                                    report.is_paused = paused;
                                    send_report(client, ReportKind::Progress, &report).await;
                                }
                            }
                            _ => {}
                        },
                        Some("end-file") => {
                            eof = message["reason"].as_str() == Some("eof");
                        }
                        _ => {}
                    }
                }
                Some(command) = control.recv() => match command {
                    Control::Pause => { let _ = self.send(json!(["set_property", "pause", true])).await; }
                    Control::Quit(done) => {
                        quit_waiters.push(done);
                        let _ = self.send(json!(["quit"])).await;
                    }
                },
                _ = tick.tick(), if started && !report.is_paused => {
                    send_report(client, ReportKind::Progress, &report).await;
                }
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
        let fraction = match (position, request.duration_ms) {
            (Some(p), Some(d)) if d > 0 => Some(p as f64 / d as f64),
            _ => None,
        };
        let finished = eof || fraction.is_some_and(|f| f >= PLAYED_MIN);
        if started {
            send_report(client, ReportKind::Stopped, &report).await;
            let now = now_ms();
            let result = if finished {
                client.record_play(&request.id, now).await
            } else {
                match (position, fraction) {
                    (Some(p), Some(f)) if f >= RESUME_MIN => {
                        client.record_position(&request.id, p).await
                    }
                    _ => Ok(()),
                }
            };
            if let Err(error) = result {
                tracing::warn!(code = %error.code, message = %error.message, "Jellyfin user data not saved");
            }
        }
        for waiter in quit_waiters {
            let _ = waiter.send(());
        }
        VideoEvent::Ended {
            id: request.id.clone(),
            position_ms: position,
            finished,
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub async fn send_report(client: &Client, kind: ReportKind, report: &PlaybackReport) {
    if let Err(error) = client.report(kind, report).await {
        tracing::warn!(?kind, code = %error.code, message = %error.message, "Jellyfin playback report failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_window_that_fails_to_start_leaves_no_private_dir() {
        for program in ["false", "/nonexistent/mpv"] {
            let dir = private_dir().unwrap();
            assert!(dir.is_dir());
            let failure = spawn_window(program.as_ref(), Path::new("/nonexistent.so"), dir.clone())
                .await
                .err()
                .expect("no window");
            assert_eq!(failure.code, "video");
            assert!(!dir.exists(), "{program}: the 0700 dir is removed");
        }
    }
}

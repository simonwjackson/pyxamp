//! The mpv engine: one private mpv process, driven over JSON IPC.
//!
//! The engine plays exactly what the active source loads. It never retries,
//! skips or advances on its own after a failure: it reports
//! [`EngineEventKind::Failed`] once for the failed load and stops. The source
//! decides what happens next (docs: retry rule, `own.retry`).
//!
//! mpv runs with `--no-config --load-scripts=no` and a private config
//! directory, so no user script (for example an MPRIS plugin) can make the
//! engine appear as a foreign player.
use super::{
    EngineCommand, EngineContext, EngineEvent, EngineEventKind, EngineRequest, EngineState, Media,
};
use crate::protocol::{Empty, EngineStatus, EqSettings, Failure, SourceId, Track, Transport};
use serde_json::{Value, json};
use std::{
    os::fd::AsRawFd,
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    sync::{mpsc, watch},
};

pub mod filters;
mod ipc;
use ipc::Ipc;

/// The pid of the running engine mpv, or 0. Capture and MPRIS discovery read it.
static ENGINE_PID: AtomicU32 = AtomicU32::new(0);

/// The process id of the engine's mpv while it runs.
pub fn engine_pid() -> Option<u32> {
    match ENGINE_PID.load(Ordering::Relaxed) {
        0 => None,
        pid => Some(pid),
    }
}

const RESTART_DELAY: Duration = Duration::from_secs(1);
/// How long a starting mpv may take to answer its first IPC command.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Position updates closer than this are not published, so watchers wake a few times a second.
const POSITION_STEP_MS: u64 = 200;
/// An EOF this far (or 5 % of the duration, if more) before the known end of
/// an http(s) stream counts as a dropped stream, not a finished item.
const EARLY_END_MS: u64 = 3_000;

/// How to start mpv. [`run`] uses [`Options::from_env`].
#[derive(Clone, Debug)]
pub struct Options {
    /// The mpv binary: `$PYXAMP_MPV`, else `mpv` on `PATH`.
    pub mpv: PathBuf,
    /// Extra mpv arguments, for example `--ao=pcm` in tests.
    pub extra_args: Vec<String>,
    /// Parent of the private mpv config directory.
    pub runtime_dir: PathBuf,
}
impl Options {
    pub fn from_env() -> Self {
        Self {
            mpv: std::env::var_os("PYXAMP_MPV").map_or_else(|| "mpv".into(), PathBuf::from),
            extra_args: vec![],
            runtime_dir: std::env::var_os("XDG_RUNTIME_DIR")
                .map_or_else(std::env::temp_dir, PathBuf::from),
        }
    }
}

pub async fn run(
    ctx: EngineContext,
    active: watch::Receiver<Option<SourceId>>,
) -> anyhow::Result<()> {
    run_with(ctx, active, Options::from_env()).await
}

/// Settings that outlive one load and one mpv process.
#[derive(Clone, Debug)]
struct Settings {
    eq: EqSettings,
    balance: f64,
    volume: Option<f64>,
    speed: Option<f64>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            eq: EqSettings {
                on: false,
                preamp_db: 0.0,
                bands_db: vec![0.0; crate::protocol::BANDS],
            },
            balance: 0.0,
            volume: None,
            speed: None,
        }
    }
}

pub async fn run_with(
    mut ctx: EngineContext,
    active: watch::Receiver<Option<SourceId>>,
    options: Options,
) -> anyhow::Result<()> {
    let mut settings = Settings::default();
    let mut owner = None;
    let mut failed_starts = 0u32;
    loop {
        let reason = match Process::start(&options).await {
            Ok(process) => {
                failed_starts = 0;
                let mut engine = Engine::new(process, &mut ctx, &active, settings.clone(), owner);
                let outcome = engine.serve().await;
                settings = engine.settings.clone();
                owner = engine.owner;
                engine.shutdown().await;
                match outcome {
                    Outcome::Closed => return Ok(()),
                    Outcome::Crashed(reason) => reason,
                }
            }
            Err(error) => {
                failed_starts = failed_starts.saturating_add(1);
                error.to_string()
            }
        };
        tracing::warn!(%reason, "mpv engine unavailable; restarting");
        ctx.state
            .send_replace(EngineState::unavailable(reason.clone()));
        // Answer requests while mpv is down; never queue them for a later process.
        // A missing or broken mpv backs off to one attempt every 30 s.
        let delay = RESTART_DELAY * 2u32.saturating_pow(failed_starts.min(5));
        let restart = tokio::time::sleep(delay.min(Duration::from_secs(30)));
        tokio::pin!(restart);
        loop {
            tokio::select! {
                () = &mut restart => break,
                request = ctx.requests.recv() => {
                    let Some(request) = request else { return Ok(()) };
                    let _ = request.reply.send(Err(Failure::new("engine", reason.clone())));
                }
            }
        }
    }
}

struct Process {
    child: Child,
    ipc: Ipc,
    events: mpsc::UnboundedReceiver<Value>,
    dir: PathBuf,
}
impl Process {
    async fn start(options: &Options) -> anyhow::Result<Self> {
        let dir = options
            .runtime_dir
            .join(format!("pyxamp-engine-{}", uuid::Uuid::new_v4()));
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        }
        let config = dir.join("config");
        std::fs::create_dir(&config)?;
        // mpv talks over one end of a socket pair that it inherits as fd 3
        // (`--input-ipc-client`). There is no socket file to wait for: the
        // connection exists before mpv starts, and mpv's first answer is the
        // readiness signal (see `Engine::serve`).
        let (ours, theirs) = std::os::unix::net::UnixStream::pair()?;
        let inherited = theirs.as_raw_fd();
        let mut command = Command::new(&options.mpv);
        command
            .args([
                "--idle=yes",
                "--no-config",
                "--load-scripts=no",
                "--no-video",
                "--audio-display=no",
                "--no-terminal",
                "--osc=no",
                "--ytdl=no",
                "--input-default-bindings=no",
                "--input-terminal=no",
                // Only PipeWire: the visualizer captures this stream, and mpv must
                // never fall back to raw ALSA. Without PipeWire a load fails
                // (and the source retries); mpv itself starts without it.
                "--ao=pipewire",
                "--gapless-audio=weak",
                "--prefetch-playlist=yes",
                "--keep-open=no",
                "--network-timeout=15",
                "--audio-client-name=Pyxamp",
                "--audio-pitch-correction=yes",
                "--msg-level=all=error",
                "--input-ipc-client=fd://3",
            ])
            .arg(format!("--config-dir={}", config.display()))
            .args(&options.extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // SAFETY: only async-signal-safe calls (dup2, fcntl) between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if inherited == 3 {
                    let flags = libc::fcntl(3, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(3, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                } else if libc::dup2(inherited, 3) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let spawned = command.spawn();
        drop(theirs);
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dir);
                anyhow::bail!("cannot start {}: {error}", options.mpv.display());
            }
        };
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "pyxamp::mpv", "{line}");
                }
            });
        }
        match Ipc::connect(ours) {
            Ok((ipc, events)) => Ok(Self {
                child,
                ipc,
                events,
                dir,
            }),
            Err(error) => {
                let _ = child.kill().await;
                let _ = std::fs::remove_dir_all(&dir);
                Err(error)
            }
        }
    }
}

/// One load: the token a source gave it and mpv's playlist entry.
#[derive(Clone, Debug)]
struct Entry {
    owner: SourceId,
    token: u64,
    media: Media,
    entry_id: i64,
    started: bool,
}

#[derive(Default, Debug)]
struct Props {
    pause: bool,
    idle: bool,
    time_pos_ms: Option<u64>,
    duration_ms: Option<u64>,
    samplerate: Option<f64>,
    channels: Option<u32>,
    bit_depth: Option<u32>,
    bitrate_kbps: Option<f64>,
    volume: Option<f64>,
    speed: Option<f64>,
}

enum Outcome {
    Closed,
    Crashed(String),
}

const OBSERVED: [&str; 8] = [
    "pause",
    "idle-active",
    "time-pos",
    "duration",
    "audio-params",
    "audio-bitrate",
    "volume",
    "speed",
];

struct Engine<'a> {
    process: Process,
    ctx: &'a mut EngineContext,
    active: &'a watch::Receiver<Option<SourceId>>,
    settings: Settings,
    owner: Option<SourceId>,
    current: Option<Entry>,
    next: Option<Entry>,
    /// A preload that waits until the current item has opened. If the current
    /// item fails to open, mpv then has nothing it could advance to.
    pending: Option<(SourceId, u64, Media)>,
    props: Props,
}

impl<'a> Engine<'a> {
    fn new(
        process: Process,
        ctx: &'a mut EngineContext,
        active: &'a watch::Receiver<Option<SourceId>>,
        settings: Settings,
        owner: Option<SourceId>,
    ) -> Self {
        ENGINE_PID.store(process.child.id().unwrap_or(0), Ordering::Relaxed);
        Self {
            process,
            ctx,
            active,
            settings,
            owner,
            current: None,
            next: None,
            pending: None,
            props: Props {
                idle: true,
                ..Props::default()
            },
        }
    }

    async fn shutdown(mut self) {
        ENGINE_PID.store(0, Ordering::Relaxed);
        let _ = self.process.ipc.command(json!(["quit"])).await;
        if tokio::time::timeout(Duration::from_secs(2), self.process.child.wait())
            .await
            .is_err()
        {
            let _ = self.process.child.kill().await;
        }
        let _ = std::fs::remove_dir_all(&self.process.dir);
    }

    async fn serve(&mut self) -> Outcome {
        // Readiness: mpv answers once its core and IPC run. A cold start on a
        // loaded machine can be slow, so the bound is generous; a dead mpv is
        // seen at once because its end of the socket closes.
        let hello = self
            .process
            .ipc
            .command_within(json!(["get_property", "mpv-version"]), STARTUP_TIMEOUT)
            .await;
        if let Err(error) = hello {
            return Outcome::Crashed(format!("mpv did not start: {error}"));
        }
        for (id, name) in OBSERVED.iter().enumerate() {
            if let Err(error) = self
                .process
                .ipc
                .command(json!(["observe_property", id + 1, name]))
                .await
            {
                return Outcome::Crashed(format!("mpv refused to report {name}: {error}"));
            }
        }
        // mpv sends each observed property's initial value once. Take them all
        // before serving, so a stale initial value never overwrites a later command.
        let mut initial = std::collections::BTreeSet::new();
        let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
        while initial.len() < OBSERVED.len() {
            match tokio::time::timeout_at(deadline, self.process.events.recv()).await {
                Ok(Some(event)) => {
                    if event.get("event").and_then(Value::as_str) == Some("property-change")
                        && let Some(id) = event.get("id").and_then(Value::as_u64)
                    {
                        initial.insert(id);
                    }
                    self.event(event).await;
                }
                Ok(None) => return Outcome::Crashed("mpv closed its IPC connection".into()),
                Err(_) => return Outcome::Crashed("mpv did not report its initial state".into()),
            }
        }
        if let Err(failure) = self.apply_settings().await {
            return Outcome::Crashed(failure.message);
        }
        self.publish();
        loop {
            tokio::select! {
                request = self.ctx.requests.recv() => {
                    let Some(request) = request else { return Outcome::Closed };
                    self.request(request).await;
                }
                event = self.process.events.recv() => {
                    let Some(event) = event else {
                        return Outcome::Crashed(self.crashed("mpv closed its IPC connection").await);
                    };
                    self.event(event).await;
                }
                status = self.process.child.wait() => {
                    let message = match status {
                        Ok(status) => format!("mpv exited: {status}"),
                        Err(error) => format!("mpv wait failed: {error}"),
                    };
                    return Outcome::Crashed(self.crashed(&message).await);
                }
            }
        }
    }

    /// The current load fails once when mpv dies under it.
    async fn crashed(&mut self, message: &str) -> String {
        if let Some(entry) = self.current.take() {
            self.emit(
                &entry,
                EngineEventKind::Failed(Failure::new("engine", message)),
            );
        }
        self.next = None;
        message.to_owned()
    }

    async fn apply_settings(&mut self) -> Result<(), Failure> {
        let graph = filters::graph(&self.settings.eq, self.settings.balance);
        self.set("af", json!(graph)).await?;
        if let Some(volume) = self.settings.volume {
            self.set("volume", json!(volume * 100.0)).await?;
        }
        if let Some(speed) = self.settings.speed {
            self.set("speed", json!(speed)).await?;
        }
        Ok(())
    }

    async fn set(&mut self, name: &str, value: Value) -> Result<(), Failure> {
        self.process
            .ipc
            .set(name, value)
            .await
            .map_err(|e| Failure::new("engine", format!("mpv {name}: {e}")))
    }

    async fn command(&mut self, command: Value) -> Result<Value, Failure> {
        self.process
            .ipc
            .command(command)
            .await
            .map_err(|e| Failure::new("engine", format!("mpv: {e}")))
    }

    async fn request(&mut self, request: EngineRequest) {
        let EngineRequest {
            owner,
            command,
            reply,
        } = request;
        let result = match owner {
            Some(source) if *self.active.borrow() != Some(source) => Err(Failure::new(
                "inactive",
                format!("{} is not the active source", source.label()),
            )),
            _ => self.execute(owner, command).await,
        };
        // Publish first, so a caller that sees the reply also sees the new state.
        self.publish();
        let _ = reply.send(result);
    }

    async fn execute(
        &mut self,
        owner: Option<SourceId>,
        command: EngineCommand,
    ) -> Result<(), Failure> {
        match command {
            EngineCommand::Load {
                token,
                media,
                start_ms,
                paused,
            } => {
                let source =
                    owner.ok_or_else(|| Failure::new("invalid", "only a source can load media"))?;
                validate_media(&media)?;
                self.set("pause", json!(paused)).await?;
                self.props.pause = paused;
                self.set("http-header-fields", headers(&media)).await?;
                let options = format!("start={:.3}", start_ms as f64 / 1000.0);
                let data = self
                    .command(json!(["loadfile", media.url, "replace", -1, options]))
                    .await?;
                let entry_id = entry_id(&data)?;
                self.owner = Some(source);
                self.next = None;
                self.pending = None;
                self.current = Some(Entry {
                    owner: source,
                    token,
                    media,
                    entry_id,
                    started: false,
                });
                self.props.time_pos_ms = Some(start_ms);
                self.props.duration_ms = None;
                self.props.idle = false;
                self.clear_measured();
                Ok(())
            }
            EngineCommand::Preload { token, media } => {
                let source =
                    owner.ok_or_else(|| Failure::new("invalid", "only a source can preload"))?;
                let current = self
                    .current
                    .as_ref()
                    .ok_or_else(|| Failure::new("idle", "nothing is loaded to preload after"))?;
                if current.owner != source {
                    return Err(Failure::new(
                        "inactive",
                        "another source owns the current item",
                    ));
                }
                let opened = current.started;
                if let Some(media) = &media {
                    validate_media(media)?;
                }
                self.command(json!(["playlist-clear"])).await?;
                self.next = None;
                self.pending = None;
                let Some(media) = media else { return Ok(()) };
                if opened {
                    self.append(source, token, media).await
                } else {
                    self.pending = Some((source, token, media));
                    Ok(())
                }
            }
            EngineCommand::Play => {
                if self.current.is_none() {
                    return Err(Failure::new("idle", "nothing is loaded"));
                }
                self.set("pause", json!(false)).await?;
                self.props.pause = false;
                Ok(())
            }
            EngineCommand::Pause => {
                if self.current.is_some() {
                    self.set("pause", json!(true)).await?;
                    self.props.pause = true;
                }
                Ok(())
            }
            EngineCommand::Stop => {
                self.command(json!(["stop"])).await?;
                self.current = None;
                self.next = None;
                self.pending = None;
                self.props.idle = true;
                self.props.time_pos_ms = None;
                Ok(())
            }
            EngineCommand::Seek { position_ms } => {
                if self.current.is_none() {
                    return Err(Failure::new("idle", "nothing is loaded"));
                }
                let seconds = position_ms as f64 / 1000.0;
                self.command(json!(["seek", seconds, "absolute+exact"]))
                    .await?;
                self.props.time_pos_ms = Some(position_ms);
                Ok(())
            }
            EngineCommand::Volume { value } => {
                if !(value.is_finite() && (0.0..=1.0).contains(&value)) {
                    return Err(Failure::new("invalid", "volume must be in 0..1"));
                }
                self.set("volume", json!(value * 100.0)).await?;
                self.settings.volume = Some(value);
                self.props.volume = Some(value);
                Ok(())
            }
            EngineCommand::Speed { rate } => {
                if !(rate.is_finite() && (0.5..=3.0).contains(&rate)) {
                    return Err(Failure::new("invalid", "speed must be in 0.5..3.0"));
                }
                self.set("speed", json!(rate)).await?;
                self.settings.speed = Some(rate);
                self.props.speed = Some(rate);
                Ok(())
            }
            EngineCommand::Eq(eq) => {
                filters::validate_eq(&eq)?;
                self.set("af", json!(filters::graph(&eq, self.settings.balance)))
                    .await?;
                self.settings.eq = eq;
                Ok(())
            }
            EngineCommand::Balance { value } => {
                filters::validate_balance(value)?;
                self.set("af", json!(filters::graph(&self.settings.eq, value)))
                    .await?;
                self.settings.balance = value;
                Ok(())
            }
        }
    }

    /// Puts the next item in mpv's playlist for gapless playback.
    async fn append(&mut self, owner: SourceId, token: u64, media: Media) -> Result<(), Failure> {
        // Headers apply when mpv opens the next item (prefetch or transition).
        self.set("http-header-fields", headers(&media)).await?;
        let data = self
            .command(json!(["loadfile", media.url, "append"]))
            .await?;
        self.next = Some(Entry {
            owner,
            token,
            media,
            entry_id: entry_id(&data)?,
            started: false,
        });
        Ok(())
    }

    fn clear_measured(&mut self) {
        self.props.samplerate = None;
        self.props.channels = None;
        self.props.bit_depth = None;
        self.props.bitrate_kbps = None;
    }

    async fn event(&mut self, event: Value) {
        let name = event.get("event").and_then(Value::as_str).unwrap_or("");
        let entry_id = event.get("playlist_entry_id").and_then(Value::as_i64);
        match name {
            "property-change" => self.property(&event),
            "start-file" => {
                if let (Some(id), Some(next)) = (entry_id, &self.next)
                    && next.entry_id == id
                {
                    // Gapless transition: the preloaded item is now current.
                    self.current = self.next.take();
                    self.props.duration_ms = None;
                    self.props.time_pos_ms = Some(0);
                    self.clear_measured();
                }
            }
            "file-loaded" => {
                if let Some(entry) = self.current.as_mut()
                    && !entry.started
                {
                    entry.started = true;
                    let entry = entry.clone();
                    self.emit(&entry, EngineEventKind::Started);
                    if let Some((owner, token, media)) = self.pending.take()
                        && let Err(failure) = self.append(owner, token, media).await
                    {
                        tracing::warn!(error = %failure.message, "Preload failed");
                    }
                }
            }
            "end-file" => {
                let is_current = self
                    .current
                    .as_ref()
                    .is_some_and(|entry| Some(entry.entry_id) == entry_id);
                if is_current && self.ended(&event) {
                    // Never advance on a failure: drop a preloaded item that mpv may open.
                    if let Err(failure) = self.command(json!(["stop"])).await {
                        tracing::warn!(error = %failure.message, "mpv did not stop after a failure");
                    }
                    self.props.idle = true;
                }
            }
            _ => {}
        }
        self.publish();
    }

    /// Reports the end of the current item. Returns true when it failed.
    fn ended(&mut self, event: &Value) -> bool {
        let Some(entry) = self.current.take() else {
            return false;
        };
        let reason = event.get("reason").and_then(Value::as_str).unwrap_or("");
        let early = match (self.props.time_pos_ms, self.props.duration_ms) {
            (Some(position), Some(duration)) => {
                position + EARLY_END_MS.max(duration / 20) < duration
            }
            _ => false,
        };
        let failure = match reason {
            "eof" if early && is_stream(&entry.media.url) => Some(Failure::new(
                "streamEnded",
                "the stream ended before the item's end",
            )),
            "eof" => None,
            "error" => Some(Failure::new(
                "playback",
                event
                    .get("file_error")
                    .and_then(Value::as_str)
                    .unwrap_or("mpv could not play the item")
                    .to_owned(),
            )),
            // "stop", "quit" and "redirect" come from our own replace/stop or a playlist file.
            _ => {
                return false;
            }
        };
        match failure {
            None => {
                self.emit(&entry, EngineEventKind::Ended);
                if self.next.is_none() {
                    self.props.time_pos_ms = None;
                }
                false
            }
            Some(failure) => {
                // The source decides to retry or skip; the engine never does.
                self.next = None;
                self.pending = None;
                self.props.time_pos_ms = None;
                self.emit(&entry, EngineEventKind::Failed(failure));
                true
            }
        }
    }

    fn property(&mut self, event: &Value) {
        let data = event.get("data");
        let number = data.and_then(Value::as_f64).filter(|v| v.is_finite());
        let ms = number.filter(|v| *v >= 0.0).map(|v| (v * 1000.0) as u64);
        match event.get("name").and_then(Value::as_str).unwrap_or("") {
            "pause" => self.props.pause = data.and_then(Value::as_bool).unwrap_or(false),
            "idle-active" => self.props.idle = data.and_then(Value::as_bool).unwrap_or(false),
            "time-pos" => {
                if ms.is_some() || self.current.is_none() {
                    self.props.time_pos_ms = ms;
                }
            }
            "duration" => self.props.duration_ms = ms,
            "audio-params" => {
                let params = data.filter(|v| v.is_object());
                self.props.samplerate = params
                    .and_then(|p| p.get("samplerate"))
                    .and_then(Value::as_f64)
                    .filter(|v| *v > 0.0);
                self.props.channels = params
                    .and_then(|p| p.get("channel-count"))
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok())
                    .filter(|v| *v > 0);
                self.props.bit_depth = params
                    .and_then(|p| p.get("format"))
                    .and_then(Value::as_str)
                    .and_then(bit_depth);
            }
            "audio-bitrate" => {
                self.props.bitrate_kbps = number.filter(|v| *v > 0.0).map(|v| v / 1000.0);
            }
            "volume" => self.props.volume = number.map(|v| (v / 100.0).clamp(0.0, 1.0)),
            "speed" => self.props.speed = number.filter(|v| (0.5..=3.0).contains(v)),
            _ => {}
        }
    }

    fn emit(&self, entry: &Entry, kind: EngineEventKind) {
        let _ = self.ctx.events.send(EngineEvent {
            owner: entry.owner,
            token: entry.token,
            kind,
        });
    }

    fn state(&self) -> EngineState {
        let transport = match &self.current {
            None => Transport::Stopped,
            Some(_) if self.props.pause => Transport::Paused,
            Some(_) => Transport::Playing,
        };
        let track = self
            .current
            .as_ref()
            .map(|entry| measured(&entry.media.track, &self.props));
        EngineState {
            status: EngineStatus::Ready(Empty {}),
            owner: self.owner,
            token: self.current.as_ref().map(|entry| entry.token),
            transport,
            position_ms: self.current.as_ref().and(self.props.time_pos_ms),
            track,
            // Only the engine changes volume and speed, so what it set is the truth;
            // mpv's own report fills in until then.
            volume: self.settings.volume.or(self.props.volume),
            speed: self.settings.speed.or(self.props.speed),
        }
    }

    /// Publishes a changed state. A position-only change waits for [`POSITION_STEP_MS`].
    fn publish(&mut self) {
        let state = self.state();
        self.ctx.state.send_if_modified(|old| {
            let moved = match (state.position_ms, old.position_ms) {
                (Some(now), Some(then)) => now.abs_diff(then) >= POSITION_STEP_MS,
                (now, then) => now != then,
            };
            let mut same_position = state.clone();
            same_position.position_ms = old.position_ms;
            if moved || *old != same_position {
                *old = state;
                true
            } else {
                false
            }
        });
    }
}

fn entry_id(data: &Value) -> Result<i64, Failure> {
    data.get("playlist_entry_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| Failure::new("engine", "mpv did not name the playlist entry"))
}

fn is_stream(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn validate_media(media: &Media) -> Result<(), Failure> {
    if media.url.is_empty() || media.url.contains(['\n', '\r', '\0']) {
        return Err(Failure::new("invalid", "media URL is empty or malformed"));
    }
    let lower = media.url.to_ascii_lowercase();
    if lower.contains("://") && !is_stream(&lower) && !lower.starts_with("file://") {
        return Err(Failure::new(
            "invalid",
            "only http(s) URLs and local files play",
        ));
    }
    let bad = |s: &str| s.contains(['\n', '\r', '\0']);
    if media
        .headers
        .iter()
        .any(|(name, value)| name.is_empty() || name.contains(':') || bad(name) || bad(value))
    {
        return Err(Failure::new("invalid", "malformed HTTP header"));
    }
    Ok(())
}

/// mpv's header list as a JSON array, so a comma inside a value is never split.
fn headers(media: &Media) -> Value {
    Value::Array(
        media
            .headers
            .iter()
            .map(|(name, value)| Value::String(format!("{name}: {value}")))
            .collect(),
    )
}

/// The decoder's sample format names a bit depth only when it is unambiguous.
/// FFmpeg decodes 24-bit FLAC and WAV to `s32` and lossy codecs to float, so
/// those stay unknown rather than guessed.
fn bit_depth(format: &str) -> Option<u32> {
    match format.trim_end_matches('p') {
        "u8" => Some(8),
        "s16" => Some(16),
        "s24" => Some(24),
        _ => None,
    }
}

/// Source metadata plus what mpv measured; a measured value replaces a source value.
fn measured(track: &Track, props: &Props) -> Track {
    let mut track = track.clone();
    track.duration_ms = track.duration_ms.or(props.duration_ms);
    track.sample_rate_hz = props.samplerate.or(track.sample_rate_hz);
    track.channels = props.channels.or(track.channels);
    track.bit_depth = props.bit_depth.or(track.bit_depth);
    track.bitrate_kbps = props.bitrate_kbps.or(track.bitrate_kbps);
    track
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_depth_is_known_only_for_unambiguous_formats() {
        assert_eq!(bit_depth("s16"), Some(16));
        assert_eq!(bit_depth("s16p"), Some(16));
        assert_eq!(bit_depth("s32"), None);
        assert_eq!(bit_depth("floatp"), None);
    }

    #[test]
    fn media_validation_refuses_header_injection_and_other_schemes() {
        let media = |url: &str, headers: Vec<(&str, &str)>| Media {
            url: url.into(),
            headers: headers
                .into_iter()
                .map(|(a, b)| (a.into(), b.into()))
                .collect(),
            track: Track::default(),
        };
        assert!(
            validate_media(&media("https://x/a.flac", vec![("Authorization", "a, b")])).is_ok()
        );
        assert!(validate_media(&media("/music/a.flac", vec![])).is_ok());
        assert!(validate_media(&media("https://x", vec![("A", "b\r\nX: y")])).is_err());
        assert!(validate_media(&media("ytdl://x", vec![])).is_err());
        assert!(validate_media(&media("", vec![])).is_err());
    }
}

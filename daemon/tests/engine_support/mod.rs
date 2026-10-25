//! Shared helpers for the engine tests: generated test audio, an isolated mpv
//! engine that writes its output to a raw PCM file, and a local HTTP server.
//! Nothing here touches the network or the user's audio server.
#![allow(dead_code)]
use pyxamp_daemon::{
    engine::{self, EngineCommand, EngineEvent, EngineEventKind, EngineHandle, Media, mpv},
    protocol::{Failure, SourceId, Track},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{broadcast, watch};

pub const RATE: u32 = 48_000;

/// Writes a 16-bit PCM WAV file. `sample(frame, channel)` gives values in -1..1.
pub fn write_wav(
    path: &Path,
    rate: u32,
    channels: u16,
    frames: usize,
    sample: impl Fn(usize, u16) -> f64,
) {
    let data_len = frames * channels as usize * 2;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for frame in 0..frames {
        for channel in 0..channels {
            let value = (sample(frame, channel).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    std::fs::write(path, bytes).unwrap();
}

/// A sine tone of `seconds` at `hz`, amplitude `amp`, starting at phase `phase`.
pub fn tone(path: &Path, hz: f64, seconds: f64, amp: f64, channels: u16) -> PathBuf {
    tone_from(path, hz, seconds, amp, channels, 0)
}

/// Like [`tone`], continuing a longer tone from frame `offset` (for gapless splits).
pub fn tone_from(
    path: &Path,
    hz: f64,
    seconds: f64,
    amp: f64,
    channels: u16,
    offset: usize,
) -> PathBuf {
    let frames = (seconds * RATE as f64) as usize;
    write_wav(path, RATE, channels, frames, |frame, _| {
        amp * (std::f64::consts::TAU * hz * (frame + offset) as f64 / RATE as f64).sin()
    });
    path.to_path_buf()
}

fn pcm_options(dir: &tempfile::TempDir, output: &Path) -> mpv::Options {
    mpv::Options {
        mpv: mpv_binary(),
        extra_args: vec![
            "--ao=pcm".into(),
            format!("--ao-pcm-file={}", output.display()),
            "--ao-pcm-waveheader=no".into(),
            "--ao-pcm-append=yes".into(),
            "--audio-format=s16".into(),
            format!("--audio-samplerate={RATE}"),
            "--audio-channels=stereo".into(),
        ],
        runtime_dir: dir.path().to_path_buf(),
    }
}

/// Stereo s16 output from the PCM file: (left, right) samples in -1..1.
pub fn read_output(path: &Path) -> (Vec<f64>, Vec<f64>) {
    samples(&std::fs::read(path).unwrap_or_default())
}

fn samples(bytes: &[u8]) -> (Vec<f64>, Vec<f64>) {
    let mut left = vec![];
    let mut right = vec![];
    for frame in bytes.as_chunks::<4>().0 {
        left.push(i16::from_le_bytes([frame[0], frame[1]]) as f64 / 32768.0);
        right.push(i16::from_le_bytes([frame[2], frame[3]]) as f64 / 32768.0);
    }
    (left, right)
}

pub fn rms(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f64>() / samples.len() as f64).sqrt()
}

/// The middle half of a signal, away from filter start-up and end effects.
pub fn middle(samples: &[f64]) -> &[f64] {
    &samples[samples.len() / 4..samples.len() * 3 / 4]
}

/// Frequency from rising zero crossings.
pub fn frequency(samples: &[f64]) -> f64 {
    let crossings = samples
        .windows(2)
        .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
        .count();
    crossings as f64 * RATE as f64 / samples.len() as f64
}

pub fn mpv_binary() -> PathBuf {
    std::env::var_os("PYXAMP_MPV").map_or_else(|| "mpv".into(), PathBuf::from)
}

/// One isolated engine with untimed PCM output (mpv `--ao=pcm`), so tests run
/// faster than real time and measure exactly what the engine produced.
pub struct Harness {
    pub handle: EngineHandle,
    pub active: watch::Sender<Option<SourceId>>,
    pub events: broadcast::Receiver<EngineEvent>,
    pub dir: tempfile::TempDir,
    pub output: PathBuf,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    /// Gated output: mpv writes into a FIFO that nobody reads until the gate opens.
    gate: Option<(
        std::sync::mpsc::Sender<()>,
        std::thread::JoinHandle<Vec<u8>>,
    )>,
}

impl Harness {
    pub fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.pcm");
        let options = pcm_options(&dir, &output);
        Self::with_options(dir, output, options)
    }

    /// Untimed PCM output ignores pause, because mpv's first buffer fill is
    /// unbounded. This variant writes into a FIFO that the test reads only after
    /// [`Harness::open_gate`], so pause, preload order and timing behave as with
    /// a real device. One output session only: the FIFO ends when mpv closes it.
    pub fn start_gated() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.fifo");
        let path = std::ffi::CString::new(output.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let (gate, opened) = std::sync::mpsc::channel::<()>();
        let fifo = output.clone();
        let reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut file = std::fs::File::open(&fifo).unwrap();
            let _ = opened.recv();
            let mut bytes = vec![];
            file.read_to_end(&mut bytes).unwrap();
            bytes
        });
        let mut options = pcm_options(&dir, &output);
        options.extra_args.retain(|a| a != "--ao-pcm-append=yes");
        let mut harness = Self::with_options(dir, output, options);
        harness.gate = Some((gate, reader));
        harness
    }

    /// Real-time null output (no samples kept), for pause and position behaviour.
    pub fn start_realtime() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("none");
        let options = mpv::Options {
            mpv: mpv_binary(),
            extra_args: vec!["--ao=null".into(), "--ao-null-untimed=no".into()],
            runtime_dir: dir.path().to_path_buf(),
        };
        Self::with_options(dir, output, options)
    }

    pub fn open_gate(&self) {
        if let Some((gate, _)) = &self.gate {
            let _ = gate.send(());
        }
    }

    pub fn with_options(dir: tempfile::TempDir, output: PathBuf, options: mpv::Options) -> Self {
        let (handle, ctx) = engine::channels();
        let (active, active_rx) = watch::channel(Some(SourceId::Pyxis));
        let events = handle.events.subscribe();
        let task = tokio::spawn(mpv::run_with(ctx, active_rx, options));
        Self {
            handle,
            active,
            events,
            dir,
            output,
            task,
            gate: None,
        }
    }

    pub async fn ready(&self) {
        let mut state = self.handle.state.clone();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if matches!(
                    state.borrow_and_update().status,
                    pyxamp_daemon::protocol::EngineStatus::Ready(_)
                ) {
                    return;
                }
                state.changed().await.unwrap();
            }
        })
        .await
        .expect("the engine did not become ready");
    }

    pub async fn send(&self, command: EngineCommand) -> Result<(), Failure> {
        self.handle.send(Some(SourceId::Pyxis), command).await
    }

    pub async fn load(&self, token: u64, url: &str, paused: bool) -> Result<(), Failure> {
        self.send(EngineCommand::Load {
            token,
            media: media(url),
            start_ms: 0,
            paused,
        })
        .await
    }

    /// The next event, or panic after `seconds`.
    pub async fn event(&mut self, seconds: u64) -> EngineEvent {
        tokio::time::timeout(Duration::from_secs(seconds), self.events.recv())
            .await
            .expect("no engine event in time")
            .unwrap()
    }

    /// Waits for `kind` on `token`, collecting every event seen on the way.
    pub async fn until(
        &mut self,
        token: u64,
        want: fn(&EngineEventKind) -> bool,
    ) -> Vec<EngineEvent> {
        let mut seen = vec![];
        loop {
            let event = self.event(20).await;
            let done = event.token == token && want(&event.kind);
            seen.push(event);
            if done {
                return seen;
            }
        }
    }

    /// Every event that arrives within `ms`.
    pub async fn quiet(&mut self, ms: u64) -> Vec<EngineEvent> {
        let mut seen = vec![];
        let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, self.events.recv()).await {
            seen.push(event);
        }
        seen
    }

    /// Stops the engine and returns the PCM it wrote.
    pub async fn finish(self) -> (Vec<f64>, Vec<f64>) {
        let Self {
            handle,
            active,
            task,
            output,
            dir,
            events,
            gate,
        } = self;
        drop((handle, active, events));
        let reader = gate.map(|(gate, reader)| {
            let _ = gate.send(());
            reader
        });
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("engine did not stop")
            .unwrap()
            .unwrap();
        let samples = match reader {
            Some(reader) => samples(&reader.join().unwrap()),
            None => read_output(&output),
        };
        drop(dir);
        samples
    }
}

pub fn media(url: &str) -> Media {
    Media {
        url: url.into(),
        headers: vec![],
        track: Track {
            title: "Test tone".into(),
            artist: "Pyxamp tests".into(),
            ..Track::default()
        },
    }
}

pub fn started(kind: &EngineEventKind) -> bool {
    matches!(kind, EngineEventKind::Started)
}
pub fn ended(kind: &EngineEventKind) -> bool {
    matches!(kind, EngineEventKind::Ended)
}
pub fn failed(kind: &EngineEventKind) -> bool {
    matches!(kind, EngineEventKind::Failed(_))
}

/// How the local HTTP server answers one path.
#[derive(Clone)]
pub enum Reply {
    /// The file, after an optional delay.
    File { body: Vec<u8>, delay_ms: u64 },
    /// A 404 after an optional delay.
    Missing { delay_ms: u64 },
    /// Announces the full length but closes after `sent` bytes; later requests get 404.
    Truncated { body: Vec<u8>, sent: usize },
}

/// Each request: its path and its header lines.
pub type RequestLog = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// A local HTTP/1.1 server on 127.0.0.1 that records request headers.
pub struct Server {
    pub base: String,
    pub requests: RequestLog,
}

impl Server {
    pub fn start(routes: Vec<(&str, Reply)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Arc<Vec<(String, Reply)>> = Arc::new(
            routes
                .into_iter()
                .map(|(path, reply)| (path.to_owned(), reply))
                .collect(),
        );
        let requests: RequestLog = Arc::default();
        let log = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let routes = routes.clone();
                let log = log.clone();
                std::thread::spawn(move || serve(stream, &routes, &log));
            }
        });
        Self { base, requests }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn hits(&self, path: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p == path)
            .count()
    }
}

fn serve(
    mut stream: std::net::TcpStream,
    routes: &[(String, Reply)],
    log: &Mutex<Vec<(String, Vec<String>)>>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    let mut headers = vec![];
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        headers.push(header.trim().to_owned());
    }
    let range_start = headers
        .iter()
        .find_map(|h| {
            h.to_ascii_lowercase()
                .strip_prefix("range: bytes=")
                .map(str::to_owned)
        })
        .and_then(|r| r.split('-').next().and_then(|n| n.parse::<usize>().ok()))
        .unwrap_or(0);
    let earlier = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(p, _)| *p == path)
        .count();
    log.lock().unwrap().push((path.clone(), headers));
    let reply = routes
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, r)| r.clone());
    let missing = |stream: &mut std::net::TcpStream| {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    };
    match reply {
        None => missing(&mut stream),
        Some(Reply::Missing { delay_ms }) => {
            std::thread::sleep(Duration::from_millis(delay_ms));
            missing(&mut stream);
        }
        Some(Reply::File { body, delay_ms }) => {
            std::thread::sleep(Duration::from_millis(delay_ms));
            let start = range_start.min(body.len());
            let status = if range_start > 0 {
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\n",
                    body.len().saturating_sub(1),
                    body.len()
                )
            } else {
                "HTTP/1.1 200 OK\r\n".to_owned()
            };
            let head = format!(
                "{status}Content-Type: audio/wav\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len() - start
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body[start..]);
        }
        Some(Reply::Truncated { body, sent }) => {
            if earlier > 0 {
                missing(&mut stream);
                return;
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body[..sent.min(body.len())]);
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

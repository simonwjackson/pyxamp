//! Run only through daemon/tools/test-pipewire: requires an isolated PipeWire + WirePlumber.
use pipewire as pw;
use pw::{properties::properties, spa};
use pyxamp_daemon::{
    capture::{self, CaptureIdentity},
    protocol::*,
};
use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};
use tokio::sync::watch;

struct ToneProcess(Child);
impl ToneProcess {
    fn start(frequency: &str) -> Self {
        Self(
            Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "tone_process", "--nocapture"])
                .env("PYXAMP_TEST_FREQUENCY", frequency)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
}
impl Drop for ToneProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "subprocess audio fixture, explicitly configured by targeted_capture_excludes_unrelated_tone"]
fn tone_process() {
    let frequency: f32 = std::env::var("PYXAMP_TEST_FREQUENCY")
        .expect("Explicit fixture frequency")
        .parse()
        .unwrap();
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None).unwrap();
    let context = pw::context::ContextRc::new(&mainloop, None).unwrap();
    let core = context.connect_rc(None).unwrap();
    let stream = pw::stream::StreamBox::new(&core, "Session tone", properties! {
        "media.type" => "Audio", "media.category" => "Playback", "media.role" => "Music",
        "application.name" => "Session tone", "node.name" => format!("pyxamp.test.tone.{}", std::process::id()),
        "application.process.id" => std::process::id().to_string(), "node.latency" => "256/48000",
    }).unwrap();
    let _listener = stream
        .add_local_listener_with_user_data(0.0_f32)
        .process(move |stream, phase| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let Some(data) = buffer.datas_mut().first_mut() else {
                    return;
                };
                let frames = if let Some(bytes) = data.data() {
                    let frames = bytes.len() / 8;
                    for frame in bytes.as_chunks_mut::<8>().0 {
                        let sample = (*phase).sin() * 0.01;
                        *phase = (*phase + std::f32::consts::TAU * frequency / 48000.0)
                            % std::f32::consts::TAU;
                        frame[..4].copy_from_slice(&sample.to_le_bytes());
                        frame[4..].copy_from_slice(&sample.to_le_bytes());
                    }
                    frames
                } else {
                    0
                };
                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = 8;
                *chunk.size_mut() = (frames * 8) as u32;
            }
        })
        .register()
        .unwrap();
    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    info.set_rate(48000);
    info.set_channels(2);
    let object = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    let bytes = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .unwrap()
    .0
    .into_inner();
    stream
        .connect(
            spa::utils::Direction::Output,
            None,
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut [spa::pod::Pod::from_bytes(&bytes).unwrap()],
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        mainloop.loop_().iterate(Duration::from_millis(10));
    }
}

async fn ready(frames: &mut watch::Receiver<Visualizer>, target: &Target) -> Visualizer {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let frame = frames.borrow_and_update().clone();
            if matches!(frame.status, VisualizerStatus::Ready)
                && frame.target.as_ref() == Some(target)
            {
                return frame;
            }
            if let Some(reason) = &frame.reason {
                eprintln!("capture: {reason}");
            }
            frames.changed().await.unwrap();
        }
    })
    .await
    .expect("No targeted capture samples from isolated PipeWire")
}

#[tokio::test]
#[ignore = "requires isolated PipeWire; run daemon/tools/test-pipewire"]
async fn targeted_capture_excludes_unrelated_tone() {
    let mut low = ToneProcess::start("750");
    let high = ToneProcess::start("6000");
    let identity = |pid, name: &str| CaptureIdentity {
        target: Target {
            bus_name: format!("org.mpris.MediaPlayer2.{name}"),
            owner: format!(":1.{pid}"),
        },
        pid,
        identity: "Session tone".into(),
        desktop_entry: None,
        shared_process: false,
    };
    let low_id = identity(low.0.id(), "low");
    let high_id = identity(high.0.id(), "high");
    let (identities, input) = watch::channel(Some(low_id.clone()));
    let (output, mut frames) =
        watch::channel(Visualizer::silent(None, 0, VisualizerStatus::Idle, None));
    let thread = capture::spawn(input, output);
    let frame = ready(&mut frames, &low_id.target).await;
    eprintln!(
        "low-target columns 2={} 18={}",
        frame.columns[2], frame.columns[18]
    );
    assert!(frame.columns[2] > 10);
    assert!(
        frame.columns[18] < 3,
        "Unrelated 6000Hz leaked into 750Hz capture"
    );
    identities.send_replace(Some(high_id.clone()));
    let frame = ready(&mut frames, &high_id.target).await;
    eprintln!(
        "high-target columns 2={} 18={}",
        frame.columns[2], frame.columns[18]
    );
    assert!(frame.columns[18] > 10);
    assert!(
        frame.columns[2] < 3,
        "Samples were not cleared on target change"
    );
    identities.send_replace(Some(low_id.clone()));
    ready(&mut frames, &low_id.target).await;
    low.0.kill().unwrap();
    low.0.wait().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            frames.changed().await.unwrap();
            let frame = frames.borrow_and_update().clone();
            if matches!(frame.status, VisualizerStatus::Unavailable) {
                assert_eq!(frame.columns, vec![0; 75]);
                assert_eq!(frame.waveform, vec![128; 75]);
                break;
            }
        }
    })
    .await
    .unwrap();
    drop(identities);
    drop(frames);
    thread.join().unwrap();
}

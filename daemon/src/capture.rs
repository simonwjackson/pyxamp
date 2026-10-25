use crate::{
    fft::NullsoftFft,
    protocol::{Target, Visualizer, VisualizerStatus},
};
use pipewire as pw;
use pw::{properties::properties, spa};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
    time::{Duration, Instant},
};
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureIdentity {
    pub target: Target,
    pub pid: u32,
    pub desktop_entry: Option<String>,
    pub identity: String,
    pub shared_process: bool,
}
#[derive(Clone, Debug, Default)]
pub struct AudioNode {
    pub id: u32,
    pub serial: String,
    pub props: BTreeMap<String, String>,
}

fn process_parent(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("PPid:\t")
            .and_then(|n| n.trim().parse().ok())
    })
}
fn same_process_tree(mut pid: u32, parent: u32) -> bool {
    for _ in 0..64 {
        if pid == parent {
            return true;
        }
        if pid <= 1 {
            break;
        }
        let Some(next) = process_parent(pid) else {
            break;
        };
        pid = next;
    }
    false
}
fn browser(identity: &CaptureIdentity) -> bool {
    let exe = std::fs::read_link(format!("/proc/{}/exe", identity.pid))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = format!(
        "{} {} {}",
        identity.identity,
        identity.desktop_entry.as_deref().unwrap_or(""),
        exe
    )
    .to_lowercase();
    [
        "firefox",
        "chromium",
        "chrome",
        "brave",
        "vivaldi",
        "librewolf",
        "zen-browser",
        "floorp",
        "epiphany",
        "falkon",
        "qutebrowser",
        "opera",
    ]
    .iter()
    .any(|name| text.contains(name))
}

/// Never chooses a sink/desktop monitor. A browser's audio process does not identify a tab.
/// Browser capture requires explicit node annotations supplied by a cooperating application.
pub fn select_nodes(identity: &CaptureIdentity, nodes: &[AudioNode]) -> Result<Vec<u32>, String> {
    let mut matches = Vec::new();
    let mut ambiguous_browser = false;
    for node in nodes {
        if node.props.get("media.class").map(String::as_str) != Some("Stream/Output/Audio") {
            continue;
        }
        let pid = node
            .props
            .get("application.process.id")
            .and_then(|s| s.parse::<u32>().ok());
        let exact = node.props.get("media.mpris.bus-name") == Some(&identity.target.bus_name)
            && node.props.get("media.mpris.owner") == Some(&identity.target.owner);
        let process = pid.is_some_and(|pid| same_process_tree(pid, identity.pid));
        if !process {
            continue;
        }
        if node.props.contains_key("media.mpris.bus-name")
            && node.props.contains_key("media.mpris.owner")
            && !exact
        {
            continue;
        }
        if (browser(identity) || identity.shared_process) && !exact {
            ambiguous_browser = true;
            continue;
        }
        // Conflicting live application identity is not resolved by an arbitrary first match.
        if let (Some(expected), Some(actual)) =
            (&identity.desktop_entry, node.props.get("application.id"))
            && !exact
            && actual != expected
            && actual.trim_end_matches(".desktop") != expected.trim_end_matches(".desktop")
        {
            continue;
        }
        matches.push(node.id);
    }
    if ambiguous_browser {
        return Err(
            "Shared/browser audio process cannot identify the followed player/tab; capture unavailable".into(),
        );
    }
    if matches.is_empty() {
        return Err(
            "No unambiguous PipeWire playback stream belongs to the followed D-Bus process".into(),
        );
    }
    matches.sort();
    Ok(matches)
}

#[derive(Default)]
struct SampleWindow {
    samples: VecDeque<f32>,
    updated: Option<Instant>,
    format: spa::param::audio::AudioInfoRaw,
}
impl SampleWindow {
    fn push(&mut self, sample: f32) {
        if self.samples.len() == 1024 {
            self.samples.pop_front();
        }
        self.samples
            .push_back(if sample.is_finite() { sample } else { 0.0 });
        self.updated = Some(Instant::now());
    }
}
struct CaptureStream<'a> {
    serial: String,
    _listener: pw::stream::StreamListener<Rc<RefCell<SampleWindow>>>,
    stream: pw::stream::StreamBox<'a>,
    samples: Rc<RefCell<SampleWindow>>,
}

fn stream<'a>(core: &'a pw::core::Core, node: &AudioNode) -> anyhow::Result<CaptureStream<'a>> {
    let props = properties! {
        "media.type" => "Audio", "media.category" => "Capture", "media.role" => "Music",
        "application.name" => "Pyxamp visualizer", "node.name" => format!("pyxamp.capture.{}", node.id),
        "target.object" => node.serial.clone(),
        "node.dont-reconnect" => "true", "node.dont-fallback" => "true", "node.passive" => "true",
        "stream.monitor" => "true",
    };
    let stream = pw::stream::StreamBox::new(core, "Pyxamp targeted capture", props)?;
    let samples = Rc::new(RefCell::new(SampleWindow::default()));
    let listener = stream
        .add_local_listener_with_user_data(samples.clone())
        .param_changed(|_, window, id, param| {
            if id == spa::param::ParamType::Format.as_raw() {
                let mut window = window.borrow_mut();
                window.samples.clear();
                window.updated = None;
                if let Some(param) = param
                    && let Err(error) = window.format.parse(param)
                {
                    tracing::debug!(%error, "Capture format unavailable");
                }
            }
        })
        .process(|stream, window| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let mut window = window.borrow_mut();
            let channels = window.format.channels() as usize;
            if channels == 0
                || channels > 64
                || window.format.format() != spa::param::audio::AudioFormat::F32LE
            {
                return;
            }
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            let offset = data.chunk().offset() as usize;
            let length = data.chunk().size() as usize;
            let Some(bytes) = data
                .data()
                .and_then(|data| data.get(offset..offset.saturating_add(length)))
            else {
                return;
            };
            for frame in bytes.chunks_exact(channels * 4) {
                let sample = frame
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|v| f32::from_le_bytes([v[0], v[1], v[2], v[3]]))
                    .sum::<f32>()
                    / channels as f32;
                window.push(sample);
            }
        })
        .register()?;
    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    let object = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    let bytes = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )?
    .0
    .into_inner();
    let mut params = [
        spa::pod::Pod::from_bytes(&bytes).ok_or_else(|| anyhow::anyhow!("Invalid format pod"))?
    ];
    // Explicit target id plus dont-fallback prevents session-manager fallback to a default sink.
    stream.connect(
        spa::utils::Direction::Input,
        Some(node.id),
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;
    Ok(CaptureStream {
        serial: node.serial.clone(),
        _listener: listener,
        stream,
        samples,
    })
}

fn session(
    identities: &mut watch::Receiver<Option<CaptureIdentity>>,
    output: &watch::Sender<Visualizer>,
    sequence: &mut u64,
) -> anyhow::Result<()> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let connection_error = Rc::new(RefCell::new(None));
    let error = connection_error.clone();
    let _core_listener = core
        .add_listener_local()
        .error(move |id, _, _, message| {
            if id == 0 {
                *error.borrow_mut() = Some(message.to_string());
            }
        })
        .register();
    let registry = core.get_registry_rc()?;
    let objects = Rc::new(RefCell::new(BTreeMap::<
        u32,
        (pw::types::ObjectType, BTreeMap<String, String>),
    >::new()));
    let globals = objects.clone();
    let removed = objects.clone();
    type BoundObject = (Box<dyn pw::proxy::ProxyT>, Box<dyn pw::proxy::Listener>);
    let bound = Rc::new(RefCell::new(BTreeMap::<u32, BoundObject>::new()));
    let bound_added = bound.clone();
    let bound_removed = bound.clone();
    let registry_weak = registry.downgrade();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |object| {
            if matches!(
                object.type_,
                pw::types::ObjectType::Node | pw::types::ObjectType::Client
            ) {
                let props = object
                    .props
                    .map(|p| {
                        p.iter()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                globals
                    .borrow_mut()
                    .insert(object.id, (object.type_.clone(), props));
                let Some(registry) = registry_weak.upgrade() else {
                    return;
                };
                let id = object.id;
                let update = globals.clone();
                let object: Option<BoundObject> = match object.type_ {
                    pw::types::ObjectType::Node => {
                        registry.bind::<pw::node::Node, _>(object).ok().map(|node| {
                            let listener = node
                                .add_listener_local()
                                .info(move |info| {
                                    if let (Some((_, props)), Some(live)) =
                                        (update.borrow_mut().get_mut(&id), info.props())
                                    {
                                        props.extend(
                                            live.iter()
                                                .map(|(k, v)| (k.to_string(), v.to_string())),
                                        );
                                    }
                                })
                                .register();
                            (
                                Box::new(node) as Box<dyn pw::proxy::ProxyT>,
                                Box::new(listener) as Box<dyn pw::proxy::Listener>,
                            )
                        })
                    }
                    pw::types::ObjectType::Client => registry
                        .bind::<pw::client::Client, _>(object)
                        .ok()
                        .map(|client| {
                            let listener = client
                                .add_listener_local()
                                .info(move |info| {
                                    if let (Some((_, props)), Some(live)) =
                                        (update.borrow_mut().get_mut(&id), info.props())
                                    {
                                        props.extend(
                                            live.iter()
                                                .map(|(k, v)| (k.to_string(), v.to_string())),
                                        );
                                    }
                                })
                                .register();
                            (
                                Box::new(client) as Box<dyn pw::proxy::ProxyT>,
                                Box::new(listener) as Box<dyn pw::proxy::Listener>,
                            )
                        }),
                    _ => None,
                };
                if let Some(object) = object {
                    bound_added.borrow_mut().insert(id, object);
                }
            }
        })
        .global_remove(move |id| {
            removed.borrow_mut().remove(&id);
            bound_removed.borrow_mut().remove(&id);
        })
        .register();
    let mut streams = BTreeMap::new();
    let mut selected: Option<CaptureIdentity> = None;
    let mut fft = NullsoftFft::default();
    let mut last_emit = Instant::now();
    let mut pending_clear = true;
    loop {
        mainloop.loop_().iterate(Duration::from_millis(5));
        if let Some(error) = connection_error.borrow_mut().take() {
            anyhow::bail!("PipeWire disconnected: {error}");
        }
        if identities.has_changed().is_err() || output.is_closed() {
            return Ok(());
        }
        let identity = identities.borrow_and_update().clone();
        if identity != selected {
            streams.clear();
            selected = identity.clone();
            pending_clear = true;
        }
        if last_emit.elapsed() < Duration::from_millis(17) {
            continue;
        }
        last_emit = Instant::now();
        *sequence += 1;
        if pending_clear {
            pending_clear = false;
            output.send_replace(Visualizer::silent(
                identity.as_ref().map(|i| i.target.clone()),
                *sequence,
                if identity.is_some() {
                    VisualizerStatus::Unavailable
                } else {
                    VisualizerStatus::Idle
                },
                identity
                    .as_ref()
                    .map(|_| "Target changed; samples cleared".into()),
            ));
            continue;
        }
        let Some(identity) = identity else {
            continue;
        };
        let nodes: Vec<AudioNode> = {
            let objects = objects.borrow();
            objects
                .iter()
                .filter(|(_, (kind, _))| *kind == pw::types::ObjectType::Node)
                .map(|(id, (_, props))| {
                    let mut props = props.clone();
                    if let Some((_, client)) = props
                        .get("client.id")
                        .and_then(|id| id.parse::<u32>().ok())
                        .and_then(|id| objects.get(&id))
                    {
                        for (k, v) in client {
                            props.entry(k.clone()).or_insert_with(|| v.clone());
                        }
                    }
                    AudioNode {
                        id: *id,
                        serial: props
                            .get("object.serial")
                            .cloned()
                            .unwrap_or_else(|| id.to_string()),
                        props,
                    }
                })
                .collect()
        };
        let frame = match select_nodes(&identity, &nodes) {
            Err(reason) => {
                streams.clear();
                Visualizer::silent(
                    Some(identity.target.clone()),
                    *sequence,
                    VisualizerStatus::Unavailable,
                    Some(reason),
                )
            }
            Ok(ids) => {
                let ids: BTreeSet<_> = ids.into_iter().collect();
                streams.retain(|id, capture: &mut CaptureStream<'_>| {
                    ids.contains(id)
                        && nodes
                            .iter()
                            .any(|node| node.id == *id && node.serial == capture.serial)
                });
                let mut error = None;
                for node in nodes.iter().filter(|node| ids.contains(&node.id)) {
                    if let std::collections::btree_map::Entry::Vacant(entry) =
                        streams.entry(node.id)
                    {
                        match stream(&core, node) {
                            Ok(stream) => {
                                entry.insert(stream);
                            }
                            Err(e) => error = Some(e.to_string()),
                        }
                    }
                }
                let mut samples = [0.0; 1024];
                let mut ready = error.is_none() && streams.len() == ids.len();
                for capture in streams.values() {
                    let window = capture.samples.borrow();
                    if !matches!(capture.stream.state(), pw::stream::StreamState::Streaming)
                        || window.samples.len() != 1024
                        || window
                            .updated
                            .is_none_or(|t| t.elapsed() > Duration::from_millis(250))
                    {
                        ready = false;
                        continue;
                    }
                    for (sum, sample) in samples.iter_mut().zip(window.samples.iter()) {
                        *sum += sample;
                    }
                }
                if ready {
                    let (columns, waveform) = fft.frame(&samples);
                    Visualizer {
                        target: Some(identity.target.clone()),
                        sequence: *sequence,
                        status: VisualizerStatus::Ready,
                        columns,
                        waveform,
                        reason: None,
                    }
                } else {
                    Visualizer::silent(
                        Some(identity.target.clone()),
                        *sequence,
                        VisualizerStatus::Unavailable,
                        Some(error.unwrap_or_else(|| {
                            "Waiting for targeted stream samples (no desktop fallback)".into()
                        })),
                    )
                }
            }
        };
        output.send_replace(frame);
    }
}

/// PipeWire objects stay on one dedicated thread; watch channels retain only the latest frame.
pub fn spawn(
    mut identities: watch::Receiver<Option<CaptureIdentity>>,
    output: watch::Sender<Visualizer>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut sequence = 0;
        loop {
            match session(&mut identities, &output, &mut sequence) {
                Ok(()) => return,
                Err(error) => {
                    sequence += 1;
                    let target = identities.borrow().as_ref().map(|i| i.target.clone());
                    let status = if target.is_some() {
                        VisualizerStatus::Unavailable
                    } else {
                        VisualizerStatus::Idle
                    };
                    output.send_replace(Visualizer::silent(
                        target,
                        sequence,
                        status,
                        Some(error.to_string()),
                    ));
                    tracing::warn!(%error, "PipeWire capture unavailable; retrying in 1 second");
                    if output.is_closed() || identities.has_changed().is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excludes_sink_and_other_process_and_rejects_browser_tab_guess() {
        let target = Target {
            bus_name: "org.mpris.MediaPlayer2.player".into(),
            owner: ":1.2".into(),
        };
        let identity = CaptureIdentity {
            target,
            pid: std::process::id(),
            identity: "Audio player".into(),
            desktop_entry: None,
            shared_process: false,
        };
        let node = |id: u32, pid: u32, class: &str| AudioNode {
            id,
            serial: id.to_string(),
            props: BTreeMap::from([
                ("media.class".into(), class.into()),
                ("application.process.id".into(), pid.to_string()),
            ]),
        };
        let nodes = vec![
            node(1, identity.pid, "Stream/Output/Audio"),
            node(2, 1, "Stream/Output/Audio"),
            node(3, identity.pid, "Audio/Sink"),
        ];
        assert_eq!(select_nodes(&identity, &nodes).unwrap(), vec![1]);
        let browser = CaptureIdentity {
            identity: "Firefox".into(),
            ..identity
        };
        assert!(select_nodes(&browser, &nodes).unwrap_err().contains("tab"));
    }
}

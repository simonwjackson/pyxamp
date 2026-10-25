//! The system volume: the default PipeWire audio sink's volume and mute.
//!
//! Reading is event-driven. One thread holds a PipeWire connection and follows
//! the `default` metadata (`default.audio.sink`), every sink node's `Props` and
//! every audio device's `Route`. It works out the value the same way
//! WirePlumber's mixer API (and so `wpctl get-volume`) does: the active route's
//! props when the sink belongs to a device route, else the node's props; the
//! first channel volume on the cubic scale. Pyxamp never sets a volume at
//! start-up; it only reads.
//!
//! Setting runs `wpctl set-volume @DEFAULT_AUDIO_SINK@ <value>`, so WirePlumber
//! applies it to the hardware route exactly as volume keys do.
use crate::protocol::{Failure, SYSTEM_VOLUME_MAX, Snapshot, SystemVolume};
use pipewire as pw;
use pw::spa::pod::{Value, ValueArray, deserialize::PodDeserializer};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::OsString,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::watch;

// spa/param/props.h and spa/param/route.h.
const PROP_MUTE: u32 = 0x10004;
const PROP_CHANNEL_VOLUMES: u32 = 0x10008;
const ROUTE_DEVICE: u32 = 3;
const ROUTE_PROPS: u32 = 10;

/// How long one `wpctl set-volume` may take.
const SET_TIMEOUT: Duration = Duration::from_secs(2);

/// One sink node as the reader knows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sink {
    pub name: String,
    /// `device.id`: the device whose route carries the volume, if any.
    pub device: Option<u32>,
    /// `card.profile.device`: which of the device's routes is this sink's.
    pub profile_device: Option<i32>,
    /// From the node's `Props`.
    pub props: Option<SystemVolume>,
}

/// What the reader has seen of the graph. Pure, so the choice is testable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    /// `default.audio.sink` from the `default` metadata: a node name.
    pub default_sink: Option<String>,
    pub sinks: BTreeMap<u32, Sink>,
    /// Device id -> route device index -> the route's props.
    pub routes: BTreeMap<u32, BTreeMap<i32, Option<SystemVolume>>>,
}

impl Graph {
    /// The default sink's volume, as `wpctl get-volume @DEFAULT_AUDIO_SINK@` shows it.
    pub fn system_volume(&self) -> Option<SystemVolume> {
        let name = self.default_sink.as_deref()?;
        let sink = self.sinks.values().find(|sink| sink.name == name)?;
        let route = sink
            .device
            .zip(sink.profile_device)
            .and_then(|(device, index)| self.routes.get(&device)?.get(&index).cloned())
            .flatten();
        route.or_else(|| sink.props.clone())
    }
}

/// `{"name":"alsa_output..."}`, the value of `default.audio.sink`.
pub fn default_sink_name(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    value.get("name")?.as_str().map(str::to_owned)
}

/// Props with channel volumes and mute, as WirePlumber's mixer API requires both.
pub fn from_props(props: &Value) -> Option<SystemVolume> {
    let Value::Object(object) = props else {
        return None;
    };
    let mut first = None;
    let mut muted = None;
    for property in &object.properties {
        match (property.key, &property.value) {
            (PROP_CHANNEL_VOLUMES, Value::ValueArray(ValueArray::Float(values))) => {
                first = values.first().copied();
            }
            (PROP_MUTE, Value::Bool(value)) => muted = Some(*value),
            _ => {}
        }
    }
    let linear = f64::from(first?);
    let volume = if linear.is_finite() && linear > 0.0 {
        // The cubic scale of wpctl and the volume keys; 4 decimals hide f32 noise.
        (linear.cbrt().min(SYSTEM_VOLUME_MAX) * 10_000.0).round() / 10_000.0
    } else {
        0.0
    };
    Some(SystemVolume {
        volume,
        muted: muted?,
    })
}

/// A device `Route` param: its device index and the volume its props carry.
pub fn from_route(route: &Value) -> Option<(i32, Option<SystemVolume>)> {
    let Value::Object(object) = route else {
        return None;
    };
    let mut device = None;
    let mut props = None;
    for property in &object.properties {
        match (property.key, &property.value) {
            (ROUTE_DEVICE, Value::Int(index)) => device = Some(*index),
            (ROUTE_PROPS, value) => props = from_props(value),
            _ => {}
        }
    }
    Some((device?, props))
}

fn decode(pod: Option<&pw::spa::pod::Pod>) -> Option<Value> {
    PodDeserializer::deserialize_any_from(pod?.as_bytes())
        .ok()
        .map(|(_, value)| value)
}

fn props_of(dict: &pw::spa::utils::dict::DictRef) -> BTreeMap<String, String> {
    dict.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

type Bound = (Box<dyn pw::proxy::ProxyT>, Box<dyn pw::proxy::Listener>);

fn session(output: &watch::Sender<Option<SystemVolume>>) -> anyhow::Result<()> {
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
    let graph = Rc::new(RefCell::new(Graph::default()));
    let bound = Rc::new(RefCell::new(BTreeMap::<u32, Bound>::new()));
    let (added_graph, removed_graph) = (graph.clone(), graph.clone());
    let (added_bound, removed_bound) = (bound.clone(), bound.clone());
    let registry_weak = registry.downgrade();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |object| {
            let props = object.props.map(props_of).unwrap_or_default();
            let class = props.get("media.class").map(String::as_str);
            let Some(registry) = registry_weak.upgrade() else {
                return;
            };
            let id = object.id;
            let proxy: Option<Bound> = match object.type_ {
                pw::types::ObjectType::Metadata
                    if props.get("metadata.name").map(String::as_str) == Some("default") =>
                {
                    registry
                        .bind::<pw::metadata::Metadata, _>(object)
                        .ok()
                        .map(|metadata| {
                            let graph = added_graph.clone();
                            let listener = metadata
                                .add_listener_local()
                                .property(move |subject, key, _, value| {
                                    if subject == 0 {
                                        match key {
                                            Some("default.audio.sink") => {
                                                graph.borrow_mut().default_sink =
                                                    value.and_then(default_sink_name);
                                            }
                                            None => graph.borrow_mut().default_sink = None,
                                            _ => {}
                                        }
                                    }
                                    0
                                })
                                .register();
                            (
                                Box::new(metadata) as Box<dyn pw::proxy::ProxyT>,
                                Box::new(listener) as Box<dyn pw::proxy::Listener>,
                            )
                        })
                }
                pw::types::ObjectType::Node
                    if matches!(class, Some("Audio/Sink" | "Audio/Duplex")) =>
                {
                    added_graph.borrow_mut().sinks.insert(
                        id,
                        Sink {
                            name: props.get("node.name").cloned().unwrap_or_default(),
                            ..Sink::default()
                        },
                    );
                    registry.bind::<pw::node::Node, _>(object).ok().map(|node| {
                        node.subscribe_params(&[pw::spa::param::ParamType::Props]);
                        let (info_graph, param_graph) = (added_graph.clone(), added_graph.clone());
                        let listener = node
                            .add_listener_local()
                            .info(move |info| {
                                let Some(live) = info.props() else { return };
                                let live = props_of(live);
                                if let Some(sink) = info_graph.borrow_mut().sinks.get_mut(&id) {
                                    if let Some(name) = live.get("node.name") {
                                        sink.name.clone_from(name);
                                    }
                                    sink.device =
                                        live.get("device.id").and_then(|d| d.parse().ok());
                                    sink.profile_device = live
                                        .get("card.profile.device")
                                        .and_then(|d| d.parse().ok());
                                }
                            })
                            .param(move |_, kind, _, _, pod| {
                                if kind != pw::spa::param::ParamType::Props {
                                    return;
                                }
                                // A Props param without volumes (some nodes send several) keeps the last reading.
                                if let Some(reading) = decode(pod).as_ref().and_then(from_props)
                                    && let Some(sink) = param_graph.borrow_mut().sinks.get_mut(&id)
                                {
                                    sink.props = Some(reading);
                                }
                            })
                            .register();
                        (
                            Box::new(node) as Box<dyn pw::proxy::ProxyT>,
                            Box::new(listener) as Box<dyn pw::proxy::Listener>,
                        )
                    })
                }
                pw::types::ObjectType::Device if class == Some("Audio/Device") => registry
                    .bind::<pw::device::Device, _>(object)
                    .ok()
                    .map(|device| {
                        device.subscribe_params(&[pw::spa::param::ParamType::Route]);
                        let graph = added_graph.clone();
                        let listener = device
                            .add_listener_local()
                            .param(move |_, kind, _, _, pod| {
                                if kind != pw::spa::param::ParamType::Route {
                                    return;
                                }
                                if let Some((index, reading)) =
                                    decode(pod).as_ref().and_then(from_route)
                                {
                                    graph
                                        .borrow_mut()
                                        .routes
                                        .entry(id)
                                        .or_default()
                                        .insert(index, reading);
                                }
                            })
                            .register();
                        (
                            Box::new(device) as Box<dyn pw::proxy::ProxyT>,
                            Box::new(listener) as Box<dyn pw::proxy::Listener>,
                        )
                    }),
                _ => None,
            };
            if let Some(proxy) = proxy {
                added_bound.borrow_mut().insert(id, proxy);
            }
        })
        .global_remove(move |id| {
            let mut graph = removed_graph.borrow_mut();
            graph.sinks.remove(&id);
            graph.routes.remove(&id);
            removed_bound.borrow_mut().remove(&id);
        })
        .register();
    loop {
        // Events wake the loop at once; the timeout only bounds the exit check.
        mainloop.loop_().iterate(Duration::from_millis(100));
        if let Some(error) = connection_error.borrow_mut().take() {
            anyhow::bail!("PipeWire disconnected: {error}");
        }
        if output.is_closed() {
            return Ok(());
        }
        let reading = graph.borrow().system_volume();
        output.send_if_modified(|old| {
            let changed = *old != reading;
            if changed {
                *old = reading;
            }
            changed
        });
    }
}

/// Reads the system volume on a dedicated thread until `output` closes.
/// Without PipeWire the value is absent, and the thread tries again every second.
pub fn spawn(output: watch::Sender<Option<SystemVolume>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut warned = false;
        loop {
            match session(&output) {
                Ok(()) => return,
                Err(error) => {
                    output.send_replace(None);
                    if warned {
                        tracing::debug!(%error, "System volume unavailable");
                    } else {
                        tracing::warn!(%error, "System volume unavailable; retrying every second");
                        warned = true;
                    }
                    if output.is_closed() {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    })
}

/// Copies each reading into the snapshot, until either side closes.
pub async fn publish(
    mut readings: watch::Receiver<Option<SystemVolume>>,
    state: watch::Sender<Snapshot>,
) {
    loop {
        let reading = readings.borrow_and_update().clone();
        state.send_if_modified(|snapshot| {
            if snapshot.system_volume == reading {
                return false;
            }
            snapshot.system_volume = reading;
            snapshot.revision += 1;
            true
        });
        if readings.changed().await.is_err() {
            return;
        }
    }
}

/// Sets the system volume with `wpctl`. Sets run one at a time; a set still
/// waiting when a newer one arrives is superseded (accepted without running),
/// so a slider drag cannot queue up stale values.
#[derive(Clone)]
pub struct Control {
    program: Arc<OsString>,
    turn: Arc<tokio::sync::Mutex<()>>,
    latest: Arc<AtomicU64>,
}

impl Control {
    /// `wpctl` from PATH (the package puts WirePlumber's there).
    pub fn wpctl() -> Self {
        Self::with_program("wpctl")
    }

    pub fn with_program(program: impl Into<OsString>) -> Self {
        Self {
            program: Arc::new(program.into()),
            turn: Arc::default(),
            latest: Arc::default(),
        }
    }

    /// `value` is 0..1 on wpctl's cubic scale; the request boundary checks it.
    pub async fn set(&self, value: f64) -> Result<(), Failure> {
        if !(value.is_finite() && (0.0..=1.0).contains(&value)) {
            return Err(Failure::new("invalid", "volume must be in 0..1"));
        }
        let ticket = self.latest.fetch_add(1, Ordering::SeqCst) + 1;
        let _turn = self.turn.lock().await;
        if self.latest.load(Ordering::SeqCst) != ticket {
            return Ok(());
        }
        let run = tokio::process::Command::new(self.program.as_os_str())
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{value:.4}")])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .output();
        let output = match tokio::time::timeout(SET_TIMEOUT, run).await {
            Err(_) => {
                return Err(Failure::new(
                    "systemVolume",
                    "wpctl did not answer within 2 s",
                ));
            }
            Ok(Err(error)) => {
                return Err(Failure::new(
                    "systemVolume",
                    format!("wpctl cannot run: {error}"),
                ));
            }
            Ok(Ok(output)) => output,
        };
        if output.status.success() {
            return Ok(());
        }
        let reason: String = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(200)
            .collect();
        Err(Failure::new(
            "systemVolume",
            if reason.is_empty() {
                format!("wpctl failed ({})", output.status)
            } else {
                reason
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw::spa::pod::{Object, Property};

    fn props(volumes: Vec<f32>, mute: Option<bool>) -> Value {
        let mut properties = vec![Property::new(
            PROP_CHANNEL_VOLUMES,
            Value::ValueArray(ValueArray::Float(volumes)),
        )];
        if let Some(mute) = mute {
            properties.push(Property::new(PROP_MUTE, Value::Bool(mute)));
        }
        Value::Object(Object {
            type_: pw::spa::utils::SpaTypes::ObjectParamProps.as_raw(),
            id: pw::spa::param::ParamType::Props.as_raw(),
            properties,
        })
    }

    /// Through real pod bytes, as the param callback gets them.
    fn round_trip(value: &Value) -> Value {
        let bytes = pw::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            value,
        )
        .unwrap()
        .0
        .into_inner();
        let pod = pw::spa::pod::Pod::from_bytes(&bytes).unwrap();
        decode(Some(pod)).unwrap()
    }

    #[test]
    fn props_read_like_wpctl_first_channel_cubic_with_mute() {
        let reading = from_props(&round_trip(&props(vec![0.125, 0.9], Some(true)))).unwrap();
        assert_eq!(
            reading,
            SystemVolume {
                volume: 0.5,
                muted: true
            }
        );
        // Boost above 100 % keeps its real value.
        let boost = from_props(&props(vec![1.728], Some(false))).unwrap();
        assert_eq!(boost.volume, 1.2);
        assert_eq!(
            from_props(&props(vec![1000.0], Some(false)))
                .unwrap()
                .volume,
            SYSTEM_VOLUME_MAX
        );
        assert_eq!(
            from_props(&props(vec![-1.0], Some(false))).unwrap().volume,
            0.0
        );
        assert_eq!(from_props(&props(vec![], Some(false))), None);
        assert_eq!(
            from_props(&props(vec![0.5], None)),
            None,
            "mute is required"
        );
        assert_eq!(from_props(&Value::Int(3)), None);
    }

    #[test]
    fn route_param_names_its_device_and_props() {
        let route = Value::Object(Object {
            type_: pw::spa::utils::SpaTypes::ObjectParamRoute.as_raw(),
            id: pw::spa::param::ParamType::Route.as_raw(),
            properties: vec![
                Property::new(1, Value::Int(7)),
                Property::new(ROUTE_DEVICE, Value::Int(2)),
                Property::new(ROUTE_PROPS, props(vec![0.008], Some(false))),
            ],
        });
        let (device, reading) = from_route(&round_trip(&route)).unwrap();
        assert_eq!(device, 2);
        assert_eq!(reading.unwrap().volume, 0.2);
    }

    #[test]
    fn default_sink_prefers_its_device_route_then_its_node_props() {
        let volume = |volume| {
            Some(SystemVolume {
                volume,
                muted: false,
            })
        };
        let mut graph = Graph::default();
        assert_eq!(graph.system_volume(), None, "no default sink");
        graph.sinks.insert(
            40,
            Sink {
                name: "speakers".into(),
                device: Some(30),
                profile_device: Some(1),
                props: volume(0.3),
            },
        );
        graph.sinks.insert(
            41,
            Sink {
                name: "headset".into(),
                props: volume(0.9),
                ..Sink::default()
            },
        );
        graph.default_sink = Some("speakers".into());
        assert_eq!(
            graph.system_volume(),
            volume(0.3),
            "no route yet: node props"
        );
        graph
            .routes
            .entry(30)
            .or_default()
            .extend([(0, volume(0.1)), (1, volume(0.6))]);
        assert_eq!(graph.system_volume(), volume(0.6), "the sink's own route");
        graph.default_sink = Some("headset".into());
        assert_eq!(graph.system_volume(), volume(0.9));
        graph.default_sink = Some("gone".into());
        assert_eq!(
            graph.system_volume(),
            None,
            "the default names no live sink"
        );
        assert_eq!(
            default_sink_name(r#"{ "name": "alsa_output.pci" }"#).as_deref(),
            Some("alsa_output.pci")
        );
        assert_eq!(default_sink_name("not json"), None);
        assert_eq!(default_sink_name(r#"{"other":1}"#), None);
    }

    #[tokio::test]
    async fn set_reports_wpctl_failure_and_refuses_out_of_range() {
        let control = Control::with_program("false");
        assert_eq!(control.set(0.5).await.unwrap_err().code, "systemVolume");
        let missing = Control::with_program("/nonexistent/wpctl");
        assert!(
            missing
                .set(0.5)
                .await
                .unwrap_err()
                .message
                .contains("cannot run")
        );
        assert_eq!(control.set(1.5).await.unwrap_err().code, "invalid");
        assert!(Control::with_program("true").set(0.25).await.is_ok());
    }

    #[tokio::test]
    async fn publish_copies_readings_into_the_snapshot_with_a_new_revision() {
        let (readings_tx, readings) = watch::channel(None);
        let (state, mut snapshots) = watch::channel(Snapshot::default());
        let task = tokio::spawn(publish(readings, state));
        readings_tx.send_replace(Some(SystemVolume {
            volume: 0.4,
            muted: false,
        }));
        tokio::time::timeout(Duration::from_secs(2), async {
            while snapshots.borrow_and_update().system_volume.is_none() {
                snapshots.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(snapshots.borrow().revision, 1);
        drop(readings_tx);
        task.await.unwrap();
    }
}

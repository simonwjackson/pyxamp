use crate::{
    CommandRequest,
    capture::CaptureIdentity,
    policy::{FollowPolicy, is_self},
    protocol::*,
};
use futures_util::StreamExt;
use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};
use tokio::sync::{mpsc, watch};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

pub const PATH: &str = "/org/mpris/MediaPlayer2";
pub const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
pub const ROOT: &str = "org.mpris.MediaPlayer2";
type Properties = HashMap<String, OwnedValue>;

#[derive(Clone, Debug)]
pub struct Observation {
    pub player: Following,
    pub capture: CaptureIdentity,
}

async fn proxy<'a>(
    connection: &'a Connection,
    owner: &'a str,
    interface: &'a str,
) -> zbus::Result<Proxy<'a>> {
    Proxy::new(connection, owner, PATH, interface).await
}
fn string(p: &Properties, key: &str) -> Option<String> {
    p.get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .map(str::to_owned)
}
fn boolean(p: &Properties, key: &str) -> bool {
    p.get(key)
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}
fn number(p: &Properties, key: &str) -> Option<f64> {
    p.get(key)
        .and_then(|v| f64::try_from(v).ok())
        .filter(|v| v.is_finite())
}
fn integer(p: &Properties, key: &str) -> Option<u64> {
    p.get(key)
        .and_then(|v| i64::try_from(v).ok())
        .and_then(|v| v.try_into().ok())
}
fn transport(s: Option<&str>) -> Transport {
    match s {
        Some("Playing") => Transport::Playing,
        Some("Paused") => Transport::Paused,
        Some("Stopped") => Transport::Stopped,
        _ => Transport::Unknown,
    }
}
async fn properties(
    connection: &Connection,
    owner: &str,
    interface: &str,
) -> zbus::Result<Properties> {
    let p = proxy(connection, owner, "org.freedesktop.DBus.Properties").await?;
    p.call("GetAll", &(interface,)).await
}

pub async fn observe(connection: &Connection, target: Target) -> anyhow::Result<Observation> {
    let p = properties(connection, &target.owner, PLAYER).await?;
    let root = properties(connection, &target.owner, ROOT).await?;
    let xml: String = proxy(
        connection,
        &target.owner,
        "org.freedesktop.DBus.Introspectable",
    )
    .await?
    .call("Introspect", &())
    .await?;
    anyhow::ensure!(
        xml.len() <= 1024 * 1024,
        "MPRIS introspection exceeds 1 MiB"
    );
    // D-Bus introspection includes a standard DOCTYPE. No external entity resolver is installed.
    let tree = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 16_384,
            entity_resolver: None,
        },
    )?;
    let writable = |name| {
        tree.descendants()
            .filter(|n| n.has_tag_name("interface") && n.attribute("name") == Some(PLAYER))
            .flat_map(|n| n.children())
            .any(|n| {
                n.has_tag_name("property")
                    && n.attribute("name") == Some(name)
                    && matches!(n.attribute("access"), Some("write" | "readwrite"))
            })
    };
    let metadata = p
        .get("Metadata")
        .and_then(|v| Properties::try_from(v.try_clone().ok()?).ok())
        .unwrap_or_default();
    let track = Track {
        title: string(&metadata, "xesam:title").unwrap_or_default(),
        artist: metadata
            .get("xesam:artist")
            .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
            .unwrap_or_default()
            .join(", "),
        id: metadata
            .get("mpris:trackid")
            .and_then(|v| <&zbus::zvariant::ObjectPath<'_>>::try_from(v).ok())
            .map(ToString::to_string),
        album: string(&metadata, "xesam:album"),
        duration_ms: integer(&metadata, "mpris:length").map(|v| v / 1000),
        artwork_url: string(&metadata, "mpris:artUrl"),
        // No audio-format inference from file names, apps or codecs.
        ..Track::default()
    };
    let control = boolean(&p, "CanControl");
    let volume = number(&p, "Volume");
    let repeat = string(&p, "LoopStatus").and_then(|s| match s.as_str() {
        "None" => Some(Repeat::Off),
        "Playlist" => Some(Repeat::All),
        "Track" => Some(Repeat::One),
        _ => None,
    });
    let shuffle = p.get("Shuffle").and_then(|v| bool::try_from(v).ok());
    let capabilities = Capabilities {
        play: control && boolean(&p, "CanPlay"),
        pause: control && boolean(&p, "CanPause"),
        stop: control,
        next: control && boolean(&p, "CanGoNext"),
        previous: control && boolean(&p, "CanGoPrevious"),
        seek: control
            && boolean(&p, "CanSeek")
            && track.duration_ms.is_some()
            && track.id.is_some(),
        volume: control && volume.is_some() && writable("Volume"),
        repeat: control && repeat.is_some() && writable("LoopStatus"),
        shuffle: control && shuffle.is_some() && writable("Shuffle"),
    };
    let dbus = zbus::fdo::DBusProxy::new(connection).await?;
    let pid = dbus
        .get_connection_unix_process_id(target.owner.as_str().try_into()?)
        .await?;
    let identity = string(&root, "Identity").unwrap_or_else(|| target.bus_name.clone());
    let capture = CaptureIdentity {
        target: target.clone(),
        pid,
        desktop_entry: string(&root, "DesktopEntry"),
        identity: identity.clone(),
        shared_process: false,
    };
    Ok(Observation {
        player: Following {
            target,
            identity,
            track,
            transport: transport(string(&p, "PlaybackStatus").as_deref()),
            capabilities,
            position_ms: integer(&p, "Position").map(|v| v / 1000),
            volume,
            repeat,
            shuffle,
            rate: number(&p, "Rate").filter(|r| *r > 0.0),
        },
        capture,
    })
}

/// A command is addressed to the captured unique owner, never the reusable service.
/// Refreshes capabilities/track before each native call; never mutates observed state.
pub async fn execute(
    connection: &Connection,
    selected: Option<&Target>,
    command: &Command,
) -> Result<(), Failure> {
    let selected = selected.cloned();
    execute_fenced(connection, || selected.clone(), command).await
}

async fn execute_fenced(
    connection: &Connection,
    followed: impl Fn() -> Option<Target>,
    command: &Command,
) -> Result<(), Failure> {
    command.validate()?;
    let selected =
        followed().ok_or_else(|| Failure::new("noTarget", "No playing foreign player"))?;
    if selected != command.target || is_self(&command.target.bus_name) {
        return Err(Failure::new("staleTarget", "Followed target changed"));
    }
    let dbus = zbus::fdo::DBusProxy::new(connection)
        .await
        .map_err(native)?;
    let bus = command
        .target
        .bus_name
        .as_str()
        .try_into()
        .map_err(|e: zbus::names::Error| Failure::new("invalid", e.to_string()))?;
    let owner = dbus
        .get_name_owner(bus)
        .await
        .map_err(|_| Failure::new("staleTarget", "Player disappeared"))?;
    if owner.as_str() != command.target.owner {
        return Err(Failure::new("staleTarget", "D-Bus owner changed"));
    }
    let current = observe(connection, command.target.clone())
        .await
        .map_err(native)?;
    let c = &current.player.capabilities;
    let supported = match &command.action {
        Action::Play(_) => c.play,
        Action::Pause(_) => c.pause,
        Action::PlayPause(_) => {
            if current.player.transport == Transport::Playing {
                c.pause
            } else {
                c.play
            }
        }
        Action::Stop(_) => c.stop,
        Action::Previous(_) => c.previous,
        Action::Next(_) => c.next,
        Action::Seek(s) => {
            if current.player.track.id.as_deref() != Some(&s.track_id) {
                return Err(Failure::new("staleTrack", "Track changed before seek"));
            }
            if current
                .player
                .track
                .duration_ms
                .is_some_and(|d| s.position_ms > d)
            {
                return Err(Failure::new("invalid", "Position exceeds duration"));
            }
            c.seek
        }
        Action::Volume { .. } => c.volume,
        Action::Repeat { .. } => c.repeat,
        Action::Shuffle { .. } => c.shuffle,
    };
    if !supported {
        return Err(Failure::new(
            "unsupported",
            "Player does not support this action",
        ));
    }
    let p = proxy(connection, &command.target.owner, PLAYER)
        .await
        .map_err(native)?;
    // GetAll/introspection can yield. Fence again after those native reads.
    let owner = dbus
        .get_name_owner(
            command
                .target
                .bus_name
                .as_str()
                .try_into()
                .map_err(native)?,
        )
        .await
        .map_err(|_| Failure::new("staleTarget", "Player disappeared"))?;
    if owner.as_str() != command.target.owner || followed().as_ref() != Some(&command.target) {
        return Err(Failure::new(
            "staleTarget",
            "Target changed during native validation",
        ));
    }
    let result: zbus::Result<()> = match &command.action {
        Action::Play(_) => p.call("Play", &()).await,
        Action::Pause(_) => p.call("Pause", &()).await,
        Action::PlayPause(_) => p.call("PlayPause", &()).await,
        Action::Stop(_) => p.call("Stop", &()).await,
        Action::Previous(_) => p.call("Previous", &()).await,
        Action::Next(_) => p.call("Next", &()).await,
        Action::Seek(s) => {
            let track: OwnedObjectPath = s
                .track_id
                .clone()
                .try_into()
                .map_err(|e| Failure::new("invalid", format!("{e}")))?;
            p.call("SetPosition", &(track, (s.position_ms * 1000) as i64))
                .await
        }
        Action::Volume { value } => p.set_property("Volume", value).await.map_err(Into::into),
        Action::Repeat { mode } => p
            .set_property(
                "LoopStatus",
                match mode {
                    Repeat::Off => "None",
                    Repeat::All => "Playlist",
                    Repeat::One => "Track",
                },
            )
            .await
            .map_err(Into::into),
        Action::Shuffle { enabled } => p.set_property("Shuffle", enabled).await.map_err(Into::into),
    };
    result.map_err(native)
}
fn native(error: impl std::fmt::Display) -> Failure {
    Failure::new("native", error.to_string())
}

pub async fn run(
    connection: Connection,
    state: watch::Sender<Snapshot>,
    capture: watch::Sender<Option<CaptureIdentity>>,
    mut commands: mpsc::Receiver<CommandRequest>,
) -> anyhow::Result<()> {
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .build();
    let mut events = zbus::MessageStream::for_match_rule(rule, &connection, Some(1024)).await?;
    let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
    let mut observations: BTreeMap<String, Observation> = BTreeMap::new();
    let mut policy = FollowPolicy::default();
    let mut failures = BTreeMap::<String, String>::new();
    let mut startup = true;
    let mut clock = tokio::time::interval(Duration::from_millis(250));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let pending = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let serial = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    loop {
        let mut refresh = false;
        let mut discovery_only = false;
        tokio::select! {
            _ = clock.tick() => { refresh = true; }
            request = commands.recv() => {
                let Some(request) = request else { return Ok(()); };
                match pending.clone().try_acquire_owned() {
                    Err(_) => { let _ = request.reply.send(Err(Failure::new("busy", "Native command queue is full"))); },
                    Ok(permit) => {
                        let connection = connection.clone(); let snapshot = state.subscribe(); let serial = serial.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            let _serial = serial.lock().await;
                            // Do not execute a gesture after its client timed out or disconnected.
                            if request.reply.is_closed() { return; }
                            let followed = || match &snapshot.borrow().player { PlayerState::Following(p) => Some(p.target.clone()), _ => None };
                            let result = execute_fenced(&connection, followed, &request.command).await;
                            let _ = request.reply.send(result);
                        });
                    }
                }
            }
            event = events.next() => {
                let Some(event) = event else { anyhow::bail!("D-Bus signal stream ended"); };
                let event = event?;
                let header = event.header();
                if header.interface().is_some_and(|i| i.as_str() == "org.freedesktop.DBus") && header.member().is_some_and(|m| m.as_str() == "NameOwnerChanged") {
                    let Ok((name, _, _)) = event.body().deserialize::<(String, String, String)>() else { continue; };
                    if name.starts_with("org.mpris.MediaPlayer2.") && !is_self(&name) {
                        policy.remove(&name); observations.remove(&name); failures.remove(&name);
                        refresh = true; discovery_only = true;
                    }
                } else if header.interface().is_some_and(|i| i.as_str() == "org.freedesktop.DBus.Properties") && header.member().is_some_and(|m| m.as_str() == "PropertiesChanged") && header.path().is_some_and(|p| p.as_str() == PATH) {
                    let Ok((interface, changed, _)) = event.body().deserialize::<(String, Properties, Vec<String>)>() else { continue; };
                    if interface == PLAYER {
                        let sender = header.sender().map(|s| s.as_str());
                        for observation in observations.values_mut().filter(|o| Some(o.player.target.owner.as_str()) == sender) {
                            if let Some(status) = string(&changed, "PlaybackStatus") {
                                observation.player.transport = transport(Some(&status));
                                policy.observe(observation.player.target.clone(), observation.player.transport, false);
                            }
                        }
                        // Other properties refresh on the next tick. No command mutates this table.
                    }
                } else if header.interface().is_some_and(|i| i.as_str() == PLAYER) && header.member().is_some_and(|m| m.as_str() == "Seeked") && header.path().is_some_and(|p| p.as_str() == PATH)
                    && let Ok((position,)) = event.body().deserialize::<(i64,)>() {
                        let sender = header.sender().map(|s| s.as_str());
                        for observation in observations.values_mut().filter(|o| Some(o.player.target.owner.as_str()) == sender) {
                            observation.player.position_ms = u64::try_from(position).ok().map(|p| p / 1000);
                            if policy.followed() == Some(&observation.player.target) {
                                connection.emit_signal(None::<&str>, PATH, PLAYER, "Seeked", &(position,)).await?;
                            }
                        }
                    }
            }
        }
        if refresh {
            let mut names = dbus
                .list_names()
                .await?
                .into_iter()
                .map(|n| n.to_string())
                .filter(|n| n.starts_with("org.mpris.MediaPlayer2.") && !is_self(n))
                .collect::<Vec<_>>();
            names.sort();
            failures.retain(|name, _| names.contains(name));
            observations.retain(|name, _| {
                let keep = names.contains(name);
                if !keep {
                    policy.remove(name);
                }
                keep
            });
            for name in names {
                let owner = match dbus.get_name_owner(name.as_str().try_into()?).await {
                    Ok(v) => v.to_string(),
                    Err(_) => {
                        policy.remove(&name);
                        observations.remove(&name);
                        continue;
                    }
                };
                if connection
                    .unique_name()
                    .is_some_and(|n| n.as_str() == owner)
                {
                    policy.remove(&name);
                    observations.remove(&name);
                    continue;
                }
                if discovery_only
                    && observations
                        .get(&name)
                        .is_some_and(|o| o.player.target.owner == owner)
                {
                    continue;
                }
                match observe(
                    &connection,
                    Target {
                        bus_name: name.clone(),
                        owner,
                    },
                )
                .await
                {
                    Ok(observation) => {
                        failures.remove(&name);
                        policy.snapshot(
                            observation.player.target.clone(),
                            observation.player.transport,
                            startup,
                        );
                        observations.insert(name, observation);
                    }
                    Err(error) => {
                        tracing::debug!(%name, %error, "Cannot observe MPRIS service");
                        policy.remove(&name);
                        observations.remove(&name);
                        failures.insert(name, error.to_string());
                    }
                }
            }
            startup = false;
        }
        let selected = policy
            .followed()
            .and_then(|t| observations.get(&t.bus_name));
        let player = selected.map_or_else(
            || {
                failures.first_key_value().map_or_else(
                    || PlayerState::Idle(Empty {}),
                    |(name, error)| {
                        PlayerState::Unavailable(Failure::new(
                            "mpris",
                            format!("Cannot observe {name}: {error}"),
                        ))
                    },
                )
            },
            |o| PlayerState::Following(Box::new(o.player.clone())),
        );
        let next_capture = selected.map(|o| {
            let mut identity = o.capture.clone();
            identity.shared_process = observations
                .values()
                .filter(|other| other.capture.pid == identity.pid)
                .count()
                > 1;
            identity
        });
        capture.send_if_modified(|old| {
            if *old == next_capture {
                false
            } else {
                *old = next_capture;
                true
            }
        });
        state.send_if_modified(|old| {
            if old.player == player {
                false
            } else {
                old.revision += 1;
                old.player = player;
                true
            }
        });
    }
}

pub fn metadata(player: Option<&Following>) -> HashMap<&'static str, Value<'static>> {
    let mut m = HashMap::new();
    if let Some(p) = player {
        m.insert("xesam:title", Value::from(p.track.title.clone()));
        m.insert("xesam:artist", Value::from(vec![p.track.artist.clone()]));
        if let Some(id) = p
            .track
            .id
            .as_ref()
            .and_then(|id| OwnedObjectPath::try_from(id.clone()).ok())
        {
            m.insert("mpris:trackid", Value::from(id));
        }
        if let Some(d) = p
            .track
            .duration_ms
            .and_then(|v| v.checked_mul(1000))
            .and_then(|v| i64::try_from(v).ok())
        {
            m.insert("mpris:length", Value::from(d));
        }
        if let Some(v) = &p.track.album {
            m.insert("xesam:album", Value::from(v.clone()));
        }
        if let Some(v) = &p.track.artwork_url {
            m.insert("mpris:artUrl", Value::from(v.clone()));
        }
    }
    m
}

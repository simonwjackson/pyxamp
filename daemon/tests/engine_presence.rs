//! Pyxamp's own MPRIS for own playback, and the own-stream capture identity.
//! Run under `dbus-run-session` (private bus), like the step-1 MPRIS tests.
mod engine_support;
use engine_support::Harness;
use pyxamp_daemon::{
    engine::{self, mpv},
    mpris,
    presence::{self, own_track_id},
    protocol::*,
    session::RequestEnvelope,
};
use std::{collections::HashMap, time::Duration};
use tokio::sync::{mpsc, watch};
use zbus::{
    Connection,
    zvariant::{ObjectPath, OwnedValue},
};

fn own() -> OwnPlayback {
    OwnPlayback {
        source: SourceId::Ossicle,
        output: Output {
            id: "local".into(),
            name: "This computer".into(),
            local: true,
            active: true,
        },
        track: Track {
            title: "Episode 12".into(),
            artist: "A Show".into(),
            duration_ms: Some(600_000),
            ..Track::default()
        },
        transport: Transport::Playing,
        capabilities: Capabilities {
            play: true,
            pause: true,
            stop: true,
            previous: true,
            next: true,
            seek: true,
            volume: true,
            repeat: false,
            shuffle: false,
        },
        occurrence: Some("up-next/42".into()),
        position_ms: Some(30_000),
        volume: Some(0.5),
        repeat: None,
        shuffle: None,
        speed: Some(1.5),
        skip: None,
        retry: None,
        yielded: None,
        note: None,
    }
}

fn following() -> PlayerState {
    PlayerState::Following(Box::new(Following {
        target: Target {
            bus_name: "org.mpris.MediaPlayer2.firefox".into(),
            owner: ":1.99".into(),
        },
        identity: "Firefox".into(),
        track: Track {
            title: "Video".into(),
            ..Track::default()
        },
        transport: Transport::Paused,
        capabilities: Capabilities::default(),
        position_ms: None,
        volume: None,
        repeat: None,
        shuffle: None,
        rate: None,
    }))
}

/// Answers one own-playback request from MPRIS and returns it.
async fn answer(requests: &mut mpsc::Receiver<RequestEnvelope>) -> Request {
    let envelope = tokio::time::timeout(Duration::from_secs(3), requests.recv())
        .await
        .expect("no request from MPRIS")
        .unwrap();
    let _ = envelope.reply.send(Ok(None));
    envelope.request
}

#[tokio::test]
async fn own_mpris_reflects_and_controls_own_playback_until_a_foreign_player_is_followed() {
    let snapshot = Snapshot {
        own: Some(own()),
        ..Snapshot::default()
    };
    let (state_tx, state) = watch::channel(snapshot);
    let (commands, mut forwarded) = mpsc::channel(4);
    let (requests, mut received) = mpsc::channel(4);
    let _service = zbus::connection::Builder::session()
        .expect("Run with dbus-run-session")
        .name("org.mpris.MediaPlayer2.pyxamp")
        .unwrap()
        .serve_at(mpris::PATH, presence::Root)
        .unwrap()
        .serve_at(
            mpris::PATH,
            presence::Player {
                state,
                commands,
                requests,
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &client,
        "org.mpris.MediaPlayer2.pyxamp",
        mpris::PATH,
        mpris::PLAYER,
    )
    .await
    .unwrap();
    let fresh = || async {
        // Properties are read fresh each time: the proxy cache is bypassed with Get.
        let p = zbus::fdo::PropertiesProxy::builder(&client)
            .destination("org.mpris.MediaPlayer2.pyxamp")
            .unwrap()
            .path(mpris::PATH)
            .unwrap()
            .build()
            .await
            .unwrap();
        p.get_all(mpris::PLAYER.try_into().unwrap()).await.unwrap()
    };
    let props: HashMap<String, OwnedValue> = fresh().await;
    let text = |p: &HashMap<String, OwnedValue>, key: &str| -> String {
        String::try_from(p[key].try_clone().unwrap()).unwrap()
    };
    assert_eq!(text(&props, "PlaybackStatus"), "Playing");
    assert_eq!(f64::try_from(&props["Rate"]).unwrap(), 1.5);
    assert_eq!(f64::try_from(&props["MinimumRate"]).unwrap(), 0.5);
    assert_eq!(f64::try_from(&props["MaximumRate"]).unwrap(), 3.0);
    assert_eq!(f64::try_from(&props["Volume"]).unwrap(), 0.5);
    assert!(bool::try_from(&props["CanSeek"]).unwrap());
    assert_eq!(i64::try_from(&props["Position"]).unwrap(), 30_000_000);
    let metadata =
        HashMap::<String, OwnedValue>::try_from(props["Metadata"].try_clone().unwrap()).unwrap();
    assert_eq!(text(&metadata, "xesam:title"), "Episode 12");
    let track_id = own_track_id("up-next/42");
    assert_eq!(
        <&ObjectPath<'_>>::try_from(&metadata["mpris:trackid"])
            .unwrap()
            .as_str(),
        track_id
    );

    // Media keys act on own playback through the session, as user actions.
    let call = tokio::spawn({
        let proxy = proxy.clone();
        async move { proxy.call::<_, _, ()>("PlayPause", &()).await }
    });
    let request = answer(&mut received).await;
    call.await.unwrap().unwrap();
    assert!(request.id.starts_with("mpris:"));
    assert!(matches!(
        request.action,
        SourceAction::Transport {
            action: OwnTransport::PlayPause
        }
    ));
    let call = tokio::spawn({
        let proxy = proxy.clone();
        let path = ObjectPath::try_from(track_id.clone()).unwrap().into_owned();
        async move {
            proxy
                .call::<_, _, ()>("SetPosition", &(path, 90_000_000i64))
                .await
        }
    });
    let request = answer(&mut received).await;
    call.await.unwrap().unwrap();
    assert!(
        matches!(&request.action, SourceAction::Seek { position_ms: 90_000, occurrence } if occurrence == "up-next/42"),
        "{:?}",
        request.action
    );
    let call = tokio::spawn({
        let proxy = proxy.clone();
        async move { proxy.call::<_, _, ()>("Seek", &(10_000_000i64,)).await }
    });
    let request = answer(&mut received).await;
    call.await.unwrap().unwrap();
    assert!(matches!(
        request.action,
        SourceAction::Seek {
            position_ms: 40_000,
            ..
        }
    ));
    let call = tokio::spawn({
        let proxy = proxy.clone();
        async move { proxy.set_property("Rate", 2.0f64).await }
    });
    let request = answer(&mut received).await;
    call.await.unwrap().unwrap();
    assert!(matches!(request.action, SourceAction::Speed { rate } if rate == 2.0));

    // A followed foreign player wins: properties and commands go to it.
    state_tx.send_modify(|s| s.player = following());
    let props = fresh().await;
    assert_eq!(text(&props, "PlaybackStatus"), "Paused");
    let call = tokio::spawn({
        let proxy = proxy.clone();
        async move { proxy.call::<_, _, ()>("Next", &()).await }
    });
    let command = tokio::time::timeout(Duration::from_secs(3), forwarded.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        command.command.target.bus_name,
        "org.mpris.MediaPlayer2.firefox"
    );
    let _ = command.reply.send(Ok(()));
    call.await.unwrap().unwrap();
    assert!(
        received.try_recv().is_err(),
        "own playback got a foreign command"
    );

    // Nothing at all: stopped, and own controls are not invented.
    state_tx.send_modify(|s| {
        s.player = PlayerState::Idle(Empty {});
        s.own = None;
    });
    let props = fresh().await;
    assert_eq!(text(&props, "PlaybackStatus"), "Stopped");
    assert!(!bool::try_from(&props["CanControl"]).unwrap());
}

#[tokio::test]
async fn own_capture_targets_only_the_engine_mpv_while_own_plays_here() {
    let connection = Connection::session()
        .await
        .expect("Run with dbus-run-session");
    let mut snapshot = Snapshot {
        own: Some(own()),
        ..Snapshot::default()
    };
    // No engine running: unknown stream, so no capture identity.
    assert!(engine::own_capture(&snapshot, &connection).is_none());
    let h = Harness::start_realtime();
    h.ready().await;
    let pid = mpv::engine_pid().unwrap();
    let identity = engine::own_capture(&snapshot, &connection).unwrap();
    assert_eq!(identity.pid, pid);
    assert_eq!(identity.target.bus_name, "org.mpris.MediaPlayer2.pyxamp");
    assert!(identity.target.validate().is_ok());
    assert!(engine::is_engine_process(pid));
    assert!(!engine::is_engine_process(std::process::id()));
    snapshot.player = following();
    assert!(engine::own_capture(&snapshot, &connection).is_none());
    snapshot.player = PlayerState::Idle(Empty {});
    snapshot.own.as_mut().unwrap().transport = Transport::Paused;
    assert!(engine::own_capture(&snapshot, &connection).is_none());
    snapshot.own.as_mut().unwrap().transport = Transport::Playing;
    snapshot.own.as_mut().unwrap().output.local = false;
    assert!(engine::own_capture(&snapshot, &connection).is_none());
    h.finish().await;
}

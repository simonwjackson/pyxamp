use pyxamp_daemon::{mpris, protocol::*};
struct ReadOnlyPlayer;
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl ReadOnlyPlayer {
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        0.4
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn loop_status(&self) -> &str {
        "Playlist"
    }
    #[zbus(property)]
    fn playback_status(&self) -> &str {
        "Playing"
    }
}
struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Read-only session player"
    }
}

#[tokio::test]
async fn read_only_native_properties_are_not_advertised_as_writable_capabilities() {
    let connection = zbus::connection::Builder::session()
        .unwrap()
        .name("org.mpris.MediaPlayer2.readonly")
        .unwrap()
        .serve_at(mpris::PATH, ReadOnlyPlayer)
        .unwrap()
        .serve_at(mpris::PATH, Root)
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = zbus::Connection::session().await.unwrap();
    let target = Target {
        bus_name: "org.mpris.MediaPlayer2.readonly".into(),
        owner: connection.unique_name().unwrap().to_string(),
    };
    let observed = mpris::observe(&client, target.clone())
        .await
        .unwrap()
        .player;
    assert_eq!(observed.volume, Some(0.4));
    assert_eq!(observed.repeat, Some(Repeat::All));
    assert_eq!(observed.shuffle, Some(false));
    let c = observed.capabilities;
    assert!(!c.volume && !c.repeat && !c.shuffle && !c.seek && !c.play && !c.next && !c.previous);
    assert!(c.pause && c.stop);
    let command = Command {
        id: "volume".into(),
        target: target.clone(),
        action: Action::Volume { value: 0.2 },
    };
    assert_eq!(
        mpris::execute(&client, Some(&target), &command)
            .await
            .unwrap_err()
            .code,
        "unsupported"
    );
}

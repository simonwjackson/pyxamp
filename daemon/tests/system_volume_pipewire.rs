//! Run only through daemon/tools/test-system-volume: needs an isolated PipeWire and
//! WirePlumber with the null sinks `pyxamp-test-sink` (default) and `pyxamp-test-sink-2`.
//! Never run it against a user's audio server: it changes the default sink's volume.
use pyxamp_daemon::{protocol::SystemVolume, system_volume};
use std::{
    process::Command,
    time::{Duration, Instant},
};
use tokio::sync::watch;

/// `wpctl get-volume @DEFAULT_AUDIO_SINK@`: "Volume: 0.40" or "Volume: 0.40 [MUTED]".
fn wpctl_reading() -> SystemVolume {
    let out = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    let volume = text
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("unexpected wpctl output {text:?}"));
    SystemVolume {
        volume,
        muted: text.contains("[MUTED]"),
    }
}

/// What a volume key does: another process changes the default sink.
fn wpctl(args: &[&str]) {
    let status = Command::new("wpctl").args(args).status().unwrap();
    assert!(status.success(), "wpctl {args:?}");
}

/// The PipeWire id of a node, from pw-dump.
fn node_id(name: &str) -> u32 {
    let out = Command::new("pw-dump").output().unwrap();
    let objects: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    objects
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["info"]["props"]["node.name"] == name)
        .and_then(|o| o["id"].as_u64())
        .unwrap_or_else(|| panic!("no node {name}")) as u32
}

/// Waits until the reader reports a reading that `matches`; returns it and how long it took.
async fn until(
    readings: &mut watch::Receiver<Option<SystemVolume>>,
    what: &str,
    matches: impl Fn(&SystemVolume) -> bool,
) -> (SystemVolume, Duration) {
    let start = Instant::now();
    let reading = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(reading) = readings.borrow_and_update().clone()
                && matches(&reading)
            {
                return reading;
            }
            readings.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the reader never showed {what}; last {:?}",
            readings.borrow()
        )
    });
    (reading, start.elapsed())
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.006
}

#[tokio::test]
#[ignore = "requires isolated PipeWire and WirePlumber; run daemon/tools/test-system-volume"]
async fn reads_follows_and_sets_the_default_sink_volume() {
    // A volume the reader must find, and must not change by starting.
    wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", "0.77"]);
    wpctl(&["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]);
    let (output, mut readings) = watch::channel(None);
    let _thread = system_volume::spawn(output);
    let (first, took) = until(&mut readings, "the start volume", |r| near(r.volume, 0.77)).await;
    eprintln!("read {first:?} in {took:?}");
    assert!(!first.muted);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        near(wpctl_reading().volume, 0.77),
        "starting the reader changed nothing"
    );

    // An external change (what the volume keys run) is followed.
    // Timed from before the key's wpctl runs, so its own run time counts too.
    let pressed = Instant::now();
    wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", "0.3"]);
    until(&mut readings, "the keys' 0.3", |r| near(r.volume, 0.3)).await;
    let took = pressed.elapsed();
    eprintln!("followed an external set-volume {took:?} after the key");
    assert!(took < Duration::from_millis(500), "took {took:?}");
    wpctl(&["set-mute", "@DEFAULT_AUDIO_SINK@", "1"]);
    let (_, took) = until(&mut readings, "mute", |r| r.muted).await;
    eprintln!("followed an external mute in {took:?}");
    wpctl(&["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]);
    until(&mut readings, "unmute", |r| !r.muted).await;

    // Boost above 100 % is reported as it is.
    wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", "1.2"]);
    until(&mut readings, "boost 1.2", |r| near(r.volume, 1.2)).await;

    // Pyxamp's own set goes to the default sink, and wpctl agrees.
    let control = system_volume::Control::wpctl();
    control.set(0.65).await.unwrap();
    until(&mut readings, "Pyxamp's 0.65", |r| near(r.volume, 0.65)).await;
    assert!(near(wpctl_reading().volume, 0.65));

    // Another default sink: the reading follows the new default.
    let second = node_id("pyxamp-test-sink-2").to_string();
    wpctl(&["set-volume", &second, "0.15"]);
    wpctl(&["set-default", &second]);
    let (_, took) = until(&mut readings, "the new default sink", |r| {
        near(r.volume, 0.15)
    })
    .await;
    eprintln!("followed a default-sink change in {took:?}");
    control.set(0.5).await.unwrap();
    until(&mut readings, "0.5 on the new default", |r| {
        near(r.volume, 0.5)
    })
    .await;
    let first_id = node_id("pyxamp-test-sink").to_string();
    let out = Command::new("wpctl")
        .args(["get-volume", &first_id])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("0.65"),
        "the old sink kept its volume: {out:?}"
    );
}

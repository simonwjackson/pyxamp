use pyxamp_daemon::{
    capture::{AudioNode, CaptureIdentity, select_nodes},
    protocol::Target,
};
use std::{
    collections::BTreeMap,
    process::{Child, Command},
};

fn browser() -> CaptureIdentity {
    CaptureIdentity {
        target: Target {
            bus_name: "org.mpris.MediaPlayer2.chromium.instance1".into(),
            owner: ":1.2".into(),
        },
        pid: std::process::id(),
        desktop_entry: Some("chromium".into()),
        identity: "Chromium".into(),
        shared_process: false,
    }
}
fn node(id: u32, pid: u32, class: &str) -> AudioNode {
    AudioNode {
        id,
        serial: id.to_string(),
        props: BTreeMap::from([
            ("media.class".into(), class.into()),
            ("application.process.id".into(), pid.to_string()),
            ("application.name".into(), "Chromium".into()),
            ("media.name".into(), "Playback".into()),
        ]),
    }
}
struct ChildProcess(Child);
impl Drop for ChildProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn one_native_browser_child_stream_excludes_unrelated_process_and_sink() {
    // This tests process ancestry policy. Native browser audio is proved separately by browser-probe.
    let child = ChildProcess(Command::new("sleep").arg("30").spawn().unwrap());
    let browser = browser();
    let nodes = vec![
        node(1, child.0.id(), "Stream/Output/Audio"),
        node(2, 1, "Stream/Output/Audio"),
        node(3, browser.pid, "Audio/Sink"),
    ];
    assert_eq!(select_nodes(&browser, &nodes).unwrap(), vec![1]);
}
#[test]
fn two_browser_streams_are_ambiguous_even_if_one_is_corked() {
    let browser = browser();
    let mut second = node(2, browser.pid, "Stream/Output/Audio");
    second.props.insert("pulse.corked".into(), "true".into());
    assert!(
        select_nodes(
            &browser,
            &[node(1, browser.pid, "Stream/Output/Audio"), second]
        )
        .is_err()
    );
}
#[test]
fn one_stream_does_not_disambiguate_multiple_mpris_players_in_one_process() {
    let mut browser = browser();
    browser.shared_process = true;
    assert!(select_nodes(&browser, &[node(1, browser.pid, "Stream/Output/Audio")]).is_err());
}
#[test]
fn conflicting_application_identity_cannot_be_ignored_to_invent_a_singleton() {
    let browser = browser();
    let mut conflicting = node(2, browser.pid, "Stream/Output/Audio");
    conflicting
        .props
        .insert("application.id".into(), "another-app".into());
    assert!(
        select_nodes(
            &browser,
            &[node(1, browser.pid, "Stream/Output/Audio"), conflicting]
        )
        .is_err()
    );
    assert!(select_nodes(&browser, &[node(3, 1, "Stream/Output/Audio")]).is_err());
}

#[test]
fn multiple_stream_isolation_does_not_depend_on_a_browser_name_heuristic() {
    for identity in [
        "Microsoft Edge",
        "Zen",
        "Renamed browser",
        "Native crossfade player",
    ] {
        let mut player = browser();
        player.identity = identity.into();
        player.desktop_entry = None;
        assert!(
            select_nodes(
                &player,
                &[
                    node(1, player.pid, "Stream/Output/Audio"),
                    node(2, player.pid, "Stream/Output/Audio")
                ]
            )
            .is_err(),
            "{identity} must not combine unknown item/tab streams"
        );
    }
}

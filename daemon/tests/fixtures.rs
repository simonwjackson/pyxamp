use pyxamp_daemon::{fixtures, protocol::*};
use serde_json::{Value, json};

fn replay() -> Value {
    json!({"description": "Configured replay", "state": Snapshot::default(), "visualizer": [Visualizer::silent(None, 0, VisualizerStatus::Idle, None)], "visualizerFps": 0})
}

#[test]
fn canonical_payloads_and_declared_bounds_are_both_enforced() {
    let valid = replay();
    assert_eq!(
        fixtures::validate_bytes(&serde_json::to_vec(&valid).unwrap()).unwrap(),
        1
    );
    let mut cases = Vec::new();
    let mut input = valid.clone();
    input["state"]["revision"] = json!(-1);
    cases.push(input);
    let mut input = valid.clone();
    input["state"]["player"]["_tag"] = json!("notAPlayer");
    cases.push(input);
    let mut input = valid.clone();
    input["state"]["queue"] = json!([{}]);
    cases.push(input);
    let mut input = valid.clone();
    input["state"]["library"]["extra"] = json!(true);
    cases.push(input);
    let mut input = valid.clone();
    input["visualizer"][0]["columns"] = json!(vec![0; 74]);
    cases.push(input);
    let mut input = valid.clone();
    input["visualizer"][0]["waveform"] = json!(vec![128; 76]);
    cases.push(input);
    let mut input = valid.clone();
    input["visualizer"][0]["columns"][0] = json!(256);
    cases.push(input);
    let mut input = valid.clone();
    input["visualizer"][0]["sequence"] = json!(0.5);
    cases.push(input);
    let mut input = valid.clone();
    input["visualizer"][0]["waveform"][0] = json!(127);
    cases.push(input);
    let mut input = valid.clone();
    input["visualizer"][0]["status"] = json!("notAStatus");
    cases.push(input);
    let mut input = valid.clone();
    input["visualizerFps"] = json!(61);
    cases.push(input);
    let mut input = valid;
    input["state"]["player"] = serde_json::to_value(PlayerState::Unavailable(Failure::new(
        "native",
        "x".repeat(MAX_LINE),
    )))
    .unwrap();
    cases.push(input);
    for input in cases {
        assert!(
            fixtures::validate_bytes(&serde_json::to_vec(&input).unwrap()).is_err(),
            "Accepted invalid fixture: {input}"
        );
    }
}

#[test]
fn directory_gate_checks_every_json_file_and_identifies_the_failure() {
    let directory = tempfile::tempdir().unwrap();
    let valid = serde_json::to_vec(&replay()).unwrap();
    std::fs::write(directory.path().join("a.json"), &valid).unwrap();
    std::fs::write(directory.path().join("b.json"), &valid).unwrap();
    assert_eq!(
        fixtures::validate_directory(directory.path()).unwrap(),
        (2, 2)
    );
    std::fs::write(directory.path().join("b.json"), b"{}").unwrap();
    let error = fixtures::validate_directory(directory.path()).unwrap_err();
    assert!(error.to_string().contains("b.json"));
    let empty = tempfile::tempdir().unwrap();
    assert!(fixtures::validate_directory(empty.path()).is_err());
}

#[test]
fn runnable_fixture_gate_uses_real_canonical_deserialization() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("valid.json"),
        serde_json::to_vec(&replay()).unwrap(),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyxamp-validate-fixtures"))
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Validated 1 fixtures and 1 visualizer frames")
    );
}

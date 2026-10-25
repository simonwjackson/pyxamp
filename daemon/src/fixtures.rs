//! Replay-file validation through the same serde types that generate the socket schema.
use crate::protocol::{MAX_LINE, Page, ServerMessage, Snapshot, Visualizer};
use anyhow::Context;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReplayFixture {
    #[serde(default, rename = "description")]
    _description: String,
    state: Snapshot,
    visualizer: Vec<Visualizer>,
    #[serde(default)]
    visualizer_fps: Option<u32>,
    /// Step 2: answers the replay server gives to `library.open` and `search`.
    #[serde(default)]
    pages: Vec<Page>,
}

pub fn validate_bytes(bytes: &[u8]) -> anyhow::Result<usize> {
    let fixture: ReplayFixture =
        serde_json::from_slice(bytes).context("Canonical protocol deserialization failed")?;
    fixture
        .state
        .validate()
        .map_err(|e| anyhow::anyhow!("state: {}: {}", e.code, e.message))?;
    anyhow::ensure!(
        serde_json::to_vec(&ServerMessage::State(fixture.state.clone()))?.len() <= MAX_LINE,
        "state exceeds the 1 MiB wire frame limit"
    );
    for (index, frame) in fixture.visualizer.iter().enumerate() {
        frame
            .validate()
            .map_err(|e| anyhow::anyhow!("visualizer[{index}]: {}: {}", e.code, e.message))?;
        anyhow::ensure!(
            serde_json::to_vec(&ServerMessage::Visualizer(frame.clone()))?.len() <= MAX_LINE,
            "visualizer[{index}] exceeds the 1 MiB wire frame limit"
        );
    }
    for (index, page) in fixture.pages.iter().enumerate() {
        anyhow::ensure!(
            serde_json::to_vec(&ServerMessage::Page(page.clone()))?.len() <= MAX_LINE,
            "pages[{index}] exceeds the 1 MiB wire frame limit"
        );
    }
    anyhow::ensure!(
        fixture.visualizer_fps.is_none_or(|fps| fps <= 60),
        "visualizerFps must be in 0..60"
    );
    Ok(fixture.visualizer.len())
}

pub fn validate_directory(path: &Path) -> anyhow::Result<(usize, usize)> {
    let mut files = std::fs::read_dir(path)
        .with_context(|| format!("Cannot read fixture directory {}", path.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    anyhow::ensure!(
        !files.is_empty(),
        "No JSON fixtures found in {}",
        path.display()
    );
    let mut frames = 0;
    for file in &files {
        let bytes =
            std::fs::read(file).with_context(|| format!("Cannot read {}", file.display()))?;
        frames += validate_bytes(&bytes)
            .with_context(|| format!("Invalid replay fixture {}", file.display()))?;
    }
    Ok((files.len(), frames))
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "face/fixtures".into());
    anyhow::ensure!(
        std::env::args().len() <= 2,
        "Usage: pyxamp-validate-fixtures [face/fixtures]"
    );
    let (files, frames) = pyxamp_daemon::fixtures::validate_directory(std::path::Path::new(&path))?;
    println!(
        "Validated {files} fixtures and {frames} visualizer frames through canonical serde and protocol bounds"
    );
    Ok(())
}

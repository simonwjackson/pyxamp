fn main() -> anyhow::Result<()> {
    let schema = pyxamp_daemon::protocol::schema();
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contracts/pyxamp.schema.json");
    match std::env::args().nth(1).as_deref() {
        Some("--write") => std::fs::write(path, schema)?,
        Some("--check") => anyhow::ensure!(
            std::fs::read_to_string(path)? == schema,
            "Protocol schema drift: run cargo run --bin pyxamp-schema -- --write"
        ),
        None => print!("{schema}"),
        _ => anyhow::bail!("Usage: pyxamp-schema [--write|--check]"),
    }
    Ok(())
}

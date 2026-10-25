//! Explicit, test-only native MPRIS fixture. Never installed in the daemon package.
//! Shares its D-Bus implementation with daemon/tests/mpris.rs.
use std::{path::PathBuf, time::Duration};
#[path = "../tests/support/player.rs"]
mod player;

fn write_observation(path: &std::path::Path, player: &player::BusPlayer) -> anyhow::Result<()> {
    let config = player
        .config
        .lock()
        .map_err(|_| anyhow::anyhow!("Fixture state lock failed"))?;
    let observation = serde_json::json!({
        "target": player.target,
        "calls": config.calls,
        "volume": config.volume,
        "status": config.status,
        "track": config.track,
        "repeat": config.repeat,
        "shuffle": config.shuffle,
    });
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&observation)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var("PYXAMP_FIXTURE_SESSION").as_deref() == Ok("1"),
        "Fixture only: use a private dbus-run-session and set PYXAMP_FIXTURE_SESSION=1"
    );
    let mut args = std::env::args().skip(1);
    let mut name = "fixtureFace".to_owned();
    let mut calls_file = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--name" => {
                name = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--name requires a service suffix"))?
            }
            "--calls-file" => {
                calls_file = Some(PathBuf::from(args.next().ok_or_else(|| {
                    anyhow::anyhow!("--calls-file requires a private output path")
                })?))
            }
            _ => {
                anyhow::bail!("Usage: mpris-player-fixture [--name fixtureFace] --calls-file PATH")
            }
        }
    }
    let calls_file = calls_file.ok_or_else(|| anyhow::anyhow!("--calls-file is required"))?;
    let bus_name = format!("org.mpris.MediaPlayer2.{name}");
    zbus::names::WellKnownName::try_from(bus_name.as_str())?;
    let player = player::BusPlayer::start(&name, "Playing").await;
    player.status("Playing").await;
    write_observation(&calls_file, &player)?;
    println!("{}", serde_json::to_string(&player.target)?);
    let mut clock = tokio::time::interval(Duration::from_millis(20));
    loop {
        tokio::select! {
            _ = clock.tick() => write_observation(&calls_file, &player)?,
            result = tokio::signal::ctrl_c() => { result?; break; }
        }
    }
    Ok(())
}

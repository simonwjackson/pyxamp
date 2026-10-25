use pyxamp_daemon::{capture, mpris, presence, protocol::*, socket};
use tokio::sync::{mpsc, watch};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    if std::env::args().len() > 1 {
        anyhow::bail!("Usage: pyxamp-daemon (requires XDG_RUNTIME_DIR and a session D-Bus)");
    }
    let listener = socket::PrivateListener::bind(&socket::runtime_socket()?)?;
    let (state_tx, state) = watch::channel(Snapshot::default());
    let (capture_tx, identities) = watch::channel(None);
    let (frames_tx, frames) =
        watch::channel(Visualizer::silent(None, 0, VisualizerStatus::Idle, None));
    let (command_tx, commands) = mpsc::channel(32);
    let connection = zbus::connection::Builder::session()?
        .method_timeout(std::time::Duration::from_secs(2))
        .name("org.mpris.MediaPlayer2.pyxamp")?
        .serve_at(mpris::PATH, presence::Root)?
        .serve_at(
            mpris::PATH,
            presence::Player {
                state: state.clone(),
                commands: command_tx.clone(),
            },
        )?
        .build()
        .await?;
    let _capture_thread = capture::spawn(identities, frames_tx);
    let mpris_task = mpris::run(connection.clone(), state_tx.clone(), capture_tx, commands);
    let socket_task = socket::run(
        &listener,
        uuid::Uuid::new_v4().to_string(),
        state,
        frames,
        command_tx,
    );
    let publication = presence::publish(connection, state_tx.subscribe());
    tokio::pin!(mpris_task, socket_task, publication);
    tokio::select! {
        result = &mut mpris_task => {
            let failure = result.err().map_or_else(|| "MPRIS discovery stopped".into(), |e| e.to_string());
            state_tx.send_modify(|state| { state.revision += 1; state.player = PlayerState::Unavailable(Failure::new("dbus", failure)); });
            // Give existing clients the unavailable snapshot before shutting down.
            tokio::select! { _ = &mut socket_task => {}, _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {} }
            anyhow::bail!("MPRIS session unavailable");
        },
        result = &mut socket_task => result?,
        result = &mut publication => result?,
        _ = tokio::signal::ctrl_c() => {},
    }
    Ok(())
}

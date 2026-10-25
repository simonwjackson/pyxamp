use pyxamp_daemon::{
    capture, config, engine, mpris, presence, protocol::*, session, socket, sources, system_volume,
};
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
    let (request_tx, requests) = mpsc::channel(32);
    let connection = zbus::connection::Builder::session()?
        .method_timeout(std::time::Duration::from_secs(2))
        .name("org.mpris.MediaPlayer2.pyxamp")?
        .serve_at(mpris::PATH, presence::Root)?
        .serve_at(
            mpris::PATH,
            presence::Player {
                state: state.clone(),
                commands: command_tx.clone(),
                requests: request_tx.clone(),
            },
        )?
        .build()
        .await?;
    let _capture_thread = capture::spawn(identities, frames_tx);
    // The system volume (default sink) is read here and only ever set on request.
    let (system_tx, system_readings) = watch::channel(None);
    let _system_volume_thread = system_volume::spawn(system_tx);
    tokio::spawn(system_volume::publish(system_readings, state_tx.clone()));
    let mpris_task = mpris::run(connection.clone(), state_tx.clone(), capture_tx, commands);

    // Step 2: own engine, configured sources, and the session that routes between them.
    let sources_config = config::load(config::default_path().as_deref()).unwrap_or_else(|error| {
        tracing::error!(%error, "Ignoring malformed sources config");
        config::Sources::default()
    });
    let (engine_handle, engine_ctx) = engine::channels();
    let (active_tx, active) = watch::channel(None);
    let engine_active = active.clone();
    tokio::spawn(async move {
        if let Err(error) = engine::mpv::run(engine_ctx, engine_active).await {
            tracing::error!(%error, "Engine stopped");
        }
    });
    let mut handles = vec![];
    macro_rules! start_source {
        ($config:expr, $id:expr, $run:path) => {
            if let Some(config) = $config {
                let (handle, ctx) =
                    sources::channels($id, config, engine_handle.clone(), active.clone());
                handles.push(handle);
                tokio::spawn(async move {
                    if let Err(error) = $run(ctx).await {
                        tracing::error!(%error, source = ?$id, "Source stopped");
                    }
                });
            }
        };
    }
    start_source!(sources_config.pyxis, SourceId::Pyxis, sources::pyxis::run);
    start_source!(
        sources_config.ossicle,
        SourceId::Ossicle,
        sources::ossicle::run
    );
    start_source!(
        sources_config.jellyfin,
        SourceId::Jellyfin,
        sources::jellyfin::run
    );
    let session_task = session::run(
        handles,
        engine_handle,
        system_volume::Control::wpctl(),
        active_tx,
        state_tx.clone(),
        requests,
    );

    let socket_task = socket::run(
        &listener,
        uuid::Uuid::new_v4().to_string(),
        state,
        frames,
        command_tx,
        request_tx,
    );
    let publication = presence::publish(connection, state_tx.subscribe());
    tokio::pin!(mpris_task, socket_task, publication, session_task);
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
        result = &mut session_task => result?,
        _ = tokio::signal::ctrl_c() => {},
    }
    Ok(())
}

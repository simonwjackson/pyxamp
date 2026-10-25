//! mpv engine. Seam stub: reports unavailable and refuses every request.
//! The `engine` worker replaces this body; keep the `run` signature.
use super::{EngineContext, EngineState};
use crate::protocol::Failure;

pub async fn run(mut ctx: EngineContext) -> anyhow::Result<()> {
    ctx.state
        .send_replace(EngineState::unavailable("own playback is not built yet"));
    while let Some(request) = ctx.requests.recv().await {
        let _ = request
            .reply
            .send(Err(Failure::new("engine", "own playback is not built yet")));
    }
    Ok(())
}

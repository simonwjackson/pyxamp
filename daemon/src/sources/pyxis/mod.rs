//! Pyxis source. Seam stub; the `pyxis` worker replaces this body. Keep the `run` signature.
use super::SourceContext;
use crate::config::PyxisConfig;

pub async fn run(ctx: SourceContext<PyxisConfig>) -> anyhow::Result<()> {
    super::unavailable(ctx, "the Pyxis connector is not built yet").await
}

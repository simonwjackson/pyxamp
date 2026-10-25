//! Jellyfin source. Seam stub; the `jellyfin` worker replaces this body. Keep the `run` signature.
use super::SourceContext;
use crate::config::JellyfinConfig;

pub async fn run(ctx: SourceContext<JellyfinConfig>) -> anyhow::Result<()> {
    super::unavailable(ctx, "the Jellyfin connector is not built yet").await
}

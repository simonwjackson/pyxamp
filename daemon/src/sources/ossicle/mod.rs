//! Ossicle source. Seam stub; the `ossicle` worker replaces this body. Keep the `run` signature.
use super::SourceContext;
use crate::config::OssicleConfig;

pub async fn run(ctx: SourceContext<OssicleConfig>) -> anyhow::Result<()> {
    super::unavailable(ctx, "the Ossicle connector is not built yet").await
}

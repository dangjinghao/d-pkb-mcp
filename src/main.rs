mod args;
mod hash;
mod paths;
mod rg;
mod server;
mod snapshot;
mod staging;
mod tools;

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = args::Args::new().await;
    let root = args.pkb_root;
    let tmp_path = args.tmp_path;
    let addr = args.addr;
    let metadata = tokio::fs::metadata(&root)
        .await
        .with_context(|| format!("Cannot inspect PKB root: {}", root.display()))?;
    anyhow::ensure!(
        metadata.is_dir(),
        "PKB root must be a directory: {}",
        root.display()
    );

    if !args.no_snapshot {
        snapshot::init(&root, &tmp_path)
            .await
            .context("Cannot initialize snapshots")?;
        snapshot::snapshot("init").await?;
    }

    server::run(root, tmp_path, &addr).await
}

mod args;
mod paths;
mod rg;
mod server;
mod tools;

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = args::Args::new().await;
    let root = args.pkb_root;
    let addr = args.addr;
    let metadata = tokio::fs::metadata(&root)
        .await
        .with_context(|| format!("Cannot inspect PKB root: {}", root.display()))?;
    anyhow::ensure!(
        metadata.is_dir(),
        "PKB root must be a directory: {}",
        root.display()
    );

    server::run(root, &addr).await
}

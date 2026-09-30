mod args;
mod constants;
mod downloads;
mod hash;
mod paths;
mod rg;
mod server;
mod staging;
mod tools;
mod transfers;
mod uploads;

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

    server::run(root, tmp_path, &addr, args.transfer_quota_bytes).await
}

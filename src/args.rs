use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Personal knowledge base MCP server")]
pub(crate) struct Args {
    /// Root directory of the personal knowledge base.
    #[arg(long, default_value = ".")]
    pub(crate) pkb_root: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub(crate) addr: String,
    #[arg(long, default_value = "/tmp")]
    pub(crate) tmp_path: PathBuf,
    /// Maximum combined bytes of prepared and active download copies.
    #[arg(long, default_value_t = 1_073_741_824)]
    pub(crate) download_quota_bytes: u64,
}

impl Args {
    pub(crate) async fn new() -> Self {
        let args = Args::parse();
        let root = tokio::fs::canonicalize(&args.pkb_root)
            .await
            .with_context(|| format!("Cannot resolve PKB root: {}", args.pkb_root.display()))
            .unwrap();
        tokio::fs::create_dir_all(&args.tmp_path)
            .await
            .with_context(|| format!("Cannot create temp directory: {}", args.tmp_path.display()))
            .unwrap();

        Args {
            pkb_root: root,
            addr: args.addr,
            tmp_path: args.tmp_path,
            download_quota_bytes: args.download_quota_bytes,
        }
    }
}

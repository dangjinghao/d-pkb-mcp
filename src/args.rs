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
}

impl Args {
    pub(crate) async fn new() -> Self {
        let args = Args::parse();
        let root = tokio::fs::canonicalize(&args.pkb_root)
            .await
            .with_context(|| format!("Cannot resolve PKB root: {}", args.pkb_root.display()))
            .unwrap();
        Args {
            pkb_root: root,
            addr: args.addr,
        }
    }
}

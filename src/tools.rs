//! MCP tool modules and their combined router.

mod create;
mod edit;
mod find;
mod find_rg;
mod list;
mod mkdir;
mod overwrite;
mod read;
mod remove;
mod rename;
mod search;
mod search_rg;
mod snapshot_list;
mod undo;

use rmcp::{ServerHandler, handler::server::router::tool::ToolRouter, tool_handler};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

const DEFAULT_LIMIT: usize = 128;

#[derive(Clone)]
pub(crate) struct PkbManager {
    pkb_root: Arc<PathBuf>,
    tmp_path: Arc<PathBuf>,
    mutex_lock: Arc<Mutex<()>>,
}

impl PkbManager {
    pub(crate) fn new(root: PathBuf, tmp_path: PathBuf) -> Self {
        Self {
            pkb_root: Arc::new(root),
            tmp_path: Arc::new(tmp_path),
            mutex_lock: Arc::new(Mutex::new(())),
        }
    }

    fn tool_router() -> ToolRouter<Self> {
        Self::list_router()
            + Self::read_router()
            + Self::create_router()
            + Self::overwrite_router()
            + Self::edit_router()
            + Self::mkdir_router()
            + Self::rename_router()
            + Self::remove_router()
            + Self::find_router()
            + Self::find_rg_router()
            + Self::search_router()
            + Self::search_rg_router()
            + Self::snapshot_list_router()
            + Self::undo_router()
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for PkbManager {}

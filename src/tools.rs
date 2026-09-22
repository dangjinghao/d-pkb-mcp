//! MCP tool modules and their combined router.

mod edit;
mod find;
mod find_rg;
mod list;
mod mkdir;
mod read;
mod remove;
mod rename;
mod search;
mod search_rg;
mod undo;
mod write;

use rmcp::{ServerHandler, handler::server::router::tool::ToolRouter, tool_handler};
use std::{path::PathBuf, sync::Arc};

const DEFAULT_LIMIT: usize = 1024;

#[derive(Clone)]
pub(crate) struct PkbManager {
    pkb_root: Arc<PathBuf>,
}

impl PkbManager {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            pkb_root: Arc::new(root),
        }
    }

    fn tool_router() -> ToolRouter<Self> {
        Self::list_router()
            + Self::read_router()
            + Self::write_router()
            + Self::edit_router()
            + Self::mkdir_router()
            + Self::rename_router()
            + Self::remove_router()
            + Self::find_router()
            + Self::find_rg_router()
            + Self::search_router()
            + Self::search_rg_router()
            + Self::undo_router()
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for PkbManager {}

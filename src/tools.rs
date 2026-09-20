//! MCP tool modules and their combined router.

mod edit;
mod find;
mod list;
mod mkdir;
mod read;
mod remove;
mod rename;
mod search;
mod undo;
mod write;

use rmcp::{ServerHandler, handler::server::router::tool::ToolRouter, tool_handler};

const DEFAULT_LIMIT: u32 = 1024;

#[derive(Clone)]
pub(crate) struct PkbManager;

impl PkbManager {
    fn tool_router() -> ToolRouter<Self> {
        Self::list_router()
            + Self::read_router()
            + Self::write_router()
            + Self::edit_router()
            + Self::mkdir_router()
            + Self::rename_router()
            + Self::remove_router()
            + Self::find_router()
            + Self::search_router()
            + Self::undo_router()
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for PkbManager {}

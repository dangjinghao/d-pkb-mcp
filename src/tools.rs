//! MCP tool modules and their combined router.

mod bin_read;
mod create;
mod download_link;
mod edit;
mod find;
mod find_rg;
mod list;
mod mkdir;
mod overwrite;
mod read;
mod remove;
mod rename;
mod resource_upload_prepare;
mod search;
mod search_rg;
mod stat;

use rmcp::{ServerHandler, handler::server::router::tool::ToolRouter, tool_handler};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

const DEFAULT_LIMIT: usize = 128;

#[derive(Clone)]
pub(crate) struct PkbManager {
    pkb_root: Arc<PathBuf>,
    tmp_path: Arc<PathBuf>,
    mutex_lock: Arc<Mutex<()>>,
    downloads: Arc<crate::downloads::Downloads>,
    pub(crate) uploads: Arc<crate::uploads::Uploads>,
}

impl PkbManager {
    pub(crate) fn new(
        root: PathBuf,
        tmp_path: PathBuf,
        downloads: Arc<crate::downloads::Downloads>,
        quota: Arc<crate::transfers::TransferQuota>,
    ) -> Self {
        let mutex_lock = Arc::new(Mutex::new(()));
        let uploads = Arc::new(crate::uploads::Uploads::new(
            root.clone(),
            tmp_path.clone(),
            mutex_lock.clone(),
            quota,
        ));
        Self {
            pkb_root: Arc::new(root),
            tmp_path: Arc::new(tmp_path),
            mutex_lock,
            downloads,
            uploads,
        }
    }

    fn tool_router() -> ToolRouter<Self> {
        Self::list_router()
            + Self::bin_read_router()
            + Self::stat_router()
            + Self::download_link_router()
            + Self::resource_upload_prepare_router()
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
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for PkbManager {}

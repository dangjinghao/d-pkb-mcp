use std::path::{Path, PathBuf};

use tokio::fs;

pub(crate) async fn resolve_inside_root(root: &Path, user_path: &str) -> Option<PathBuf> {
    let candidate = if Path::new(user_path).is_absolute() {
        return None;
    } else {
        root.join(user_path)
    };

    let real_path = fs::canonicalize(&candidate).await.ok()?;

    let real_root = fs::canonicalize(root).await.ok()?;

    if real_path.starts_with(&real_root) {
        Some(real_path)
    } else {
        None
    }
}

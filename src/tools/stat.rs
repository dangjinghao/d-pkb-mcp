//! File metadata and a streaming whole-file SHA-256 without returning content.

use std::{io, path::Path};

use chrono::{DateTime, SecondsFormat, Utc};
use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_router,
};
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncReadExt};

use super::PkbManager;
use crate::paths::resolve_inside_root;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct StatParams {
    path: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct StatOutput {
    sha256: String,
    full_size: u64,
    mtime: String,
}

async fn stat_file(path: &Path) -> io::Result<StatOutput> {
    let mut file = File::open(path).await?;
    let metadata = file.metadata().await?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must be a regular file",
        ));
    }
    let modified = metadata.modified()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 65_536];
    let mut bytes_read = 0u64;
    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes_read += count as u64;
    }
    let after = file.metadata().await?;
    if bytes_read != metadata.len()
        || after.len() != metadata.len()
        || after.modified()? != modified
    {
        return Err(io::Error::other(
            "file changed while computing SHA-256; retry stat",
        ));
    }
    Ok(StatOutput {
        sha256: format!("{:x}", hasher.finalize()),
        full_size: metadata.len(),
        mtime: DateTime::<Utc>::from(modified).to_rfc3339_opts(SecondsFormat::AutoSi, true),
    })
}

#[tool_router(router = stat_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Return {sha256, full_size, mtime} for the regular file at <path> without returning file content. Paths are literal and relative to the PKB root; reject paths outside it. sha256 is the lowercase hexadecimal SHA-256 of the entire file's raw bytes, full_size is the total byte count, and mtime is the modification time as a UTC RFC 3339 string. Works for text and binary files. Computing SHA-256 scans the whole file with bounded memory and is serialized with MCP writes. Use sha256 as if_hash for conditional writes; mtime is not a version identifier. When modifying content read with bin_read, obtain stat first and keep that original sha256 for the final write. Missing files, directories, and filesystem failures are tool errors.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<StatOutput>()
    )]
    async fn stat(
        &self,
        Parameters(StatParams { path }): Parameters<StatParams>,
    ) -> Result<CallToolResult, String> {
        let target = resolve_inside_root(&self.pkb_root, &path).ok_or("Unsupported <path>")?;
        let _guard = self.mutex_lock.lock().await;
        let output = stat_file(&target)
            .await
            .map_err(|error| error.to_string())?;
        let value = rmcp::serde_json::to_value(output).map_err(|error| error.to_string())?;
        Ok(CallToolResult::structured(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{downloads::Downloads, hash::sha256_hex};
    use std::sync::Arc;

    #[tokio::test]
    async fn hashes_binary_content_across_buffers_and_returns_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary");
        let bytes: Vec<u8> = (0..150_001).map(|index| index as u8).collect();
        tokio::fs::write(&path, &bytes).await.unwrap();
        let modified = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .modified()
            .unwrap();
        let output = stat_file(&path).await.unwrap();
        assert_eq!(output.sha256, sha256_hex(&bytes));
        assert_eq!(output.full_size, bytes.len() as u64);
        assert_eq!(
            DateTime::parse_from_rfc3339(&output.mtime)
                .unwrap()
                .with_timezone(&Utc),
            DateTime::<Utc>::from(modified)
        );
        assert!(output.mtime.ends_with('Z'));
    }

    #[tokio::test]
    async fn supports_empty_and_text_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        tokio::fs::write(&path, b"").await.unwrap();
        let empty = stat_file(&path).await.unwrap();
        assert_eq!(
            empty.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(empty.full_size, 0);
        tokio::fs::write(&path, "hello\r\n你好\n").await.unwrap();
        let text = stat_file(&path).await.unwrap();
        assert_eq!(text.sha256, sha256_hex("hello\r\n你好\n".as_bytes()));
        assert_eq!(text.full_size, "hello\r\n你好\n".len() as u64);
    }

    #[tokio::test]
    async fn rejects_outside_paths_directories_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let manager = PkbManager::new(root.clone(), root, Arc::new(Downloads::new(1).unwrap()));
        for path in ["../outside", "/etc/passwd", ".", "missing"] {
            assert!(
                manager
                    .stat(Parameters(StatParams { path: path.into() }))
                    .await
                    .is_err(),
                "{path}"
            );
        }
    }
}

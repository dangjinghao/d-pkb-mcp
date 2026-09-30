//! Bounded binary reads, encoded as base64 for MCP transport.

use std::{io, path::Path};

use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_router,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};

use super::PkbManager;
use crate::paths::resolve_inside_root;

const MAX_READ_SIZE: usize = 65_536;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BinReadParams {
    path: String,
    start: u64,
    size: usize,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct BinReadOutput {
    base64: String,
    size: usize,
}

async fn read_range(path: &Path, start: u64, size: usize) -> io::Result<BinReadOutput> {
    if size > MAX_READ_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "size must be at most 65536 bytes; use download_link for a full download",
        ));
    }
    let mut file = File::open(path).await?;
    let metadata = file.metadata().await?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must be a regular file",
        ));
    }
    if start > metadata.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "start is beyond EOF",
        ));
    }
    file.seek(io::SeekFrom::Start(start)).await?;
    let mut bytes = Vec::with_capacity(size);
    file.take(size as u64).read_to_end(&mut bytes).await?;
    Ok(BinReadOutput {
        base64: STANDARD.encode(&bytes),
        size: bytes.len(),
    })
}

#[tool_router(router = bin_read_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Read bytes from the regular file at <path>. Paths are literal and relative to the PKB root; reject paths outside it. <start> is a required zero-based byte offset. <size> is a required byte count from 0 through 65536 (64 KiB), regardless of total file size. Return {base64, size}, where size is the actual byte count before encoding. A range past EOF returns remaining bytes; start at EOF returns empty content; start beyond EOF is an error. Use download_link for complete large-file downloads.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<BinReadOutput>()
    )]
    async fn bin_read(
        &self,
        Parameters(BinReadParams { path, start, size }): Parameters<BinReadParams>,
    ) -> Result<CallToolResult, String> {
        let target = resolve_inside_root(&self.pkb_root, &path).ok_or("Unsupported <path>")?;
        let _guard = self.mutex_lock.lock().await;
        let output = read_range(&target, start, size)
            .await
            .map_err(|error| error.to_string())?;
        let value = rmcp::serde_json::to_value(output).map_err(|error| error.to_string())?;
        Ok(CallToolResult::structured(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_binary_ranges_and_eof() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary");
        tokio::fs::write(&path, [0, 255, 128, 1]).await.unwrap();
        let output = read_range(&path, 1, 2).await.unwrap();
        assert_eq!(STANDARD.decode(output.base64).unwrap(), [255, 128]);
        assert_eq!(output.size, 2);
        assert_eq!(read_range(&path, 3, 5).await.unwrap().size, 1);
        let eof = read_range(&path, 4, 1).await.unwrap();
        assert_eq!(eof.size, 0);
        assert!(eof.base64.is_empty());
        assert!(read_range(&path, 5, 0).await.is_err());
        assert_eq!(read_range(&path, 0, 0).await.unwrap().size, 0);
    }

    #[tokio::test]
    async fn limits_each_request_not_file_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large");
        tokio::fs::write(&path, vec![255; MAX_READ_SIZE + 1])
            .await
            .unwrap();
        assert_eq!(
            read_range(&path, 0, MAX_READ_SIZE).await.unwrap().size,
            MAX_READ_SIZE
        );
        assert!(read_range(&path, 0, MAX_READ_SIZE + 1).await.is_err());
        assert!(read_range(dir.path(), 0, 1).await.is_err());
        assert!(read_range(&dir.path().join("missing"), 0, 1).await.is_err());
    }
}

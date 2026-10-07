//! Staging file content in a temporary file for an atomic replacement.

use std::{fs::Permissions, io, os::unix::fs::PermissionsExt, path::Path};

use tempfile::{Builder, NamedTempFile};
use tokio::{
    fs,
    io::{AsyncRead, AsyncWriteExt},
};

const DEFAULT_FILE_MODE: u32 = 0o666;

pub(crate) async fn stage(
    tmp_path: &Path,
    target: &Path,
    mut reader: impl AsyncRead + Unpin,
) -> io::Result<NamedTempFile> {
    let existing = fs::metadata(target)
        .await
        .ok()
        .filter(|metadata| metadata.is_file());

    let stem: &str = target
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("tmp");
    let prefix = format!("{stem}-");
    let suffix = target
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();

    let mut builder = Builder::new();
    builder.prefix(&prefix).suffix(&suffix);
    if existing.is_none() {
        builder.permissions(Permissions::from_mode(DEFAULT_FILE_MODE));
    }
    let temp = builder.tempfile_in(tmp_path)?;

    let mut destination = fs::File::from_std(temp.reopen()?);
    tokio::io::copy(&mut reader, &mut destination).await?;
    destination.flush().await?;
    drop(destination);
    if let Some(metadata) = &existing {
        fs::set_permissions(temp.path(), metadata.permissions()).await?;
    }
    Ok(temp)
}

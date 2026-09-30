//! Expiring download copies. Anonymous files disappear when their last owner drops.

use std::{
    collections::HashMap,
    io,
    os::unix::fs::FileExt,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::{Path as HttpPath, State},
    http::{StatusCode, header},
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::stream;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::constants::{IO_BUFFER_BYTES, TOKEN_BYTES};

const MAX_COPIES: usize = 128;
const EOF_PROBE_BYTES: usize = 1;

#[derive(Default)]
struct Usage {
    bytes: u64,
    copies: usize,
}

struct Reservation {
    usage: Arc<Mutex<Usage>>,
    bytes: u64,
    _transfer: crate::transfers::Reservation,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut usage = self.usage.lock().unwrap();
        usage.bytes -= self.bytes;
        usage.copies -= 1;
    }
}

struct Snapshot {
    file: std::fs::File,
    size: u64,
    filename: String,
    expires: Instant,
    _reservation: Reservation,
}

pub(crate) struct Downloads {
    quota: Arc<crate::transfers::TransferQuota>,
    usage: Arc<Mutex<Usage>>,
    copies: Mutex<HashMap<String, Arc<Snapshot>>>,
}

impl Downloads {
    pub(crate) fn new(quota: Arc<crate::transfers::TransferQuota>) -> Self {
        Self {
            quota,
            usage: Arc::new(Mutex::new(Usage::default())),
            copies: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn prepare(
        self: &Arc<Self>,
        path: &Path,
        tmp_path: &Path,
        secs: u64,
    ) -> io::Result<String> {
        let mut source = File::open(path).await?;
        let metadata = source.metadata().await?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must be a regular file",
            ));
        }
        let size = metadata.len();
        let reservation = {
            let mut usage = self.usage.lock().unwrap();
            if usage.copies >= MAX_COPIES {
                return Err(io::Error::other(
                    "download copy quota exceeded; wait for existing links to expire",
                ));
            }
            let transfer = self.quota.reserve(size).ok_or_else(|| io::Error::other("transfer byte quota exceeded; wait for downloads or uploads to release capacity"))?;
            usage.bytes += size;
            usage.copies += 1;
            Reservation {
                usage: self.usage.clone(),
                bytes: size,
                _transfer: transfer,
            }
        };
        // tempfile_in creates an anonymous file: no path can expose it through PKB tools.
        let temp = tmp_path.to_owned();
        let file = tokio::task::spawn_blocking(move || tempfile::tempfile_in(temp))
            .await
            .map_err(io::Error::other)??;
        let mut destination = File::from_std(file.try_clone()?);
        let copied = tokio::io::copy(&mut (&mut source).take(size), &mut destination).await?;
        let mut extra = [0u8; EOF_PROBE_BYTES];
        if copied != size || source.read(&mut extra).await? != 0 {
            return Err(io::Error::other(
                "source size changed while preparing download",
            ));
        }
        // Flush pending Tokio writes before exposing the copy.
        destination.flush().await?;
        drop(destination);
        let expires = Instant::now() + Duration::from_secs(secs);
        let snapshot = Arc::new(Snapshot {
            file,
            size,
            filename: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            expires,
            _reservation: reservation,
        });
        let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; TOKEN_BYTES]>());
        self.copies.lock().unwrap().insert(token.clone(), snapshot);
        let weak = Arc::downgrade(self);
        let cleanup_token = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep_until(tokio::time::Instant::from_std(expires)).await;
            if let Some(store) = weak.upgrade() {
                store.copies.lock().unwrap().remove(&cleanup_token);
            }
        });
        Ok(format!("/downloads/{token}"))
    }

    fn get(&self, token: &str) -> Option<Arc<Snapshot>> {
        let mut copies = self.copies.lock().unwrap();
        if copies
            .get(token)
            .is_some_and(|copy| Instant::now() >= copy.expires)
        {
            copies.remove(token);
            return None;
        }
        copies.get(token).cloned()
    }
}

pub(crate) async fn download(
    State(store): State<Arc<Downloads>>,
    HttpPath(token): HttpPath<String>,
) -> Result<Response, StatusCode> {
    let copy = store.get(&token).ok_or(StatusCode::NOT_FOUND)?;
    let length = copy.size;
    let filename: String = copy
        .filename
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    // Positional reads give concurrent GETs independent offsets. The stream owns
    // its snapshot, so expiry cannot remove an active download or free its quota.
    let chunks = stream::try_unfold((copy, 0u64), |(copy, offset)| async move {
        if offset >= copy.size {
            return Ok::<_, io::Error>(None);
        }
        let result = tokio::task::spawn_blocking(move || {
            let mut buffer = vec![0; (copy.size - offset).min(IO_BUFFER_BYTES as u64) as usize];
            let count = copy.file.read_at(&mut buffer, offset)?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "download copy truncated",
                ));
            }
            buffer.truncate(count);
            Ok((buffer, copy, offset + count as u64))
        })
        .await
        .map_err(io::Error::other)??;
        Ok(Some((result.0, (result.1, result.2))))
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename*=UTF-8''{filename}"),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::from_stream(chunks))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DEFAULT_LINK_TTL_SECS;
    use axum::body::to_bytes;

    const LARGE_FILE_BYTES: usize = 150_000;
    const LARGE_QUOTA_BYTES: usize = 300_000;
    const BINARY_FILL_BYTE: u8 = 255;
    const SHORT_LINK_TTL_SECS: u64 = 1;
    const EXPIRY_GRACE: Duration = Duration::from_millis(100);

    fn store(bytes: u64) -> Arc<Downloads> {
        Arc::new(Downloads::new(Arc::new(
            crate::transfers::TransferQuota::new(bytes).unwrap(),
        )))
    }

    async fn response(store: &Arc<Downloads>, url: &str) -> Result<Response, StatusCode> {
        download(
            State(store.clone()),
            HttpPath(url.rsplit('/').next().unwrap().to_owned()),
        )
        .await
    }

    #[tokio::test]
    async fn snapshots_survive_source_changes_and_concurrent_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("binary file.bin");
        let bytes = vec![BINARY_FILL_BYTE; LARGE_FILE_BYTES];
        tokio::fs::write(&source, &bytes).await.unwrap();
        let store = store(LARGE_QUOTA_BYTES as u64);
        let url = store
            .prepare(&source, dir.path(), DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
        assert!(url.starts_with("/downloads/"));
        assert!(!url.contains("://"));
        tokio::fs::write(&source, b"changed").await.unwrap();
        tokio::fs::remove_file(&source).await.unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let a = response(&store, &url).await.unwrap();
        let b = response(&store, &url).await.unwrap();
        assert_eq!(
            a.headers()[header::CONTENT_LENGTH],
            LARGE_FILE_BYTES.to_string()
        );
        assert!(
            a.headers()[header::CONTENT_DISPOSITION]
                .to_str()
                .unwrap()
                .contains("binary%20file.bin")
        );
        let (a, b) = tokio::join!(
            to_bytes(a.into_body(), LARGE_QUOTA_BYTES),
            to_bytes(b.into_body(), LARGE_QUOTA_BYTES)
        );
        assert_eq!(a.unwrap().as_ref(), bytes);
        assert_eq!(b.unwrap().as_ref(), bytes);
    }

    #[tokio::test]
    async fn expiry_blocks_new_requests_but_active_download_keeps_copy_and_quota() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        tokio::fs::write(&path, b"payload").await.unwrap();
        const CONTENT: &[u8] = b"payload";
        let store = store(CONTENT.len() as u64);
        let url = store
            .prepare(&path, dir.path(), SHORT_LINK_TTL_SECS)
            .await
            .unwrap();
        let active = response(&store, &url).await.unwrap();
        assert!(
            store
                .prepare(&path, dir.path(), SHORT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_secs(SHORT_LINK_TTL_SECS) + EXPIRY_GRACE).await;
        assert_eq!(
            response(&store, &url).await.unwrap_err(),
            StatusCode::NOT_FOUND
        );
        assert!(
            store
                .prepare(&path, dir.path(), SHORT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        assert_eq!(
            to_bytes(active.into_body(), CONTENT.len())
                .await
                .unwrap()
                .as_ref(),
            b"payload"
        );
        assert_eq!(store.usage.lock().unwrap().bytes, 0);
        store
            .prepare(&path, dir.path(), SHORT_LINK_TTL_SECS)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn validates_files_quota_and_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(1);
        assert!(
            store
                .prepare(dir.path(), dir.path(), DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        assert!(
            store
                .prepare(
                    &dir.path().join("missing"),
                    dir.path(),
                    DEFAULT_LINK_TTL_SECS
                )
                .await
                .is_err()
        );
        assert_eq!(
            response(&store, "invalid").await.unwrap_err(),
            StatusCode::NOT_FOUND
        );
        let source = dir.path().join("large");
        tokio::fs::write(&source, b"large").await.unwrap();
        assert!(
            store
                .prepare(&source, dir.path(), DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        assert_eq!(store.usage.lock().unwrap().copies, 0);
    }

    #[tokio::test]
    async fn failed_preparation_releases_quota_and_empty_files_download() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("empty");
        tokio::fs::write(&source, b"").await.unwrap();
        let store = store(1);
        assert!(
            store
                .prepare(&source, &dir.path().join("missing"), DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        assert_eq!(store.usage.lock().unwrap().copies, 0);
        let url = store
            .prepare(&source, dir.path(), DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
        let response = response(&store, &url).await.unwrap();
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        assert!(to_bytes(response.into_body(), 1).await.unwrap().is_empty());
    }
}

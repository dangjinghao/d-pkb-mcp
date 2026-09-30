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

const MAX_COPIES: usize = 128;

#[derive(Default)]
struct Usage {
    bytes: u64,
    copies: usize,
}

struct Reservation {
    usage: Arc<Mutex<Usage>>,
    bytes: u64,
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
    quota: u64,
    usage: Arc<Mutex<Usage>>,
    copies: Mutex<HashMap<String, Arc<Snapshot>>>,
}

impl Downloads {
    pub(crate) fn new(quota: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(quota > 0, "download-quota-bytes must be positive");
        Ok(Self {
            quota,
            usage: Arc::new(Mutex::new(Usage::default())),
            copies: Mutex::new(HashMap::new()),
        })
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
            if usage.copies >= MAX_COPIES || size > self.quota.saturating_sub(usage.bytes) {
                return Err(io::Error::other(
                    "download copy quota exceeded; wait for existing links to expire",
                ));
            }
            usage.bytes += size;
            usage.copies += 1;
            Reservation {
                usage: self.usage.clone(),
                bytes: size,
            }
        };
        // tempfile_in creates an anonymous file: no path can expose it through PKB tools.
        let temp = tmp_path.to_owned();
        let file = tokio::task::spawn_blocking(move || tempfile::tempfile_in(temp))
            .await
            .map_err(io::Error::other)??;
        let mut destination = File::from_std(file.try_clone()?);
        let copied = tokio::io::copy(&mut (&mut source).take(size), &mut destination).await?;
        let mut extra = [0u8; 1];
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
        let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
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
            let mut buffer = vec![0; (copy.size - offset).min(65_536) as usize];
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
    use axum::body::to_bytes;

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
        let bytes = vec![255; 150_000];
        tokio::fs::write(&source, &bytes).await.unwrap();
        let store = Arc::new(Downloads::new(300_000).unwrap());
        let url = store.prepare(&source, dir.path(), 300).await.unwrap();
        assert!(url.starts_with("/downloads/"));
        assert!(!url.contains("://"));
        tokio::fs::write(&source, b"changed").await.unwrap();
        tokio::fs::remove_file(&source).await.unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let a = response(&store, &url).await.unwrap();
        let b = response(&store, &url).await.unwrap();
        assert_eq!(a.headers()[header::CONTENT_LENGTH], "150000");
        assert!(
            a.headers()[header::CONTENT_DISPOSITION]
                .to_str()
                .unwrap()
                .contains("binary%20file.bin")
        );
        let (a, b) = tokio::join!(
            to_bytes(a.into_body(), 300_000),
            to_bytes(b.into_body(), 300_000)
        );
        assert_eq!(a.unwrap().as_ref(), bytes);
        assert_eq!(b.unwrap().as_ref(), bytes);
    }

    #[tokio::test]
    async fn expiry_blocks_new_requests_but_active_download_keeps_copy_and_quota() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        tokio::fs::write(&path, b"payload").await.unwrap();
        let store = Arc::new(Downloads::new(7).unwrap());
        let url = store.prepare(&path, dir.path(), 1).await.unwrap();
        let active = response(&store, &url).await.unwrap();
        assert!(store.prepare(&path, dir.path(), 1).await.is_err());
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(
            response(&store, &url).await.unwrap_err(),
            StatusCode::NOT_FOUND
        );
        assert!(store.prepare(&path, dir.path(), 1).await.is_err());
        assert_eq!(
            to_bytes(active.into_body(), 10).await.unwrap().as_ref(),
            b"payload"
        );
        assert_eq!(store.usage.lock().unwrap().bytes, 0);
        store.prepare(&path, dir.path(), 1).await.unwrap();
    }

    #[tokio::test]
    async fn validates_files_quota_and_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Downloads::new(1).unwrap());
        assert!(store.prepare(dir.path(), dir.path(), 300).await.is_err());
        assert!(
            store
                .prepare(&dir.path().join("missing"), dir.path(), 300)
                .await
                .is_err()
        );
        assert_eq!(
            response(&store, "invalid").await.unwrap_err(),
            StatusCode::NOT_FOUND
        );
        let source = dir.path().join("large");
        tokio::fs::write(&source, b"large").await.unwrap();
        assert!(store.prepare(&source, dir.path(), 300).await.is_err());
        assert_eq!(store.usage.lock().unwrap().copies, 0);
    }

    #[tokio::test]
    async fn failed_preparation_releases_quota_and_empty_files_download() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("empty");
        tokio::fs::write(&source, b"").await.unwrap();
        let store = Arc::new(Downloads::new(1).unwrap());
        assert!(
            store
                .prepare(&source, &dir.path().join("missing"), 300)
                .await
                .is_err()
        );
        assert_eq!(store.usage.lock().unwrap().copies, 0);
        let url = store.prepare(&source, dir.path(), 300).await.unwrap();
        let response = response(&store, &url).await.unwrap();
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        assert!(to_bytes(response.into_body(), 1).await.unwrap().is_empty());
    }

    #[test]
    fn validates_quota() {
        assert!(Downloads::new(1).is_ok());
        assert!(Downloads::new(0).is_err());
    }
}

//! Token-bound HTTP uploads with bounded storage and atomic conditional commits.

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    io::{AsyncSeekExt, AsyncWriteExt},
};

use crate::{
    constants::{MAX_LINK_TTL_SECS, MIN_LINK_TTL_SECS, TOKEN_BYTES},
    hash::SHA256_HEX_CHARS,
};
use crate::{hash::sha256_large_file, paths::resolve_inside_root, staging::stage};

const MAX_UPLOADS: usize = 128;
const UPLOAD_RECEIVE_TIMEOUT_SECS: u64 = 300;
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(UPLOAD_RECEIVE_TIMEOUT_SECS);

#[derive(Debug)]
pub(crate) struct UploadError {
    status: StatusCode,
    pub(crate) message: String,
}

impl UploadError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<io::Error> for UploadError {
    fn from(error: io::Error) -> Self {
        let status = if error.kind() == io::ErrorKind::NotFound {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self::new(status, error.to_string())
    }
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(rmcp::serde_json::json!({"error": self.message})),
        )
            .into_response()
    }
}

#[derive(Default)]
struct Usage {
    bytes: u64,
    count: usize,
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
        usage.count -= 1;
    }
}

#[derive(PartialEq)]
enum UploadState {
    Pending,
    Active,
    Completed,
}

struct Upload {
    target: PathBuf,
    full_size: u64,
    sha256: String,
    if_hash: Option<String>,
    expires: Instant,
    state: Mutex<UploadState>,
    reservation: Mutex<Option<Reservation>>,
}

// Cancellation, disconnects and errors restore retryability automatically.
struct Attempt(Arc<Upload>);

impl Drop for Attempt {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        if *state == UploadState::Active {
            *state = UploadState::Pending;
        }
    }
}

pub(crate) struct Uploads {
    root: PathBuf,
    tmp_path: PathBuf,
    operation_lock: Arc<tokio::sync::Mutex<()>>,
    quota: Arc<crate::transfers::TransferQuota>,
    usage: Arc<Mutex<Usage>>,
    entries: Mutex<HashMap<String, Arc<Upload>>>,
}

impl Uploads {
    pub(crate) fn new(
        root: PathBuf,
        tmp_path: PathBuf,
        operation_lock: Arc<tokio::sync::Mutex<()>>,
        quota: Arc<crate::transfers::TransferQuota>,
    ) -> Self {
        Self {
            root,
            tmp_path,
            operation_lock,
            quota,
            usage: Arc::new(Mutex::new(Usage::default())),
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn prepare(
        self: &Arc<Self>,
        path: &str,
        full_size: u64,
        sha256: String,
        if_hash: Option<String>,
        secs: u64,
    ) -> Result<String, UploadError> {
        let valid_hash = |hash: &str| {
            hash.len() == SHA256_HEX_CHARS
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        if !valid_hash(&sha256) || if_hash.as_deref().is_some_and(|hash| !valid_hash(hash)) {
            return Err(UploadError::new(
                StatusCode::BAD_REQUEST,
                format!(
                    "sha256 and if_hash must be {SHA256_HEX_CHARS}-character lowercase hexadecimal SHA-256 values"
                ),
            ));
        }
        if !(MIN_LINK_TTL_SECS..=MAX_LINK_TTL_SECS).contains(&secs) {
            return Err(UploadError::new(
                StatusCode::BAD_REQUEST,
                format!("secs must be between {MIN_LINK_TTL_SECS} and {MAX_LINK_TTL_SECS}"),
            ));
        }
        let target = resolve_inside_root(&self.root, path)
            .ok_or_else(|| UploadError::new(StatusCode::BAD_REQUEST, "Unsupported <path>"))?;
        if target == self.root {
            return Err(UploadError::new(
                StatusCode::BAD_REQUEST,
                "path must name a file",
            ));
        }
        let _guard = self.operation_lock.lock().await;
        self.check_target(&target, if_hash.as_deref()).await?;
        let mut entries = self.entries.lock().unwrap();
        let mut usage = self.usage.lock().unwrap();
        if entries.len() >= MAX_UPLOADS || usage.count >= MAX_UPLOADS {
            return Err(UploadError::new(
                StatusCode::INSUFFICIENT_STORAGE,
                "upload quota exceeded; wait for pending uploads to finish or expire",
            ));
        }
        let transfer = self.quota.reserve(full_size).ok_or_else(|| {
            UploadError::new(
                StatusCode::INSUFFICIENT_STORAGE,
                "transfer byte quota exceeded; wait for downloads or uploads to release capacity",
            )
        })?;
        usage.bytes += full_size;
        usage.count += 1;
        let expires = Instant::now() + Duration::from_secs(secs);
        let upload = Arc::new(Upload {
            target,
            full_size,
            sha256,
            if_hash,
            expires,
            state: Mutex::new(UploadState::Pending),
            reservation: Mutex::new(Some(Reservation {
                usage: self.usage.clone(),
                bytes: full_size,
                _transfer: transfer,
            })),
        });
        let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; TOKEN_BYTES]>());
        entries.insert(token.clone(), upload);
        drop(usage);
        drop(entries);
        let weak = Arc::downgrade(self);
        let cleanup_token = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep_until(tokio::time::Instant::from_std(expires)).await;
            if let Some(store) = weak.upgrade() {
                store.entries.lock().unwrap().remove(&cleanup_token);
            }
        });
        Ok(format!("/uploads/{token}"))
    }

    async fn check_target(
        &self,
        target: &std::path::Path,
        if_hash: Option<&str>,
    ) -> Result<(), UploadError> {
        let parent = target
            .parent()
            .ok_or_else(|| UploadError::new(StatusCode::BAD_REQUEST, "missing parent path"))?;
        if !fs::metadata(parent).await?.is_dir() {
            return Err(UploadError::new(
                StatusCode::CONFLICT,
                "parent must be an existing directory",
            ));
        }
        if let Some(expected) = if_hash {
            if !fs::metadata(target).await?.is_file() {
                return Err(UploadError::new(
                    StatusCode::CONFLICT,
                    "overwrite target must be a regular file",
                ));
            }
            let current = sha256_large_file(target).await?;
            if current != expected {
                return Err(UploadError::new(
                    StatusCode::CONFLICT,
                    format!("sha mismatch, current_sha: {current}"),
                ));
            }
        } else {
            match fs::symlink_metadata(target).await {
                Ok(_) => {
                    return Err(UploadError::new(
                        StatusCode::CONFLICT,
                        "create target must not already exist",
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn begin(&self, token: &str) -> Result<Attempt, UploadError> {
        let mut entries = self.entries.lock().unwrap();
        if entries
            .get(token)
            .is_some_and(|entry| Instant::now() >= entry.expires)
        {
            entries.remove(token);
        }
        let entry = entries.get(token).cloned().ok_or_else(|| {
            UploadError::new(StatusCode::NOT_FOUND, "unknown or expired upload token")
        })?;
        {
            let mut state = entry.state.lock().unwrap();
            if *state != UploadState::Pending {
                return Err(UploadError::new(
                    StatusCode::CONFLICT,
                    if *state == UploadState::Completed {
                        "upload already committed; verify the target using stat"
                    } else {
                        "upload already in progress"
                    },
                ));
            }
            *state = UploadState::Active;
        }
        Ok(Attempt(entry))
    }

    async fn receive(&self, entry: &Upload, body: Body) -> Result<fs::File, UploadError> {
        let tmp = self.tmp_path.clone();
        let file = tokio::task::spawn_blocking(move || tempfile::tempfile_in(tmp))
            .await
            .map_err(|error| {
                UploadError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            })??;
        let mut file = fs::File::from_std(file);
        let mut stream = body.into_data_stream();
        let mut received = 0u64;
        let mut hasher = Sha256::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                UploadError::new(
                    StatusCode::BAD_REQUEST,
                    format!("upload body failed: {error}"),
                )
            })?;
            if chunk.len() as u64 > entry.full_size - received {
                return Err(UploadError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "upload exceeds declared full_size",
                ));
            }
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            received += chunk.len() as u64;
        }
        if received != entry.full_size {
            return Err(UploadError::new(
                StatusCode::BAD_REQUEST,
                "upload is shorter than declared full_size",
            ));
        }
        if format!("{:x}", hasher.finalize()) != entry.sha256 {
            return Err(UploadError::new(
                StatusCode::BAD_REQUEST,
                "uploaded content SHA-256 mismatch",
            ));
        }
        file.flush().await?;
        Ok(file)
    }

    async fn commit(&self, attempt: &Attempt, mut file: fs::File) -> Result<(), UploadError> {
        let entry = &attempt.0;
        let _guard = self.operation_lock.lock().await;
        self.check_target(&entry.target, entry.if_hash.as_deref())
            .await?;
        // Anonymous receive files cannot leak through PKB tools. Only stage a
        // named file under the operation lock when the verified upload is ready.
        let temp = stage(&self.tmp_path, &entry.target, b"").await?;
        let mut destination = fs::File::from_std(temp.reopen()?);
        file.seek(io::SeekFrom::Start(0)).await?;
        let copied = tokio::io::copy(&mut file, &mut destination).await?;
        destination.flush().await?;
        if copied != entry.full_size {
            return Err(UploadError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "upload staging size mismatch",
            ));
        }
        drop(destination);
        if entry.if_hash.is_some() {
            temp.persist(&entry.target)
                .map_err(|error| UploadError::from(error.error))?;
        } else {
            temp.persist_noclobber(&entry.target).map_err(|error| {
                if error.error.kind() == io::ErrorKind::AlreadyExists {
                    UploadError::new(StatusCode::CONFLICT, "create target already exists")
                } else {
                    UploadError::from(error.error)
                }
            })?;
        }
        // No await between successful persistence and marking the token used.
        *entry.state.lock().unwrap() = UploadState::Completed;
        entry.reservation.lock().unwrap().take();
        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct UploadOutput {
    after_hash: String,
    full_size: u64,
}

pub(crate) async fn resource_upload(
    State(store): State<Arc<Uploads>>,
    Path(token): Path<String>,
    body: Body,
) -> Result<Json<UploadOutput>, UploadError> {
    let attempt = store.begin(&token)?;
    let file = tokio::time::timeout(RECEIVE_TIMEOUT, store.receive(&attempt.0, body))
        .await
        .map_err(|_| {
            UploadError::new(
                StatusCode::REQUEST_TIMEOUT,
                "upload receive timeout; retry before token expiry",
            )
        })??;
    store.commit(&attempt, file).await?;
    Ok(Json(UploadOutput {
        after_hash: attempt.0.sha256.clone(),
        full_size: attempt.0.full_size,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DEFAULT_LINK_TTL_SECS;
    use crate::hash::sha256_hex;
    use std::os::unix::fs::PermissionsExt;

    const SMALL_QUOTA_BYTES: u64 = 10;
    const LARGE_FILE_BYTES: usize = 150_001;
    const LARGE_QUOTA_BYTES: u64 = 300_000;
    const BODY_CHUNK_BYTES: usize = 8_192;
    const BINARY_FILL_BYTE: u8 = 255;
    const SHORT_LINK_TTL_SECS: u64 = 1;
    const EXPIRY_GRACE: Duration = Duration::from_millis(100);
    const FILE_MODE: u32 = 0o640;
    const FILE_MODE_MASK: u32 = 0o777;

    fn store(dir: &std::path::Path, quota: u64) -> Arc<Uploads> {
        Arc::new(Uploads::new(
            dir.to_owned(),
            dir.to_owned(),
            Arc::new(tokio::sync::Mutex::new(())),
            Arc::new(crate::transfers::TransferQuota::new(quota).unwrap()),
        ))
    }

    fn token(subpath: &str) -> String {
        subpath.rsplit('/').next().unwrap().to_owned()
    }

    async fn put(
        store: &Arc<Uploads>,
        subpath: &str,
        body: Body,
    ) -> Result<Json<UploadOutput>, UploadError> {
        resource_upload(State(store.clone()), Path(token(subpath)), body).await
    }

    #[tokio::test]
    async fn streams_binary_create_and_prevents_token_reuse() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), LARGE_QUOTA_BYTES);
        let bytes = vec![BINARY_FILL_BYTE; LARGE_FILE_BYTES];
        let url = store
            .prepare(
                "binary",
                bytes.len() as u64,
                sha256_hex(&bytes),
                None,
                DEFAULT_LINK_TTL_SECS,
            )
            .await
            .unwrap();
        assert!(url.starts_with("/uploads/"));
        let chunks: Vec<Result<Vec<u8>, io::Error>> = bytes
            .chunks(BODY_CHUNK_BYTES)
            .map(|chunk| Ok(chunk.to_vec()))
            .collect();
        let output = put(
            &store,
            &url,
            Body::from_stream(futures_util::stream::iter(chunks)),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(output.after_hash, sha256_hex(&bytes));
        assert_eq!(output.full_size, bytes.len() as u64);
        assert_eq!(fs::read(dir.path().join("binary")).await.unwrap(), bytes);
        assert_eq!(store.usage.lock().unwrap().bytes, 0);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(
            put(&store, &url, Body::from("again"))
                .await
                .unwrap_err()
                .status,
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn failed_validation_and_disconnect_allow_retry_without_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), SMALL_QUOTA_BYTES);
        let url = store
            .prepare(
                "new",
                b"abc".len() as u64,
                sha256_hex(b"abc"),
                None,
                DEFAULT_LINK_TTL_SECS,
            )
            .await
            .unwrap();
        for (body, expected) in [
            (Body::from("a"), StatusCode::BAD_REQUEST),
            (Body::from("abcd"), StatusCode::PAYLOAD_TOO_LARGE),
            (Body::from("xyz"), StatusCode::BAD_REQUEST),
        ] {
            assert_eq!(put(&store, &url, body).await.unwrap_err().status, expected);
            assert!(!dir.path().join("new").exists());
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
        let chunks = futures_util::stream::iter(vec![
            Ok(b"a".to_vec()),
            Err(io::Error::other("disconnect")),
        ]);
        assert_eq!(
            put(&store, &url, Body::from_stream(chunks))
                .await
                .unwrap_err()
                .status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            put(&store, &url, Body::from("abc"))
                .await
                .unwrap()
                .0
                .after_hash,
            sha256_hex(b"abc")
        );
        assert_eq!(fs::read(dir.path().join("new")).await.unwrap(), b"abc");
    }

    #[tokio::test]
    async fn overwrite_rechecks_hash_and_preserves_permissions() {
        const ORIGINAL: &[u8] = &[255, 0];
        const REPLACEMENT: &[u8] = &[128, 0];
        const CONFLICTING: &[u8] = &[1, 2];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary");
        fs::write(&path, ORIGINAL).await.unwrap();
        fs::set_permissions(&path, std::fs::Permissions::from_mode(FILE_MODE))
            .await
            .unwrap();
        let store = store(dir.path(), SMALL_QUOTA_BYTES);
        let original_hash = sha256_hex(ORIGINAL);
        let url = store
            .prepare(
                "binary",
                REPLACEMENT.len() as u64,
                sha256_hex(REPLACEMENT),
                Some(original_hash.clone()),
                DEFAULT_LINK_TTL_SECS,
            )
            .await
            .unwrap();
        fs::write(&path, CONFLICTING).await.unwrap();
        assert_eq!(
            put(&store, &url, Body::from(REPLACEMENT.to_vec()))
                .await
                .unwrap_err()
                .status,
            StatusCode::CONFLICT
        );
        assert_eq!(fs::read(&path).await.unwrap(), CONFLICTING);
        fs::write(&path, ORIGINAL).await.unwrap();
        assert_eq!(
            put(&store, &url, Body::from(REPLACEMENT.to_vec()))
                .await
                .unwrap()
                .0
                .after_hash,
            sha256_hex(REPLACEMENT)
        );
        assert_eq!(fs::read(&path).await.unwrap(), REPLACEMENT);
        assert_eq!(
            fs::metadata(&path).await.unwrap().permissions().mode() & FILE_MODE_MASK,
            FILE_MODE
        );
    }

    #[tokio::test]
    async fn rechecks_create_and_parent_and_cleans_failed_staging() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), SMALL_QUOTA_BYTES);
        fs::create_dir(dir.path().join("parent")).await.unwrap();
        let url = store
            .prepare(
                "parent/new",
                1,
                sha256_hex(b"x"),
                None,
                DEFAULT_LINK_TTL_SECS,
            )
            .await
            .unwrap();
        fs::remove_dir(dir.path().join("parent")).await.unwrap();
        assert_eq!(
            put(&store, &url, Body::from("x")).await.unwrap_err().status,
            StatusCode::NOT_FOUND
        );
        fs::create_dir(dir.path().join("parent")).await.unwrap();
        fs::write(dir.path().join("parent/new"), b"existing")
            .await
            .unwrap();
        assert_eq!(
            put(&store, &url, Body::from("x")).await.unwrap_err().status,
            StatusCode::CONFLICT
        );
        assert_eq!(
            fs::read(dir.path().join("parent/new")).await.unwrap(),
            b"existing"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn active_upload_survives_expiry_and_retains_quota() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), 1);
        let url = store
            .prepare("new", 1, sha256_hex(b"x"), None, SHORT_LINK_TTL_SECS)
            .await
            .unwrap();
        let attempt = store.begin(&token(&url)).unwrap();
        assert_eq!(
            put(&store, &url, Body::from("x")).await.unwrap_err().status,
            StatusCode::CONFLICT
        );
        tokio::time::sleep(Duration::from_secs(SHORT_LINK_TTL_SECS) + EXPIRY_GRACE).await;
        assert_eq!(
            put(&store, &url, Body::from("x")).await.unwrap_err().status,
            StatusCode::NOT_FOUND
        );
        assert!(
            store
                .prepare("other", 1, sha256_hex(b"y"), None, DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        let file = store.receive(&attempt.0, Body::from("x")).await.unwrap();
        store.commit(&attempt, file).await.unwrap();
        assert_eq!(store.usage.lock().unwrap().bytes, 0);
        assert_eq!(fs::read(dir.path().join("new")).await.unwrap(), b"x");
        store
            .prepare("other", 1, sha256_hex(b"y"), None, DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn validates_prepare_and_releases_expired_reservations() {
        const OVERSIZED_REQUEST_BYTES: u64 = 2;
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), 1);
        let hash = sha256_hex(b"x");
        for path in ["../outside", "/absolute", ".", "missing/new"] {
            assert!(
                store
                    .prepare(path, 1, hash.clone(), None, DEFAULT_LINK_TTL_SECS)
                    .await
                    .is_err()
            );
        }
        for secs in [MIN_LINK_TTL_SECS - 1, MAX_LINK_TTL_SECS + 1] {
            assert!(
                store
                    .prepare("new", 1, hash.clone(), None, secs)
                    .await
                    .is_err()
            );
        }
        for invalid in ["bad".to_owned(), "G".repeat(SHA256_HEX_CHARS)] {
            assert!(
                store
                    .prepare("new", 1, invalid, None, DEFAULT_LINK_TTL_SECS)
                    .await
                    .is_err()
            );
        }
        assert!(
            store
                .prepare(
                    "new",
                    1,
                    hash.clone(),
                    Some("bad".into()),
                    DEFAULT_LINK_TTL_SECS
                )
                .await
                .is_err()
        );
        assert!(
            store
                .prepare(
                    "missing",
                    1,
                    hash.clone(),
                    Some(hash.clone()),
                    DEFAULT_LINK_TTL_SECS
                )
                .await
                .is_err()
        );
        assert!(
            store
                .prepare(
                    "new",
                    OVERSIZED_REQUEST_BYTES,
                    hash.clone(),
                    None,
                    DEFAULT_LINK_TTL_SECS
                )
                .await
                .is_err()
        );
        let url = store
            .prepare("new", 1, hash.clone(), None, SHORT_LINK_TTL_SECS)
            .await
            .unwrap();
        assert!(
            store
                .prepare("other", 1, hash.clone(), None, DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_secs(SHORT_LINK_TTL_SECS) + EXPIRY_GRACE).await;
        assert_eq!(
            put(&store, &url, Body::from("x")).await.unwrap_err().status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(store.usage.lock().unwrap().bytes, 0);
        assert_eq!(store.usage.lock().unwrap().count, 0);
        let empty = store
            .prepare("empty", 0, sha256_hex(b""), None, DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
        assert_eq!(
            put(&store, &empty, Body::empty())
                .await
                .unwrap()
                .0
                .full_size,
            0
        );
        assert_eq!(fs::read(dir.path().join("empty")).await.unwrap(), b"");
    }

    #[tokio::test]
    async fn cancelled_attempt_is_retryable_and_upload_does_not_hold_operation_lock() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), SMALL_QUOTA_BYTES);
        let url = store
            .prepare("new", 1, sha256_hex(b"x"), None, DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
        let attempt = store.begin(&token(&url)).unwrap();
        let held = store.operation_lock.try_lock().unwrap();
        let file = store.receive(&attempt.0, Body::from("x")).await.unwrap();
        drop(file);
        drop(held);
        drop(attempt);
        assert_eq!(
            put(&store, &url, Body::from("x"))
                .await
                .unwrap()
                .0
                .after_hash,
            sha256_hex(b"x")
        );
    }
}

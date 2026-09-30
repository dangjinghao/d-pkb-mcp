//! Shared byte reservations for downloads and uploads.

use std::sync::{Arc, Mutex};

pub(crate) struct TransferQuota {
    limit: u64,
    used: Mutex<u64>,
}

pub(crate) struct Reservation {
    quota: Arc<TransferQuota>,
    bytes: u64,
}

impl TransferQuota {
    pub(crate) fn new(limit: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(limit > 0, "transfer-quota-bytes must be positive");
        Ok(Self {
            limit,
            used: Mutex::new(0),
        })
    }

    pub(crate) fn reserve(self: &Arc<Self>, bytes: u64) -> Option<Reservation> {
        let mut used = self.used.lock().unwrap();
        if bytes > self.limit - *used {
            return None;
        }
        *used += bytes;
        Some(Reservation {
            quota: self.clone(),
            bytes,
        })
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        *self.quota.used.lock().unwrap() -= self.bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DEFAULT_LINK_TTL_SECS;
    use crate::{
        downloads::Downloads,
        hash::sha256_hex,
        uploads::{Uploads, resource_upload},
    };
    use axum::{
        body::Body,
        extract::{Path, State},
    };

    #[test]
    fn validates_and_releases_reservations_without_overflow() {
        assert!(TransferQuota::new(0).is_err());
        let quota = Arc::new(TransferQuota::new(u64::MAX).unwrap());
        let all = quota.reserve(u64::MAX).unwrap();
        assert!(quota.reserve(1).is_none());
        drop(all);
        assert!(quota.reserve(1).is_some());
    }

    #[tokio::test]
    async fn uploads_and_downloads_share_capacity_and_commit_releases_it() {
        let dir = tempfile::tempdir().unwrap();
        const CONTENT: &[u8] = b"data";
        const SHARED_CAPACITY_BYTES: u64 = 8;
        let quota = Arc::new(TransferQuota::new(SHARED_CAPACITY_BYTES).unwrap());
        let downloads = Arc::new(Downloads::new(quota.clone()));
        let uploads = Arc::new(Uploads::new(
            dir.path().to_owned(),
            dir.path().to_owned(),
            Arc::new(tokio::sync::Mutex::new(())),
            quota.clone(),
        ));
        let source = dir.path().join("source");
        tokio::fs::write(&source, b"data").await.unwrap();
        downloads
            .prepare(&source, dir.path(), DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
        assert!(
            uploads
                .prepare(
                    "large",
                    b"large".len() as u64,
                    sha256_hex(b"large"),
                    None,
                    DEFAULT_LINK_TTL_SECS
                )
                .await
                .is_err()
        );
        let subpath = uploads
            .prepare(
                "new",
                CONTENT.len() as u64,
                sha256_hex(b"data"),
                None,
                DEFAULT_LINK_TTL_SECS,
            )
            .await
            .unwrap();
        assert_eq!(*quota.used.lock().unwrap(), SHARED_CAPACITY_BYTES);
        assert!(
            downloads
                .prepare(&source, dir.path(), DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        assert!(
            resource_upload(
                State(uploads.clone()),
                Path(subpath.rsplit('/').next().unwrap().to_owned()),
                Body::from("data")
            )
            .await
            .is_ok()
        );
        assert_eq!(*quota.used.lock().unwrap(), CONTENT.len() as u64);
        downloads
            .prepare(&source, dir.path(), DEFAULT_LINK_TTL_SECS)
            .await
            .unwrap();
        assert_eq!(*quota.used.lock().unwrap(), SHARED_CAPACITY_BYTES);
        assert!(
            uploads
                .prepare("other", 1, sha256_hex(b"x"), None, DEFAULT_LINK_TTL_SECS)
                .await
                .is_err()
        );
        drop(downloads);
        assert_eq!(*quota.used.lock().unwrap(), 0);
        assert!(
            uploads
                .prepare("other", 1, sha256_hex(b"x"), None, DEFAULT_LINK_TTL_SECS)
                .await
                .is_ok()
        );
    }
}

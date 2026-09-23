//! Paths assume a PKB containing only regular files and directories.
//! Behavior is undefined if symbolic links are present; no special handling is provided.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use tokio::{fs, io};

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

pub(crate) struct Walker {
    stack: Vec<PathBuf>,
    buffer: VecDeque<PathBuf>,
}

impl Walker {
    pub(crate) async fn new(start: PathBuf) -> io::Result<Self> {
        let metadata = fs::metadata(&start).await?;
        if metadata.is_dir() {
            Ok(Self {
                stack: vec![start],
                buffer: VecDeque::new(),
            })
        } else {
            Ok(Self {
                stack: Vec::new(),
                buffer: VecDeque::from([start]),
            })
        }
    }

    pub(crate) async fn next(&mut self) -> io::Result<Option<PathBuf>> {
        loop {
            if let Some(path) = self.buffer.pop_front() {
                return Ok(Some(path));
            }
            let Some(dir) = self.stack.pop() else {
                return Ok(None);
            };

            let mut read_dir = fs::read_dir(&dir).await?;
            let mut entries = Vec::new();
            while let Some(entry) = read_dir.next_entry().await? {
                entries.push(entry);
            }
            entries.sort_by_key(|entry| entry.file_name());

            let mut subdirs = Vec::new();
            let mut buffer = VecDeque::with_capacity(entries.len());
            for entry in entries {
                let path = entry.path();
                if entry.file_type().await?.is_dir() {
                    subdirs.push(path.clone());
                }
                buffer.push_back(path);
            }
            self.stack.extend(subdirs.into_iter().rev());
            self.buffer = buffer;
        }
    }
}

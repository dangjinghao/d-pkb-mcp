//! Async recursive directory traversal in deterministic order.

use std::{collections::VecDeque, path::PathBuf};

use tokio::{fs, io};

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

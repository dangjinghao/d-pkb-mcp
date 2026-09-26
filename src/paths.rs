//! Paths assume a PKB containing only regular files and directories.
//! Behavior is undefined if symbolic links are present; no special handling is provided.

use std::{
    collections::VecDeque,
    path::{Component, Path, PathBuf},
};

use tokio::{fs, io};

/// Canonical PKB and temporary paths, shared by all filesystem tools.
#[derive(Debug)]
pub(crate) struct PkbPath {
    root: PathBuf,
    tmp_path: PathBuf,
}

impl PkbPath {
    /// Both paths must already be canonicalized by startup configuration.
    pub(crate) fn new(root: PathBuf, tmp_path: PathBuf) -> Self {
        Self { root, tmp_path }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn tmp_path(&self) -> &Path {
        &self.tmp_path
    }

    /// Only a temporary directory inside the PKB excludes PKB paths.
    pub(crate) fn excluded_path(&self) -> Option<&Path> {
        self.tmp_path
            .starts_with(&self.root)
            .then_some(self.tmp_path())
    }

    /// Resolve a literal path without IO; the target need not exist.
    pub(crate) fn resolve_inside_root(&self, user_path: &str) -> Result<PathBuf, String> {
        let mut relative = PathBuf::new();
        for component in Path::new(user_path).components() {
            match component {
                Component::Normal(name) => relative.push(name),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !relative.pop() {
                        return Err("Path escapes the PKB root".to_owned());
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err("Absolute paths are not supported".to_owned());
                }
            }
        }
        let target = self.root.join(relative);
        if self.is_path_reserved(&target) {
            return Err(reserved_path_error(&target));
        }
        Ok(target)
    }

    pub(crate) fn is_path_reserved(&self, path: &Path) -> bool {
        self.excluded_path()
            .is_some_and(|tmp| path.starts_with(tmp))
    }

    pub(crate) fn contains_reserved_path(&self, path: &Path) -> bool {
        self.excluded_path()
            .is_some_and(|tmp| path.starts_with(tmp) || tmp.starts_with(path))
    }

    pub(crate) async fn walk(&self, start: PathBuf) -> io::Result<Walker<'_>> {
        Walker::new(start, self).await
    }
}

pub(crate) fn reserved_path_error(path: &Path) -> String {
    format!(
        "Access denied: {} is reserved for temporary files on the same filesystem.",
        path.display()
    )
}

pub(crate) struct Walker<'a> {
    paths: &'a PkbPath,
    stack: Vec<PathBuf>,
    buffer: VecDeque<PathBuf>,
}

impl<'a> Walker<'a> {
    async fn new(start: PathBuf, paths: &'a PkbPath) -> io::Result<Self> {
        let metadata = fs::metadata(&start).await?;
        if metadata.is_dir() {
            Ok(Self {
                paths,
                stack: vec![start],
                buffer: VecDeque::new(),
            })
        } else {
            Ok(Self {
                paths,
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
                if self.paths.is_path_reserved(&path) {
                    continue;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_paths_are_normalized_and_component_based() {
        let root = Path::new("/tmp/pkb");
        let tmp = root.join("internal/.tmp");
        let paths = PkbPath::new(root.to_owned(), tmp.clone());
        assert!(paths.resolve_inside_root("internal/./.tmp/file").is_err());
        assert!(paths.resolve_inside_root("other/../internal/.tmp").is_err());
        assert!(paths.resolve_inside_root("../outside").is_err());
        assert!(paths.resolve_inside_root("/absolute").is_err());
        assert!(
            paths
                .resolve_inside_root("internal/.tmp-other/file")
                .is_ok()
        );
        assert!(
            PkbPath::new(root.to_owned(), "/tmp".into())
                .resolve_inside_root("DJH")
                .is_ok()
        );
        assert!(paths.contains_reserved_path(&root.join("internal")));
        assert!(!paths.contains_reserved_path(&root.join("other")));
        assert!(!PkbPath::new(root.to_owned(), "/tmp".into()).contains_reserved_path(root));
    }

    #[tokio::test]
    async fn walker_skips_reserved_directory_and_contents() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path();
        let tmp = root.join("internal/.tmp");
        let paths = PkbPath::new(root.to_owned(), tmp.clone());
        fs::create_dir_all(&tmp).await?;
        fs::write(tmp.join("secret"), "hidden").await?;
        fs::write(root.join("visible"), "visible").await?;
        let mut walker = paths.walk(root.to_owned()).await?;
        let mut found = Vec::new();
        while let Some(path) = walker.next().await? {
            found.push(path.strip_prefix(root).unwrap().to_owned());
        }
        assert_eq!(
            found,
            vec![PathBuf::from("internal"), PathBuf::from("visible")]
        );
        Ok(())
    }
}

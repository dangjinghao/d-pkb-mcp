//! Process-local snapshots with Git metadata stored separately from the PKB.
//! Call init once before serving requests. Call snapshot while holding the tools'
//! shared mutation lock. These functions do not provide their own mutation lock.
//! Git and file reads run on blocking threads.
//! The repository is retained until explicitly cleaned up; shutdown cleanup is not
//! implemented (static values are not dropped at process exit).

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::OnceLock};

use anyhow::{Context, Result, anyhow, ensure};
use git2::{ErrorCode, Oid, Repository, Signature};
use tempfile::TempDir;

struct State {
    paths: crate::paths::PkbPath,
    repository: TempDir,
}

static STATE: OnceLock<State> = OnceLock::new();

/// Create a fresh repository using the PKB directly as its working directory.
/// The first commit is made by a subsequent call to snapshot.
pub(crate) async fn init(root: &Path, tmp_path: &Path) -> Result<()> {
    let root = root.to_owned();
    let tmp_path = tmp_path.to_owned();
    tokio::task::spawn_blocking(move || {
        ensure!(STATE.get().is_none(), "Snapshots are already initialized");
        let root = fs::canonicalize(root)?;
        ensure!(root.is_dir(), "PKB root must be a directory");
        fs::create_dir_all(&tmp_path)?;
        let tmp_path = fs::canonicalize(tmp_path)?;
        ensure!(
            tmp_path != root,
            "Snapshot temp directory must differ from PKB root"
        );
        let repository = tempfile::Builder::new()
            .prefix("pkb-snapshots-repo-")
            .tempdir_in(&tmp_path)?;
        let repo = Repository::init_bare(repository.path())?;
        // Do not create or replace a .git entry inside the PKB.
        repo.set_workdir(&root, false)?;
        // Persist the work directory so reopening the repository retains it.
        repo.config()?.set_str(
            "core.worktree",
            root.to_str().context("PKB root must be valid UTF-8")?,
        )?;
        repo.config()?.set_bool("core.bare", false)?;
        let state = State {
            paths: crate::paths::PkbPath::new(root, tmp_path),
            repository,
        };
        STATE
            .set(state)
            .map_err(|_| anyhow!("Snapshots are already initialized"))
    })
    .await?
}

/// Read the current PKB state and append one commit, even if content is unchanged.
pub(crate) async fn snapshot(message: &str) -> Result<Oid> {
    let message = message.to_owned();
    tokio::task::spawn_blocking(move || {
        let state = STATE.get().context("Snapshots are not initialized")?;
        let repo = Repository::open(state.repository.path())?;

        // Build a fresh tree directly from the PKB. Deleted paths are naturally
        // absent; earlier commits keep their objects without a second file copy.
        let tree = repo.find_tree(write_tree(
            &repo,
            state.paths.root(),
            state.paths.tmp_path(),
        )?)?;
        let parent = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(error) if error.code() == ErrorCode::UnbornBranch => None,
            Err(error) => return Err(error.into()),
        };
        let parents: Vec<_> = parent.iter().collect();
        let signature = Signature::now("d-pkb-mcp", "snapshot@localhost")?;
        Ok(repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &message,
            &tree,
            &parents,
        )?)
    })
    .await?
}

/// Record a tool snapshot unless startup disabled snapshots.
/// Callers must hold the shared mutation lock across both snapshots and the change.
pub(crate) async fn snapshot_if_enabled(message: &str) -> Result<Option<Oid>> {
    if STATE.get().is_some() {
        return Ok(Some(snapshot(message).await?));
    }
    Ok(None)
}

/// Call while holding the mutation lock. Keep recovery data if a failed operation
/// changed files partially, or the current disk state cannot be inspected.
pub(crate) async fn operation_failed(before: Option<Oid>, error: impl std::fmt::Display) -> String {
    let error = error.to_string();
    let Some(before) = before else { return error };
    match discard_unchanged_snapshot(before).await {
        Ok(true) => error,
        Ok(false) => {
            format!("{error}; pre-operation snapshot retained because disk content changed")
        }
        Err(cleanup) => format!("{error}; cannot discard pre-operation snapshot: {cleanup}"),
    }
}

/// Move only the Git reference, never the PKB files. Unreachable objects may
/// remain in storage, but no longer appear in snapshot_list.
async fn discard_unchanged_snapshot(before: Oid) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        let state = STATE.get().context("Snapshots are not initialized")?;
        let repo = Repository::open(state.repository.path())?;
        let mut head = repo.head()?;
        ensure!(head.target() == Some(before), "Snapshot HEAD changed");
        let commit = repo.find_commit(before)?;
        let tree = write_tree(&repo, state.paths.root(), state.paths.tmp_path())?;
        if tree != commit.tree_id() {
            return Ok(false);
        }
        head.set_target(commit.parent_id(0)?, "discard snapshot of failed operation")?;
        Ok(true)
    })
    .await?
}

/// List reachable commits in reverse history order, with their one-line titles.
/// Zero means unlimited; the boolean indicates whether more commits exist.
pub(crate) async fn list(limit: usize) -> Result<(Vec<(Oid, String)>, bool)> {
    tokio::task::spawn_blocking(move || {
        let state = STATE.get().context("Snapshots are not initialized")?;
        let repo = Repository::open(state.repository.path())?;
        let head = match repo.head() {
            Ok(head) => head.peel_to_commit()?.id(),
            Err(error) if error.code() == ErrorCode::UnbornBranch => {
                return Ok((Vec::new(), false));
            }
            Err(error) => return Err(error.into()),
        };
        let mut history = repo.revwalk()?;
        // Parent order remains correct even when commits share a timestamp.
        history.set_sorting(git2::Sort::TOPOLOGICAL)?;
        history.push(head)?;
        let mut entries = Vec::new();
        for id in history {
            let id = id?;
            if limit != 0 && entries.len() == limit {
                return Ok((entries, true));
            }
            let commit = repo.find_commit(id)?;
            let title = String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default())
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            entries.push((id, title));
        }
        Ok((entries, false))
    })
    .await?
}

/// Select and read a file version before recording the pre-restoration snapshot.
/// None means the selected commit has no file at this path.
pub(crate) async fn restore_content(
    path: &Path,
    snapshot: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    let path = path.to_owned();
    let snapshot = snapshot.map(str::to_owned);
    tokio::task::spawn_blocking(move || {
        let state = STATE.get().context("Snapshots are not initialized")?;
        ensure!(
            !is_excluded(&state.paths, &path),
            "Path is excluded from snapshots"
        );
        let repo = Repository::open(state.repository.path())?;
        let head = repo
            .head()
            .context("No recovery record exists")?
            .peel_to_commit()?;
        let selected = if let Some(snapshot) = snapshot {
            ensure!(snapshot.len() == 40, "Expected a full snapshot commit ID");
            let id = Oid::from_str(&snapshot).context("Invalid snapshot commit ID")?;
            ensure!(
                id == head.id() || repo.graph_descendant_of(head.id(), id)?,
                "Not a known session snapshot"
            );
            repo.find_commit(id)?
        } else {
            let mut commit = head;
            loop {
                ensure!(
                    commit.parent_count() > 0,
                    "No previous state exists for this path"
                );
                let parent = commit.parent(0)?;
                if file_entry(&commit.tree()?, &path)? != file_entry(&parent.tree()?, &path)? {
                    break parent;
                }
                commit = parent;
            }
        };
        match file_entry(&selected.tree()?, &path)? {
            Some((id, mode)) => {
                ensure!(
                    mode == 0o100644 || mode == 0o100755,
                    "Cannot restore a directory"
                );
                Ok(Some(repo.find_blob(id)?.content().to_vec()))
            }
            None => Ok(None),
        }
    })
    .await?
}

fn is_excluded(paths: &crate::paths::PkbPath, relative: &Path) -> bool {
    relative.components().any(|part| part.as_os_str() == ".git")
        // Tree construction skips the temp directory only when it is inside the PKB.
        || paths.is_path_reserved(&paths.root().join(relative))
}

fn file_entry(tree: &git2::Tree<'_>, path: &Path) -> Result<Option<(Oid, i32)>> {
    match tree.get_path(path) {
        Ok(entry) => Ok(Some((entry.id(), entry.filemode()))),
        Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn write_tree(repo: &Repository, directory: &Path, excluded: &Path) -> Result<Oid> {
    // Build trees from raw bytes so ignore rules, nested repositories, and Git
    // attributes cannot omit files or convert their contents. Empty directories
    // are omitted, like ordinary Git snapshots.
    let mut tree = repo.treebuilder(None)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name() == ".git" || entry.path() == excluded {
            continue;
        }
        let (id, mode) = if entry.file_type()?.is_dir() {
            let id = write_tree(repo, &entry.path(), excluded)?;
            if repo.find_tree(id)?.is_empty() {
                continue;
            }
            (id, 0o040000)
        } else {
            let executable = entry.metadata()?.permissions().mode() & 0o111 != 0;
            (
                repo.blob(&fs::read(entry.path())?)?,
                if executable { 0o100755 } else { 0o100644 },
            )
        };
        tree.insert(entry.file_name(), id, mode)?;
    }
    Ok(tree.write()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_only_temp_paths_inside_the_pkb() {
        let is_excluded = |root: &Path, tmp: &Path, path: &Path| {
            super::is_excluded(
                &crate::paths::PkbPath::new(root.to_owned(), tmp.to_owned()),
                path,
            )
        };
        let root = Path::new("/tmp/pkb");
        let file = Path::new("DJH");
        assert!(!is_excluded(root, Path::new("/tmp"), file));
        assert!(!is_excluded(root, Path::new("/tmp/staging"), file));
        assert!(!is_excluded(root, Path::new("/tmp/pkb/.tmp"), file));
        assert!(is_excluded(
            root,
            Path::new("/tmp/pkb/.tmp"),
            Path::new(".tmp/file")
        ));
        assert!(!is_excluded(
            root,
            Path::new("/tmp/pkb/.tmp"),
            Path::new(".tmp-other/file")
        ));
        assert!(is_excluded(
            root,
            Path::new("/tmp"),
            Path::new("nested/.git/config")
        ));
    }

    #[tokio::test]
    async fn records_versions_directly_without_changing_pkb_git_data() -> Result<()> {
        ensure!(snapshot("before init").await.is_err());
        ensure!(list(0).await.is_err());
        snapshot_if_enabled("disabled").await?;
        let folder = tempfile::tempdir()?;
        let root = folder.path().join("pkb");
        let tmp = root.join(".tmp");
        fs::create_dir_all(&tmp)?;
        fs::create_dir_all(root.join("nested/.git"))?;
        fs::write(root.join("nested/.git/config"), "original repository")?;
        fs::write(root.join(".gitignore"), "*.bin\n")?;
        fs::write(root.join(".gitattributes"), "*.txt text eol=lf\n")?;
        fs::write(root.join("nested/note.txt"), b"old\r\n")?;
        fs::write(root.join("image.bin"), [0, 255, 1])?;
        fs::write(tmp.join("staged.txt"), "not PKB content")?;

        fs::create_dir_all(root.join(".git"))?;
        fs::write(root.join(".git/config"), "root repository")?;
        init(&root, &tmp).await?;
        ensure!(list(0).await? == (Vec::new(), false));
        ensure!(init(&root, &tmp).await.is_err());
        let first = snapshot("initial").await?;
        fs::remove_file(root.join("nested/note.txt"))?;
        fs::write(root.join("new.txt"), "new")?;
        let second = snapshot("remove and create").await?;
        let third = snapshot("unchanged\n\nPrivate metadata").await?;
        let expected = vec![
            (third, "unchanged".to_owned()),
            (second, "remove and create".to_owned()),
            (first, "initial".to_owned()),
        ];
        ensure!(list(0).await? == (expected.clone(), false));
        ensure!(list(3).await? == (expected.clone(), false));
        ensure!(list(2).await? == (expected[..2].to_vec(), true));

        let repo = Repository::open(STATE.get().unwrap().repository.path())?;
        ensure!(repo.workdir() == Some(root.as_path()));
        ensure!(!repo.path().join("image.bin").exists());
        ensure!(fs::read_to_string(root.join(".git/config"))? == "root repository");
        let first_tree = repo.find_commit(first)?.tree()?;
        let second_commit = repo.find_commit(second)?;
        let second_tree = second_commit.tree()?;
        ensure!(
            repo.find_blob(first_tree.get_path(Path::new("nested/note.txt"))?.id())?
                .content()
                == b"old\r\n"
        );
        ensure!(
            repo.find_blob(first_tree.get_name("image.bin").unwrap().id())?
                .content()
                == [0, 255, 1]
        );
        ensure!(first_tree.get_name(".tmp").is_none());
        ensure!(first_tree.get_path(Path::new("nested/.git")).is_err());
        ensure!(second_tree.get_path(Path::new("nested/note.txt")).is_err());
        ensure!(second_tree.get_name("new.txt").is_some());
        ensure!(second_commit.parent_id(0)? == first);
        ensure!(repo.find_commit(third)?.parent_id(0)? == second);
        ensure!(repo.find_commit(third)?.tree_id() == second_tree.id());
        ensure!(fs::read_to_string(root.join("nested/.git/config"))? == "original repository");
        ensure!(
            restore_content(Path::new("nested/note.txt"), None).await? == Some(b"old\r\n".to_vec())
        );
        ensure!(restore_content(Path::new("new.txt"), None).await?.is_none());
        ensure!(restore_content(Path::new("image.bin"), None).await.is_err());
        ensure!(restore_content(Path::new("missing"), None).await.is_err());
        ensure!(
            restore_content(Path::new("image.bin"), Some(&first.to_string())).await?
                == Some(vec![0, 255, 1])
        );
        ensure!(
            restore_content(Path::new("nested"), Some(&first.to_string()))
                .await
                .is_err()
        );
        ensure!(
            restore_content(Path::new(".git/config"), Some(&first.to_string()))
                .await
                .is_err()
        );
        ensure!(
            restore_content(Path::new(".tmp/staged.txt"), Some(&first.to_string()))
                .await
                .is_err()
        );
        ensure!(
            restore_content(Path::new("new.txt"), Some("HEAD"))
                .await
                .is_err()
        );
        ensure!(
            restore_content(Path::new("new.txt"), Some(&"0".repeat(40)))
                .await
                .is_err()
        );
        let before = snapshot("before failed operation").await?;
        ensure!(discard_unchanged_snapshot(before).await?);
        ensure!(list(0).await?.0 == expected);
        ensure!(fs::read_to_string(root.join("new.txt"))? == "new");

        let before = snapshot("before partial failure").await?;
        fs::remove_file(root.join("new.txt"))?;
        ensure!(!discard_unchanged_snapshot(before).await?);
        ensure!(list(1).await?.0[0].0 == before);
        fs::write(root.join("new.txt"), "new")?;
        ensure!(discard_unchanged_snapshot(before).await?);
        ensure!(list(0).await?.0 == expected);

        let before = snapshot("before stale cleanup").await?;
        let after = snapshot("later commit").await?;
        ensure!(discard_unchanged_snapshot(before).await.is_err());
        ensure!(list(1).await?.0[0].0 == after);
        ensure!(discard_unchanged_snapshot(after).await?);
        ensure!(discard_unchanged_snapshot(before).await?);
        Ok(())
    }
}

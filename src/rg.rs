//! Invoking the external `rg` binary with bounded output.

use std::{ffi::OsString, path::Path, process::Stdio};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
};

pub(crate) const NOT_AVAILABLE: &str = "`rg` is not available, use native tool instead";

pub(crate) struct Output {
    pub(crate) lines: Vec<String>,
    pub(crate) truncated: bool,
}

pub(crate) async fn run(cwd: &Path, args: Vec<OsString>, limit: usize) -> Result<Output, String> {
    let mut child = Command::new("rg")
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => NOT_AVAILABLE.to_owned(),
            _ => e.to_string(),
        })?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let stderr_task = tokio::spawn(async move {
        let mut buffer = String::new();
        let _ = stderr.read_to_string(&mut buffer).await;
        buffer
    });

    let mut lines = Vec::new();
    let mut truncated = false;
    let mut reader = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if limit > 0 && lines.len() >= limit {
            truncated = true;
            break;
        }
        lines.push(line);
    }

    if truncated {
        let _ = child.kill().await;
        let _ = child.wait().await;
        let _ = stderr_task.await;
        return Ok(Output { lines, truncated });
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    let stderr = stderr_task.await.unwrap_or_default();
    //  2  exit status occurs when an error occurred. This is true for both catastrophic errors (e.g., a regex syntax
    //   error) and for soft errors (e.g., unable to read a file).
    if status.code() == Some(2) {
        return Err(stderr.trim().to_owned());
    }

    Ok(Output { lines, truncated })
}

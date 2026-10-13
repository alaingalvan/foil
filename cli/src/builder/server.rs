use super::get_foil_builder_path;
use crate::{BuildMode, Result};
use serde_derive::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// One response line: `{ id, ok, error?, result? }`.
#[derive(Deserialize)]
struct Response {
    id: Option<u64>,
    ok: bool,
    error: Option<String>,
}

/// `None` once the server's stdout has closed.
type Waiters = Arc<Mutex<Option<HashMap<u64, Sender<std::result::Result<(), String>>>>>>;

/// A long-lived `@foil-cms/builder` process speaking one JSON message per line over stdin/stdout.
pub struct BuilderServer {
    child: Child,
    stdin: Option<Mutex<ChildStdin>>,
    waiters: Waiters,
    next_id: AtomicU64,
    reader: Option<JoinHandle<()>>,
}

/// A build request that has been sent and not yet answered.
pub struct PendingBuild {
    title: String,
    rx: Receiver<std::result::Result<(), String>>,
}

impl PendingBuild {
    pub fn wait(self) -> Result<()> {
        match self.rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(format!("Build of {} failed: {}", self.title, e).into()),
            Err(_) => Err(format!("Builder exited while building {}", self.title).into()),
        }
    }
}

impl BuilderServer {
    pub fn start(mode: &BuildMode) -> Result<Self> {
        let mut command = Command::new("node");
        command
            .current_dir(get_foil_builder_path())
            .args([
                "--experimental-specifier-resolution=node",
                "--experimental-modules",
                "--experimental-import-meta-resolve",
                "--no-warnings",
                "dist/src/foil-builder.js",
                "--serve",
            ])
            .env(
                "NODE_ENV",
                match mode {
                    BuildMode::Release => "production",
                    BuildMode::Development => "development",
                },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or("builder stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("builder stdout unavailable")?;

        let waiters: Waiters = Arc::new(Mutex::new(Some(HashMap::new())));
        let reader_waiters = waiters.clone();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                let Ok(res) = serde_json::from_str::<Response>(&line) else {
                    continue;
                };
                let Some(id) = res.id else { continue };
                let outcome = if res.ok {
                    Ok(())
                } else {
                    Err(res.error.unwrap_or_default())
                };
                if let Some(Some(map)) = reader_waiters.lock().ok().as_deref_mut() {
                    if let Some(tx) = map.remove(&id) {
                        let _ = tx.send(outcome);
                    }
                }
            }
            // Dropping the senders wakes every pending `wait`.
            if let Ok(mut guard) = reader_waiters.lock() {
                *guard = None;
            }
        });

        Ok(Self {
            child,
            stdin: Some(Mutex::new(stdin)),
            waiters,
            next_id: AtomicU64::new(1),
            reader: Some(reader),
        })
    }

    /// False once the server's stdout has closed, meaning the process exited.
    pub fn is_running(&self) -> bool {
        self.waiters.lock().map(|w| w.is_some()).unwrap_or(false)
    }

    /// Send a `build` request. `payload` mirrors the one-shot CLI args in camelCase.
    pub fn build(&self, title: &str, payload: Value) -> Result<PendingBuild> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        match self.waiters.lock().map_err(|_| "builder lock poisoned")?.as_mut() {
            Some(map) => map.insert(id, tx),
            None => return Err("Builder server is not running".into()),
        };
        let line = json!({ "id": id, "command": "build", "payload": payload }).to_string();
        let stdin = self.stdin.as_ref().ok_or("builder stdin closed")?;
        let mut stdin = stdin.lock().map_err(|_| "builder lock poisoned")?;
        writeln!(stdin, "{}", line)?;
        stdin.flush()?;
        Ok(PendingBuild {
            title: title.to_string(),
            rx,
        })
    }
}

impl Drop for BuilderServer {
    fn drop(&mut self) {
        // Closing stdin makes the server finish in-flight builds and exit.
        if let Some(stdin) = self.stdin.take() {
            if let Ok(mut stdin) = stdin.lock() {
                let _ = writeln!(stdin, "{}", json!({ "id": 0, "command": "shutdown" }));
            }
        }
        // On error paths nothing waits for builds, so don't hang on them.
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

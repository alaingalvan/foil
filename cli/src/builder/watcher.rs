use super::build_mode::BuildMode;
use super::metadata::FoilMetadata;
use super::resolver::Foil;
use super::{build, changed_foils, resolve_foils};

use crate::error::Result;
use crate::server::{self, ServerChildren};

use async_std::task;
use std::env;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Same states as the TypeScript watcher: run once, wait, rebuild, then wait again.
enum WatcherState {
    Initial,
    Watching,
    Processing,
    Done,
}

struct WatchedFile {
    path: PathBuf,
    modified: Option<SystemTime>,
}

/// `foil build --watch`: compile what changed, then keep the server renderer in sync.
pub async fn watch(build_mode: BuildMode) -> Result<()> {
    let cwd = env::current_dir().unwrap_or_default();
    let mut state = WatcherState::Initial;
    let mut children: Option<ServerChildren> = None;
    let mut watched: Vec<WatchedFile> = Vec::new();

    loop {
        match state {
            WatcherState::Initial => {
                let start_foils = resolve_changed(&cwd, &build_mode).await?;
                if !start_foils.is_empty() {
                    build(build_mode.clone()).await?;
                }
                children = start_server(children, &build_mode).await?;
                println!("\n👀 · Watching for changes... · \n");
                watched = snapshot_sources(&cwd).await?;
                state = WatcherState::Watching;
            }
            WatcherState::Watching => {
                // The TypeScript watcher called resolveFoils() here and never waited.
                // Block on the files that resolve already tracks, then hand changes forward.
                wait_for_change(&mut watched).await;
                let foils = resolve_changed(&cwd, &build_mode).await?;
                if !foils.is_empty() {
                    state = WatcherState::Processing;
                } else {
                    watched = snapshot_sources(&cwd).await?;
                }
            }
            WatcherState::Processing => {
                if let Err(err) = build(build_mode.clone()).await {
                    println!("\n❌ Build failed, server renderer left running.\n{}", err);
                } else if let Err(err) = restart_renderer(&mut children, &build_mode).await {
                    println!("\n❌ Failed to restart server renderer.\n{}", err);
                }
                state = WatcherState::Done;
            }
            WatcherState::Done => {
                println!("\n👀 · Watching for changes... · \n");
                watched = snapshot_sources(&cwd).await?;
                state = WatcherState::Watching;
            }
        }
    }
}

async fn resolve_changed(cwd: &PathBuf, build_mode: &BuildMode) -> Result<Vec<Foil>> {
    let mut resolved: Vec<(Foil, FoilMetadata)> = vec![];
    resolve_foils(cwd.clone(), &mut resolved).await?;
    Ok(changed_foils(resolved, build_mode))
}

async fn start_server(
    children: Option<ServerChildren>,
    build_mode: &BuildMode,
) -> Result<Option<ServerChildren>> {
    if children.is_some() {
        return Ok(children);
    }
    let build_mode = build_mode.clone();
    crate::misc::run_on_sql_stack("foil-watch-server", move || {
        task::block_on(server::spawn_server(build_mode))
    })
}

async fn restart_renderer(
    children: &mut Option<ServerChildren>,
    build_mode: &BuildMode,
) -> Result<()> {
    let Some(current) = children.as_mut() else {
        *children = start_server(None, build_mode).await?;
        return Ok(());
    };
    server::stop_child(&mut current.renderer);
    let build_mode = build_mode.clone();
    if let Some(renderer) = crate::misc::run_on_sql_stack("foil-watch-renderer", move || {
        task::block_on(server::spawn_renderer_only(build_mode))
    })? {
        current.renderer = renderer;
    }
    Ok(())
}

async fn snapshot_sources(cwd: &PathBuf) -> Result<Vec<WatchedFile>> {
    let mut resolved: Vec<(Foil, FoilMetadata)> = vec![];
    resolve_foils(cwd.clone(), &mut resolved).await?;
    let mut watched = Vec::new();
    for (foil, _) in resolved {
        for source in foil.source_files {
            push_watched(&mut watched, PathBuf::from(source.path));
        }
        push_watched(&mut watched, foil.root_path.join("package.json"));
    }
    Ok(watched)
}

fn push_watched(watched: &mut Vec<WatchedFile>, path: PathBuf) {
    if watched.iter().any(|file| file.path == path) {
        return;
    }
    let modified = path.metadata().and_then(|meta| meta.modified()).ok();
    watched.push(WatchedFile { path, modified });
}

fn file_changed(file: &WatchedFile) -> bool {
    let modified = file.path.metadata().and_then(|meta| meta.modified()).ok();
    modified != file.modified || !file.path.exists()
}

async fn wait_for_change(watched: &mut [WatchedFile]) {
    loop {
        task::sleep(Duration::from_millis(400)).await;
        if watched.iter().any(file_changed) {
            // Editors often write a file more than once. Wait out that burst.
            task::sleep(Duration::from_millis(250)).await;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WatchedFile, file_changed};
    use std::fs;
    use std::time::{Duration, SystemTime};

    #[test]
    fn file_changed_sees_a_newer_mtime() {
        let dir = std::env::temp_dir().join(format!(
            "foil-watch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("main.tsx");
        fs::write(&path, "before").unwrap();
        let watched = WatchedFile {
            modified: path.metadata().unwrap().modified().ok(),
            path: path.clone(),
        };
        assert!(!file_changed(&watched));

        std::thread::sleep(Duration::from_millis(20));
        fs::write(&path, "after").unwrap();
        // Some filesystems only bump mtime on a timer. Force it if the write did not.
        let bumped = SystemTime::now() + Duration::from_secs(2);
        let _ = fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(bumped));

        assert!(file_changed(&watched));
        assert!(file_changed(&WatchedFile {
            path: dir.join("deleted.tsx"),
            modified: Some(SystemTime::UNIX_EPOCH),
        }));
        let _ = fs::remove_dir_all(&dir);
    }
}
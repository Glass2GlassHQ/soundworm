//! `notify`-based file watcher for the routing script.
//!
//! Watches the directory containing the script (so editor-renames /
//! atomic-writes are still seen) and triggers `state.reload_script()`
//! when the target path changes. Reload is debounced to coalesce
//! editor save bursts.

use crate::state::DaemonState;
use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const DEBOUNCE: Duration = Duration::from_millis(150);

pub fn spawn(state: Arc<DaemonState>, script_path: PathBuf) -> Result<()> {
    let dir = script_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let (tx, mut rx) = mpsc::channel::<()>(16);

    let target = script_path.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(ev) = res else { return };
        if !matches!(
            ev.kind,
            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
        ) {
            return;
        }
        if ev.paths.iter().any(|p| p == &target) {
            let _ = tx.blocking_send(());
        }
    })?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;

    tokio::spawn(async move {
        // Keep the watcher alive for the lifetime of this task.
        let _watcher = watcher;
        loop {
            if rx.recv().await.is_none() {
                return;
            }
            // Drain any further events within DEBOUNCE so we reload once
            // per editor save burst.
            tokio::time::sleep(DEBOUNCE).await;
            while rx.try_recv().is_ok() {}

            match state.reload_script() {
                Ok(true) => tracing::info!("routing.rhai reloaded"),
                Ok(false) => {}
                Err(e) => tracing::warn!("routing.rhai reload failed (keeping old): {e:#}"),
            }
        }
    });
    Ok(())
}

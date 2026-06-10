mod ipc_server;
mod script_watch;
mod state;

use anyhow::Result;
use soundworm_core::backend::AudioBackend;
use state::DaemonState;
use std::sync::Arc;

#[cfg(target_os = "linux")]
use soundworm_pipewire::PipeWireBackend as PlatformBackend;
#[cfg(target_os = "macos")]
use soundworm_coreaudio::CoreAudioBackend as PlatformBackend;
#[cfg(target_os = "windows")]
use soundworm_wasapi::WasapiBackend as PlatformBackend;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("soundworm-daemon: no audio backend available for this target");

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    tracing::info!(" soundworm daemon starting");
    tracing::info!(" platform: {}", std::env::consts::OS);
    tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let backend: Arc<dyn AudioBackend> = Arc::new(PlatformBackend::new()?);
    let nodes = backend.enumerate_nodes().await?;
    tracing::info!("Backend '{}': {} nodes found", backend.name(), nodes.len());

    let state = Arc::new(DaemonState::new(Arc::clone(&backend)));
    {
        let mut g = state.graph.lock().unwrap();
        for node in nodes {
            g.add_node(node);
        }
    }
    state.start_event_pump();

    let rules_path = config_dir().join("soundworm/rules/default.toml");
    if rules_path.exists() {
        match state.load_rules_from(rules_path.clone()) {
            Ok(n) => tracing::info!("Loaded {} rules from {:?}", n, rules_path),
            Err(e) => tracing::warn!("Failed to load rules at {:?}: {e:#}", rules_path),
        }
    } else {
        tracing::info!("No rules file at {:?} — using defaults", rules_path);
    }

    let script_path = config_dir().join("soundworm/routing.rhai");
    if script_path.exists() {
        match state.load_script_from(script_path.clone()) {
            Ok(()) => tracing::info!("Loaded routing script {:?}", script_path),
            Err(e) => tracing::warn!("Failed to load script {:?}: {e:#}", script_path),
        }
    }
    if let Err(e) = script_watch::spawn(Arc::clone(&state), script_path.clone()) {
        tracing::warn!("script watcher disabled: {e:#}");
    }

    let sock = ipc_server::socket_path();
    let ipc_state = Arc::clone(&state);
    let ipc = tokio::spawn(async move {
        if let Err(e) = ipc_server::serve(sock, ipc_state).await {
            tracing::error!("IPC server crashed: {e:#}");
        }
    });

    tracing::info!("Ready — ctrl-c to stop");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = state.shutdown.notified() => {
            tracing::info!("Shutdown signal received via IPC");
        }
        _ = ipc => {}
    }
    tracing::info!("Shutdown complete");
    Ok(())
}

fn config_dir() -> std::path::PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let mut p = std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
            p.push(".config");
            p
        })
}

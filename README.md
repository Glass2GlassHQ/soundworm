# soundworm

Cross-platform audio session manager and router, written in Rust.
Primary target: Fedora / PipeWire.

Repo: https://github.com/Glass2GlassHQ/soundworm

## Crates

- core             Shared types: Node, Port, Link, AudioBackend trait
- graph            In-memory audio graph
- policy           TOML rules engine, conflict resolution, sessions
- rhai-engine      Scriptable routing (Rhai)
- pipewire-backend Linux PipeWire backend (primary)
- coreaudio-backend macOS CoreAudio backend (HAL device list + default-device routing)
- wasapi-backend   Windows WASAPI backend (endpoint list, default-endpoint routing, volume, device notifications)
- observability    Xrun log, latency metrics
- snapshots        JSON session save/load
- ipc              Daemon IPC wire types (NDJSON over a Unix socket; see docs/IPC.md)
- cli              `sw` command-line tool
- daemon           `swd` background service
- ui               Tauri desktop UI (node-graph canvas; not in the default workspace)

Backend coverage: PipeWire is the primary, fully-featured backend.
CoreAudio and WASAPI are compile-verified on CI and implement enumeration,
routing (set the default endpoint, since neither OS exposes port-to-port
linking) and volume; on-hardware validation still needs a real Mac /
Windows box. Windows routing uses the undocumented `IPolicyConfig`
interface (see `crates/wasapi-backend/src/win.rs`).

## Quick Start (Fedora)

    sudo dnf install git gcc
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source ~/.cargo/env
    cargo build
    cargo test --workspace
    cargo run --bin swd
    cargo run --bin sw -- help

## Install systemd user service

    mkdir -p ~/.config/systemd/user
    cp contrib/systemd/soundworm.service ~/.config/systemd/user/
    systemctl --user enable --now soundworm
    systemctl --user status soundworm

## Desktop UI (Tauri)

A Tauri 2 desktop app renders the live graph as a node canvas (nodes by
media-class, drag to link/reconnect/unlink) over the `swd` IPC socket.
It's outside the default workspace, so build it explicitly. See
`crates/ui/README.md` for details.

Build deps (Fedora):

    sudo dnf install webkit2gtk4.1-devel gtk3-devel \
                     libsoup3-devel javascriptcoregtk4.1-devel librsvg2-devel
    cargo install tauri-cli --version '^2.0'

Run the daemon, then the UI:

    RUST_LOG=info cargo run --bin swd          # terminal 1
    cd crates/ui && cargo tauri dev            # terminal 2

`cargo tauri dev` runs the Vite dev server (`pnpm install` first in
`crates/ui/frontend/`). For a release binary, `pnpm run build` the
frontend, then:

    cargo build -p soundworm-ui --manifest-path crates/ui/Cargo.toml

## Routing Rules

Copy config/rules/default.toml to ~/.config/soundworm/rules/default.toml

Example:

    [[rules]]
    name     = "spotify-to-speakers"
    priority = 10
    [rules.matches]
    node_name = "spotify"
    [rules.action]
    Route = { target = "alsa_output.default" }

    [[rules]]
    name     = "zoom-usb-mic"
    priority = 20
    [rules.matches]
    node_name = "zoom"
    [rules.action]
    Route = { target = "alsa_input.usb_mic" }

## Routing Script (Rhai)

Copy config/scripts/routing.rhai to ~/.config/soundworm/scripts/routing.rhai

Example:

    if node_name == "spotify" || node_name == "vlc" {
        log_route(node_name, "speakers");
        allow()
    } else if node_name == "zoom" || node_name == "teams" {
        log_route(node_name, "usb_headset");
        allow()
    } else {
        deny()
    }

## CLI Reference

    sw list                      List all audio nodes
    sw link   <src> <sink>       Create a route
    sw unlink <link-id>          Remove a route
    sw snapshot save <name>      Save current session
    sw snapshot load <name>      Restore a session
    sw snapshot list             List saved sessions
    sw metrics                   Show latency and xrun stats

## Environment Variables

- RUST_LOG            Default: info. Log level (error/warn/info/debug/trace)
- XDG_CONFIG_HOME     Default: ~/.config. Config directory
- SOUNDWORM_BACKEND   Default: pipewire. Backend override

## License

MIT OR Apache-2.0

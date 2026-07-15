# Install

## Build from source (Linux / Fedora)

    sudo dnf install -y gcc gcc-c++ make pkg-config \
        pipewire-devel clang-devel cargo
    cargo build --release --workspace

Binaries land in `target/release/`: `swd` (daemon) and `sw` (CLI).

## Run the daemon

    RUST_LOG=info ./target/release/swd

The daemon listens on `$XDG_RUNTIME_DIR/soundworm/swd.sock` (override
with `SOUNDWORM_SOCK`). It loads `~/.config/soundworm/rules/default.toml`
and `~/.config/soundworm/routing.rhai` at startup if present, and watches
the script for changes.

## systemd user service

    cp contrib/systemd/soundworm.service ~/.config/systemd/user/
    systemctl --user daemon-reload
    systemctl --user enable --now soundworm

It restarts on failure. Logs go to the journal:

    journalctl --user -u soundworm -f

## Desktop UI

The Tauri UI is a separate crate outside the default build (it pulls in
webkit2gtk). See `crates/ui/README.md`.

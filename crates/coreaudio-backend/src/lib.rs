//! CoreAudio backend for soundworm.
//!
//! On macOS this binds to the HAL via `coreaudio-sys` to enumerate
//! `AudioDevice`s and listen for hardware property changes. On every
//! other target the backend is a stub whose methods return
//! `SoundwormError::Backend("coreaudio unavailable")` — the daemon
//! picks backends per-target so this code only runs on macOS in
//! practice.
//!
//! # Semantic gaps vs. PipeWire (v0.5 documented)
//!
//! CoreAudio's HAL has no port-to-port linking. The closest concepts:
//!
//!   * `kAudioHardwarePropertyDefaultOutputDevice` — set the system
//!     default sink (affects all apps that don't override).
//!   * `kAudioHardwarePropertyDefaultInputDevice` — same for sources.
//!   * **Per-process output device** isn't routable via stable public
//!     API without an HAL plugin. Tools like Loopback/Audio Hijack ship
//!     a kext/userspace driver (ACE) for this. Out of scope for v0.5.
//!   * Multi-input/output devices can be assembled via
//!     **Aggregate Devices** (`AudioHardwareCreateAggregateDevice`).
//!
//! So [`backend::CoreAudioBackend::create_link`] interprets a link as
//! "set the system default output (or input) to the sink node's
//! device". `destroy_link` is a no-op — you can't un-default.
//! `subscribe` emits `NodeAppeared`/`NodeRemoved` from the HAL
//! `kAudioHardwarePropertyDevices` listener; ports are synthesized
//! one-per-stream-direction since the HAL model has streams, not ports.
//!
//! # Build coverage
//!
//! This crate's macOS path is exercised by CI on a macOS runner —
//! `cargo build` + `cargo test` against the published `coreaudio-sys`.
//! On Linux/Windows CI runners only the stub paths compile.

pub mod backend;

#[cfg(target_os = "macos")]
mod macos;

pub use backend::CoreAudioBackend;

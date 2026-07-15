# Introduction

soundworm routes audio between applications and hardware. It runs as a
background daemon (`swd`) that owns the audio graph and applies routing
rules; `sw` (CLI) and the desktop UI are thin clients that talk to it
over a local socket.

- Linux / PipeWire is the primary, fully working backend.
- macOS / CoreAudio and Windows / WASAPI enumerate devices; routing
  support varies by platform (see Troubleshooting).

Rules are declarative TOML, with a Rhai script for logic TOML cannot
express. The daemon reacts to nodes appearing in under 100 ms and can
save and restore a routing session.

This guide covers install, the CLI, writing rules and scripts, and
common problems. The wire protocol between clients and the daemon is
documented separately in `docs/IPC.md`.

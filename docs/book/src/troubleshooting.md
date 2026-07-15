# Troubleshooting

## `sw` can't connect

`connect ...: Connection refused` means the daemon isn't running or is on
a different socket. Start `swd`, or check `SOUNDWORM_SOCK` matches on both
sides. Read-only commands like `sw list` fall back to a direct backend;
`sw link` and friends need the daemon.

## Nothing gets auto-routed

- Confirm the rule matches: `node_name` is a substring test, so check the
  real name with `sw list`.
- TOML rules run before the script; a matching `Deny` or an earlier
  higher-`priority` rule can shadow what you expect.
- Watch it happen: `sw watch` shows `RulesApplied` / `LinkRejected` as
  nodes appear.

## Script changes don't take effect

The daemon watches `routing.rhai` and reloads on save. If a change does
nothing, the new script probably failed to compile and the old one is
still active. Check the log:

    journalctl --user -u soundworm -f

A parse error is logged and the previous script kept.

## Audio glitches / xruns

    sw metrics            # xrun totals + per-node latency
    sw metrics --watch    # live xrun stream

Latency is reported per node where the backend exposes it (JACK clients;
many ALSA/PW-native nodes do not advertise it).

## Platform notes

- Linux/PipeWire: full enumerate + link/unlink.
- macOS/CoreAudio: enumerates devices; "link" sets the default output
  device (the HAL has no port-to-port linking).
- Windows/WASAPI: enumerates endpoints; routing is unsupported (no public
  API to set the default endpoint).

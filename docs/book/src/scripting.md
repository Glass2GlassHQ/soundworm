# Scripting

When no TOML rule matches a node, the daemon falls through to a Rhai
script at `~/.config/soundworm/routing.rhai`. The script decides where
(or whether) the node is routed. It hot-reloads on save.

## What the script sees

- `node` : a map with `name`, `app`, `media_class`, `kind`,
  `properties`, `id`.
- `sinks` : the list of available sink node names.

## What it returns

Call one of:

- `route(target)` : link the node to the named sink.
- `allow()` : accept the node, no explicit route.
- `deny()` : block auto-routing.

Returning nothing is treated the same as `allow()`.

## Example

    // send anything from a browser to headphones if present
    if node.app == "Firefox" || node.app == "chromium" {
        if sinks.contains("headphones") {
            return route("headphones");
        }
    }

    // mute-by-default a known-noisy app
    if node.name.contains("beep") {
        return deny();
    }

    allow()

## Safety

Script runtime is bounded (`set_max_operations`), so an accidental
infinite loop aborts and logs instead of hanging the daemon. A script
that fails to compile is rejected and the previous one stays active, so a
typo never takes routing down. Check the daemon log for parse errors.

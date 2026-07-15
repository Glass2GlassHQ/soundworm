# Routing rules

Rules live in `~/.config/soundworm/rules/default.toml`. Each `[[rules]]`
entry matches a node as it appears and applies an action. Higher
`priority` wins when several rules match.

    [[rules]]
    name     = "spotify-to-speakers"
    priority = 10
    [rules.matches]
    node_name = "spotify"
    [rules.action]
    Route = { target = "alsa_output.pci-0000_00_1f.3.analog-stereo" }

## Match criteria

Under `[rules.matches]`, any of:

- `node_name` : substring match on the node name.
- `node_kind` : `Source`, `Sink`, `Filter`, or `Virtual`
  (case-insensitive).
- `property`  : `["key", "value"]`, matched against the node's
  properties.

A rule with no criteria never matches.

## Actions

Under `[rules.action]`, exactly one:

- `Route = { target = "sink-node-name" }`
- `Deny = {}`  (block auto-routing for this node)
- `SetVolume = { volume = 0.8 }`
- `Notify = { message = "..." }`

`SetVolume` and `Notify` currently log only.

## Cookbook

Route every browser stream to headphones:

    [[rules]]
    name = "browsers"
    priority = 5
    [rules.matches]
    node_name = "Firefox"
    [rules.action]
    Route = { target = "headphones" }

Block a noisy source from auto-routing:

    [[rules]]
    name = "no-notifications"
    priority = 100
    [rules.matches]
    node_name = "notify-send"
    [rules.action]
    Deny = {}

Rules are evaluated first; anything left unmatched falls through to the
Rhai script (see Scripting).

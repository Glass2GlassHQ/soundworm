# CLI reference

`sw` talks to the running daemon over the socket. Read-only commands fall
back to a direct backend when the daemon is down.

| command                     | what it does                                  |
|-----------------------------|-----------------------------------------------|
| `sw list`                   | list audio nodes                              |
| `sw link <src> <sink>`      | link a source node to a sink node (by name)   |
| `sw unlink <link-id>`       | remove a link by id                           |
| `sw watch`                  | stream live events until interrupted          |
| `sw snapshot save <name>`   | save the current link set                     |
| `sw snapshot load <name>`   | restore a saved snapshot                      |
| `sw snapshot list`          | list saved snapshots                          |
| `sw rules load <path>`      | load a rules TOML file                         |
| `sw rules reload`           | reload the active rules file                  |
| `sw script load <path>`     | load a Rhai routing script                    |
| `sw script reload`          | reload the active script                      |
| `sw metrics`                | print xrun + latency metrics                  |
| `sw metrics --json`         | same, as JSON                                 |
| `sw metrics --watch`        | stream xrun events                            |
| `sw shutdown`               | ask the daemon to exit                        |

`sw link` takes node names and links their matching ports. The link id
printed by `sw link` is the one `sw unlink` expects.

Example:

    sw list
    sw link spotify alsa_output.pci-0000_00_1f.3.analog-stereo
    sw snapshot save music

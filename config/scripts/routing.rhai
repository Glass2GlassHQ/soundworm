// soundworm routing script — evaluated once per NodeAppeared event.
//
// Scope:
//   node  : map with .name, .app, .media_class, .kind, .properties, .id
//   sinks : array of sink-node names currently in the graph
//
// Return one of:
//   route("target-node-name")  -- queue a link to that node
//   allow()                    -- explicitly do nothing (no link)
//   deny()                     -- emit a LinkRejected event
//   ()                         -- fall through (treated as no-op)

if node.name == "spotify" || node.name == "vlc" {
    log_route(node.name, "speakers");
    route("speakers")
} else if node.name == "zoom" || node.name == "teams" {
    log_route(node.name, "usb_headset");
    route("usb_headset")
} else if node.media_class == "Stream/Output/Audio" && sinks.len > 0 {
    route(sinks[0])
} else {
    ()
}

use crate::{link::Link, node::Node, port::Port, link::LinkId, node::NodeId, port::PortId};

// Non-exhaustive: backends gain event kinds over time (the profiler POD
// path, per-device events); downstream matchers must tolerate unknowns.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BackendEvent {
    NodeAppeared(Node),
    NodeRemoved(NodeId),
    PortAppeared(Port),
    PortRemoved(PortId),
    LinkAppeared(Link),
    LinkRemoved(LinkId),
    /// Backend observed a buffer underrun / xrun on `node_id`.
    /// `gap_ms` is the duration of the missed buffer.
    Xrun { node_id: NodeId, gap_ms: f32 },
    /// Backend sampled a fresh per-link latency reading.
    LatencySample { node_id: NodeId, latency_ms: f32 },
}

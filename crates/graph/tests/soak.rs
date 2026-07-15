// Soak the in-memory AudioGraph under sustained synthetic churn and
// assert it stays consistent and leak-free. This is the bounded, CI-run
// proxy for the v0.7 24h daemon soak: the graph is the stateful piece
// that could leak nodes/ports/links or desync under add/remove pressure.

use soundworm_core::{
    event::BackendEvent,
    link::{Link, LinkId},
    node::{Node, NodeId, NodeKind},
    port::{Direction, Port, PortId},
};
use soundworm_graph::AudioGraph;
use std::collections::HashMap;

// Deterministic PRNG so failures reproduce exactly (no rand dep, and the
// harness forbids nondeterministic time/random anyway).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 16
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

fn node(id: u64) -> Node {
    Node {
        id: NodeId(id),
        name: format!("node-{id}"),
        kind: NodeKind::Filter,
        app_name: None,
        media_class: "Audio/Filter".into(),
        sample_rate: 48000,
        channels: 2,
        latency_ms: 0.0,
        properties: HashMap::new(),
    }
}
fn port(id: u64, node_id: u64) -> Port {
    Port { id: PortId(id), node_id: NodeId(node_id), name: format!("p-{id}"), direction: Direction::Output, channels: 1 }
}
fn link(id: u64, src: u64, sink: u64) -> Link {
    Link { id: LinkId(id), source_port: PortId(src), sink_port: PortId(sink), latency_compensation_ms: 0.0 }
}

// Fast regression guard, runs in CI.
#[test]
fn graph_stays_consistent_under_churn() {
    churn(20_000);
}

// Heavy on-demand soak (the v0.7 long-run proxy). Excluded from normal
// CI because draining N nodes is O(N * ports); run with:
//   cargo test -p soundworm-graph --test soak -- --ignored
#[test]
#[ignore = "heavy soak; run explicitly with --ignored"]
fn graph_soak_heavy() {
    churn(500_000);
}

fn churn(iters: usize) {
    let mut g = AudioGraph::new();
    let mut rng = Rng(0x5eed);

    let mut live_nodes: Vec<u64> = Vec::new();
    let mut ports_of: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut live_links: Vec<u64> = Vec::new();
    // Every id ever minted, to drain exhaustively at the end.
    let mut all_nodes: Vec<u64> = Vec::new();
    let mut all_links: Vec<u64> = Vec::new();
    let mut next_id: u64 = 1;

    for i in 0..iters {
        match rng.below(10) {
            // Add a node with two ports (weighted so the graph grows then churns).
            0..=2 => {
                let nid = next_id; next_id += 1;
                g.apply_event(BackendEvent::NodeAppeared(node(nid)));
                let mut ps = Vec::new();
                for _ in 0..2 {
                    let pid = next_id; next_id += 1;
                    g.apply_event(BackendEvent::PortAppeared(port(pid, nid)));
                    ps.push(pid);
                }
                ports_of.insert(nid, ps);
                live_nodes.push(nid);
                all_nodes.push(nid);
            }
            // Remove a random node: graph must cascade its ports away.
            3 | 4 => {
                if !live_nodes.is_empty() {
                    let idx = rng.below(live_nodes.len());
                    let nid = live_nodes.swap_remove(idx);
                    g.apply_event(BackendEvent::NodeRemoved(NodeId(nid)));
                    ports_of.remove(&nid);
                }
            }
            // Add a link between two live ports. Sample O(1) by picking
            // random live nodes then a port each (avoids scanning ports).
            5..=7 => {
                if live_nodes.len() >= 2 {
                    let a_ports = &ports_of[&live_nodes[rng.below(live_nodes.len())]];
                    let b_ports = &ports_of[&live_nodes[rng.below(live_nodes.len())]];
                    if !a_ports.is_empty() && !b_ports.is_empty() {
                        let a = a_ports[rng.below(a_ports.len())];
                        let b = b_ports[rng.below(b_ports.len())];
                        let lid = next_id; next_id += 1;
                        g.apply_event(BackendEvent::LinkAppeared(link(lid, a, b)));
                        live_links.push(lid);
                        all_links.push(lid);
                    }
                }
            }
            // Remove a random link.
            _ => {
                if !live_links.is_empty() {
                    let idx = rng.below(live_links.len());
                    let lid = live_links.swap_remove(idx);
                    g.apply_event(BackendEvent::LinkRemoved(LinkId(lid)));
                }
            }
        }

        // Consistency invariant, checked periodically: no port may outlive
        // its owning node (remove_node must cascade).
        if i % 5000 == 0 {
            for p in g.ports() {
                assert!(
                    g.get_node(&p.node_id).is_some(),
                    "orphan port {:?} references dead node {:?} at iter {i}",
                    p.id, p.node_id
                );
            }
            // Node count can never exceed the number ever created.
            assert!(g.nodes().count() <= all_nodes.len(), "node count exceeds minted ids");
        }
    }

    // Drain everything ever created (idempotent for already-removed ids).
    // Links first so nothing references a port mid-drain.
    for lid in &all_links {
        g.apply_event(BackendEvent::LinkRemoved(LinkId(*lid)));
    }
    for nid in &all_nodes {
        g.apply_event(BackendEvent::NodeRemoved(NodeId(*nid)));
    }

    // Leak check: a fully-drained graph holds nothing.
    assert_eq!(g.nodes().count(), 0, "leaked nodes after full drain");
    assert_eq!(g.ports().count(), 0, "leaked ports after full drain");
    assert_eq!(g.links().count(), 0, "leaked links after full drain");
}

// Exercise the MockBackend subscribe/emit path under volume, draining as
// we go so the bounded channel never overflows (which would silently drop
// the subscriber).
#[test]
fn mock_backend_emit_drain_roundtrip() {
    use soundworm_core::backend::AudioBackend;
    use soundworm_graph::mock::MockBackend;

    let backend = MockBackend::new();
    let rx = backend.subscribe();
    let mut g = AudioGraph::new();

    const N: u64 = 10_000;
    for id in 1..=N {
        backend.emit(BackendEvent::NodeAppeared(node(id)));
        // Drain immediately to stay under the 256 channel bound.
        while let Ok(ev) = rx.try_recv() {
            g.apply_event(ev);
        }
    }
    while let Ok(ev) = rx.try_recv() {
        g.apply_event(ev);
    }
    assert_eq!(g.nodes().count() as u64, N, "every emitted node reached the graph");
}

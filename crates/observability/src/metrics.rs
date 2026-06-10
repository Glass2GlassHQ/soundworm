//! Per-node latency metrics backed by [`hdrhistogram::Histogram`].
//!
//! Values are stored as integer microseconds (the histogram tracks
//! integer values); the public API still takes/returns milliseconds.

use hdrhistogram::Histogram;
use serde::Serialize;
use soundworm_core::node::NodeId;
use std::collections::HashMap;

/// Histogram tracks 1 µs .. 10 s with 2 significant figures.
const HIST_MAX_US: u64 = 10_000_000;

pub struct Metrics {
    latency: HashMap<NodeId, Histogram<u64>>,
}

impl Metrics {
    pub fn record_latency_ms(&mut self, id: NodeId, ms: f32) {
        let us = (ms * 1000.0).round().max(1.0) as u64;
        let h = self
            .latency
            .entry(id)
            .or_insert_with(|| Histogram::<u64>::new_with_max(HIST_MAX_US, 2).unwrap());
        let _ = h.record(us.min(HIST_MAX_US));
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut nodes: Vec<NodeLatency> = self
            .latency
            .iter()
            .map(|(id, h)| NodeLatency {
                node_id: id.clone(),
                count: h.len(),
                min_ms: us_to_ms(h.min()),
                p50_ms: us_to_ms(h.value_at_quantile(0.50)),
                p95_ms: us_to_ms(h.value_at_quantile(0.95)),
                p99_ms: us_to_ms(h.value_at_quantile(0.99)),
                max_ms: us_to_ms(h.max()),
            })
            .collect();
        nodes.sort_by_key(|n| n.node_id.0);
        MetricsSnapshot { nodes }
    }
}

fn us_to_ms(us: u64) -> f32 { us as f32 / 1000.0 }

impl Default for Metrics {
    fn default() -> Self { Self { latency: HashMap::new() } }
}

/// Wire-stable snapshot of all per-node latency stats.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub nodes: Vec<NodeLatency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeLatency {
    pub node_id: NodeId,
    pub count:   u64,
    pub min_ms:  f32,
    pub p50_ms:  f32,
    pub p95_ms:  f32,
    pub p99_ms:  f32,
    pub max_ms:  f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_roughly_match() {
        let mut m = Metrics::default();
        for i in 1..=100 {
            m.record_latency_ms(NodeId(1), i as f32);
        }
        let snap = m.snapshot();
        let n = &snap.nodes[0];
        assert_eq!(n.count, 100);
        assert!(n.p50_ms >= 49.0 && n.p50_ms <= 52.0, "p50={}", n.p50_ms);
        assert!(n.p99_ms >= 98.0 && n.p99_ms <= 100.5, "p99={}", n.p99_ms);
    }

    #[test]
    fn separate_nodes_separate_histograms() {
        let mut m = Metrics::default();
        m.record_latency_ms(NodeId(1), 5.0);
        m.record_latency_ms(NodeId(2), 50.0);
        let snap = m.snapshot();
        assert_eq!(snap.nodes.len(), 2);
    }
}

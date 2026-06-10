//! Bounded xrun ring buffer + per-node counters.
//!
//! Drops the oldest event when [`XrunLog::CAP`] is exceeded, so memory
//! stays flat under sustained churn.

use serde::Serialize;
use soundworm_core::node::NodeId;
use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize)]
pub struct Xrun {
    pub node_id: NodeId,
    /// Seconds since UNIX epoch. Wall-clock; not monotonic.
    pub timestamp_secs: u64,
    pub gap_ms: f32,
}

pub struct XrunLog {
    events: VecDeque<Xrun>,
    counts: HashMap<NodeId, u64>,
    total: u64,
}

impl XrunLog {
    /// Last-N ring size. Sized for ~10 minutes at one xrun/sec.
    pub const CAP: usize = 1024;

    pub fn record(&mut self, node_id: NodeId, gap_ms: f32) {
        tracing::warn!("xrun on {:?}: {:.2}ms", node_id, gap_ms);
        let timestamp_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        *self.counts.entry(node_id.clone()).or_insert(0) += 1;
        self.total += 1;
        if self.events.len() == Self::CAP {
            self.events.pop_front();
        }
        self.events.push_back(Xrun { node_id, timestamp_secs, gap_ms });
    }

    pub fn recent(&self, n: usize) -> Vec<Xrun> {
        let len = self.events.len();
        self.events.iter().skip(len.saturating_sub(n)).cloned().collect()
    }

    pub fn worst_offender(&self) -> Option<NodeId> {
        self.counts
            .iter()
            .max_by_key(|(_, c)| **c)
            .map(|(id, _)| id.clone())
    }

    pub fn count_for(&self, id: &NodeId) -> u64 {
        self.counts.get(id).copied().unwrap_or(0)
    }

    pub fn total(&self) -> u64 { self.total }

    pub fn counts(&self) -> &HashMap<NodeId, u64> { &self.counts }
}

impl Default for XrunLog {
    fn default() -> Self {
        Self { events: VecDeque::with_capacity(Self::CAP), counts: HashMap::new(), total: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_counts() {
        let mut log = XrunLog::default();
        log.record(NodeId(1), 2.5);
        log.record(NodeId(2), 10.0);
        log.record(NodeId(1), 1.0);
        assert_eq!(log.total(), 3);
        assert_eq!(log.count_for(&NodeId(1)), 2);
        assert_eq!(log.worst_offender(), Some(NodeId(1)));
    }

    #[test]
    fn ring_is_bounded() {
        let mut log = XrunLog::default();
        for i in 0..(XrunLog::CAP + 10) {
            log.record(NodeId(i as u64), 1.0);
        }
        assert_eq!(log.recent(usize::MAX).len(), XrunLog::CAP);
        assert_eq!(log.total(), (XrunLog::CAP + 10) as u64);
    }
}

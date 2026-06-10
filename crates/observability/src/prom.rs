//! Minimal Prometheus text-format exporter for [`MetricsSnapshot`] and
//! [`XrunLog`]. Enabled with the `prometheus` cargo feature. We hand-roll
//! the format string rather than pull in the `prometheus` crate — the
//! exposition surface is small and stable.

use crate::{metrics::MetricsSnapshot, xrun::XrunLog};
use std::fmt::Write;

pub fn render(snap: &MetricsSnapshot, xruns: &XrunLog) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# HELP soundworm_xrun_total total xruns observed");
    let _ = writeln!(s, "# TYPE soundworm_xrun_total counter");
    let _ = writeln!(s, "soundworm_xrun_total {}", xruns.total());
    let _ = writeln!(s, "# HELP soundworm_xrun_count xruns by node");
    let _ = writeln!(s, "# TYPE soundworm_xrun_count counter");
    for (id, c) in xruns.counts() {
        let _ = writeln!(s, "soundworm_xrun_count{{node=\"{}\"}} {}", id.0, c);
    }

    let _ = writeln!(s, "# HELP soundworm_latency_ms link latency quantiles");
    let _ = writeln!(s, "# TYPE soundworm_latency_ms summary");
    for n in &snap.nodes {
        let _ = writeln!(
            s,
            "soundworm_latency_ms{{node=\"{}\",quantile=\"0.5\"}} {}",
            n.node_id.0, n.p50_ms
        );
        let _ = writeln!(
            s,
            "soundworm_latency_ms{{node=\"{}\",quantile=\"0.95\"}} {}",
            n.node_id.0, n.p95_ms
        );
        let _ = writeln!(
            s,
            "soundworm_latency_ms{{node=\"{}\",quantile=\"0.99\"}} {}",
            n.node_id.0, n.p99_ms
        );
        let _ = writeln!(
            s,
            "soundworm_latency_ms_count{{node=\"{}\"}} {}",
            n.node_id.0, n.count
        );
    }
    s
}

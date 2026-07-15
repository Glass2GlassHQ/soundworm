#![no_main]
// Compiling and evaluating a rhai routing script must never panic or
// hang. Any script that compiles is then evaluated against a fixed node
// so the runtime op-limit guard (set_max_operations) is exercised on
// arbitrary control flow — an unbounded loop should abort, not spin.
use libfuzzer_sys::fuzz_target;
use soundworm_core::node::{Node, NodeId, NodeKind};
use soundworm_rhai::ScriptEngine;
use std::collections::HashMap;

fuzz_target!(|data: &[u8]| {
    let Ok(script) = std::str::from_utf8(data) else { return };
    let Ok(engine) = ScriptEngine::load_str(script) else { return };
    let node = Node {
        id: NodeId(1),
        name: "fuzz".into(),
        kind: NodeKind::Source,
        app_name: None,
        media_class: "Stream/Output/Audio".into(),
        sample_rate: 48000,
        channels: 2,
        latency_ms: 0.0,
        properties: HashMap::new(),
    };
    let _ = engine.evaluate(&node, &["speakers".to_string()]);
});

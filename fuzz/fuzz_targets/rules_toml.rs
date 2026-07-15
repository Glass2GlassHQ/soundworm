#![no_main]
// Rules TOML parsing must never panic on malformed input — only return
// an error. Feeds arbitrary bytes (as UTF-8) to RulesEngine::load_toml.
use libfuzzer_sys::fuzz_target;
use soundworm_policy::rules::RulesEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let mut engine = RulesEngine::default();
        let _ = engine.load_toml(s);
    }
});

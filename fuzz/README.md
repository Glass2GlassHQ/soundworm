# soundworm-fuzz

cargo-fuzz targets for the two parsers that ingest untrusted-ish input.

    cargo install cargo-fuzz
    cargo +nightly fuzz run rules_toml
    cargo +nightly fuzz run rhai_script

- `rules_toml` — `RulesEngine::load_toml`, must never panic on malformed TOML.
- `rhai_script` — `ScriptEngine::load_str` then `evaluate`, exercising the
  runtime op-limit guard against arbitrary scripts (no panic, no hang).

Not part of the default workspace (own `[workspace]`), so `cargo build
--workspace` and CI skip it. Run manually or in a dedicated fuzz job.

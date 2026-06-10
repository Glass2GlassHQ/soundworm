//! Rhai scripting engine for soundworm.
//!
//! A single `routing.rhai` script is evaluated once per `NodeAppeared`
//! event. The script sees the node as `node` (a Rhai map) and a list of
//! available sink names as `sinks`. It returns a [`Decision`] via the
//! registered `route(target)` / `allow()` / `deny()` builtins.
//!
//! Reload is atomic: a new script is parsed and compiled into a fresh
//! engine before swapping into place — a malformed script never replaces
//! a working one.

use anyhow::{anyhow, Result};
use rhai::{Dynamic, Engine, Map, AST};
use soundworm_core::node::Node;
use std::path::{Path, PathBuf};

/// Maximum Rhai operations per script invocation. Cheap proxy for a
/// wall-clock timeout; ~1 ms on commodity hardware.
const MAX_OPERATIONS: u64 = 100_000;

/// Outcome of evaluating the routing script against a single node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Route(String),
    Allow,
    Deny,
    None,
}

impl Decision {
    fn from_dynamic(d: Dynamic) -> Decision {
        if d.is_unit() {
            return Decision::None;
        }
        if let Some(m) = d.read_lock::<Map>() {
            match m.get("kind").and_then(|v| v.clone().into_string().ok()).as_deref() {
                Some("route") => {
                    if let Some(t) = m.get("target").and_then(|v| v.clone().into_string().ok()) {
                        return Decision::Route(t);
                    }
                }
                Some("allow") => return Decision::Allow,
                Some("deny") => return Decision::Deny,
                _ => {}
            }
        }
        Decision::None
    }
}

/// A loaded routing script bound to an engine. `evaluate` is cheap; only
/// `load_from_path` (or `load_str`) does parsing.
pub struct ScriptEngine {
    engine: Engine,
    ast: AST,
    source_path: Option<PathBuf>,
}

impl ScriptEngine {
    /// Build a fresh engine + compile `script`. Use this when you want
    /// the script body without it touching the filesystem.
    pub fn load_str(script: &str) -> Result<Self> {
        let engine = make_engine();
        let ast = engine
            .compile(script)
            .map_err(|e| anyhow!("rhai parse error: {e}"))?;
        Ok(Self { engine, ast, source_path: None })
    }

    /// Read a script from disk and compile it. Records the path so
    /// [`reload`] can re-read the same file.
    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let body = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("read {}: {e}", path.display()))?;
        let mut me = Self::load_str(&body)?;
        me.source_path = Some(path);
        Ok(me)
    }

    /// Re-read the previously loaded path and compile. Returns a *new*
    /// [`ScriptEngine`]; on parse failure the caller keeps the old one.
    pub fn reload(&self) -> Result<Self> {
        let path = self
            .source_path
            .clone()
            .ok_or_else(|| anyhow!("no script path to reload"))?;
        Self::load_from_path(path)
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    /// Evaluate the script with `node` and `sinks` injected as constants.
    /// Caps execution at [`MAX_OPERATIONS`]; on overrun returns
    /// `Decision::None` and logs.
    pub fn evaluate(&self, node: &Node, sinks: &[String]) -> Decision {
        let mut scope = rhai::Scope::new();
        scope.push_constant("node", node_to_map(node));
        scope.push_constant(
            "sinks",
            sinks.iter().cloned().map(Dynamic::from).collect::<Vec<_>>(),
        );

        match self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &self.ast) {
            Ok(d) => Decision::from_dynamic(d),
            Err(e) => {
                tracing::warn!("rhai eval failed for node '{}': {e}", node.name);
                Decision::None
            }
        }
    }
}

fn node_to_map(n: &Node) -> Map {
    let mut m = Map::new();
    m.insert("id".into(), Dynamic::from(n.id.0 as i64));
    m.insert("name".into(), Dynamic::from(n.name.clone()));
    m.insert(
        "app".into(),
        n.app_name.clone().map(Dynamic::from).unwrap_or(Dynamic::UNIT),
    );
    m.insert("media_class".into(), Dynamic::from(n.media_class.clone()));
    m.insert("kind".into(), Dynamic::from(format!("{:?}", n.kind)));
    let props: Map = n
        .properties
        .iter()
        .map(|(k, v)| (k.clone().into(), Dynamic::from(v.clone())))
        .collect();
    m.insert("properties".into(), Dynamic::from(props));
    m
}

fn make_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);

    // route("target") -> #{ kind: "route", target: <target> }
    engine.register_fn("route", |target: &str| -> Map {
        let mut m = Map::new();
        m.insert("kind".into(), Dynamic::from("route".to_string()));
        m.insert("target".into(), Dynamic::from(target.to_string()));
        m
    });
    engine.register_fn("allow", || -> Map {
        let mut m = Map::new();
        m.insert("kind".into(), Dynamic::from("allow".to_string()));
        m
    });
    engine.register_fn("deny", || -> Map {
        let mut m = Map::new();
        m.insert("kind".into(), Dynamic::from("deny".to_string()));
        m
    });
    engine.register_fn("log_route", |from: &str, to: &str| {
        tracing::info!("[rhai] route: {from} → {to}");
    });
    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundworm_core::node::{Node, NodeId, NodeKind};

    fn n(name: &str) -> Node {
        Node {
            id: NodeId(1),
            name: name.into(),
            kind: NodeKind::Source,
            app_name: Some("Firefox".into()),
            media_class: "Stream/Output/Audio".into(),
            sample_rate: 48000,
            channels: 2,
            latency_ms: 0.0,
            properties: Default::default(),
        }
    }

    #[test]
    fn route_target() {
        let s = ScriptEngine::load_str(r#"if node.name == "spotify" { route("speakers") } else { deny() }"#).unwrap();
        assert_eq!(s.evaluate(&n("spotify"), &[]), Decision::Route("speakers".into()));
        assert_eq!(s.evaluate(&n("vlc"), &[]), Decision::Deny);
    }

    #[test]
    fn media_class_match() {
        let s = ScriptEngine::load_str(
            r#"if node.media_class == "Stream/Output/Audio" { route("default") } else { allow() }"#,
        )
        .unwrap();
        assert_eq!(s.evaluate(&n("anything"), &[]), Decision::Route("default".into()));
    }

    #[test]
    fn no_decision_when_script_returns_unit() {
        let s = ScriptEngine::load_str("let x = 1;").unwrap();
        assert_eq!(s.evaluate(&n("x"), &[]), Decision::None);
    }

    #[test]
    fn parse_error_does_not_panic() {
        let err = ScriptEngine::load_str("this is not rhai !!!").err().unwrap();
        assert!(err.to_string().contains("rhai parse error"));
    }

    #[test]
    fn runaway_script_is_aborted() {
        let s = ScriptEngine::load_str("let i = 0; loop { i += 1; }").unwrap();
        assert_eq!(s.evaluate(&n("x"), &[]), Decision::None);
    }
}

use serde::{Deserialize, Serialize};
use soundworm_core::node::Node;
use toml;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub name:     String,
    pub priority: i32,
    pub matches:  MatchCriteria,
    pub action:   Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchCriteria {
    pub node_name: Option<String>,
    pub node_kind: Option<String>,
    pub property:  Option<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Route     { target: String },
    SetVolume { volume: f32 },
    Deny,
    Notify    { message: String },
}

#[derive(Default)]
pub struct RulesEngine { rules: Vec<RoutingRule> }

impl RulesEngine {
    pub fn load_toml(&mut self, content: &str) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct RuleFile { rules: Vec<RoutingRule> }
        let file: RuleFile = toml::from_str(content)?;
        self.rules.extend(file.rules);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(())
    }

    pub fn evaluate(&self, node_name: &str) -> Option<&Action> {
        self.evaluate_rule(node_name).map(|r| &r.action)
    }

    /// Same as [`evaluate`] but returns the matching rule so callers can
    /// access its name (for logging and `RulesApplied` events).
    pub fn evaluate_rule(&self, node_name: &str) -> Option<&RoutingRule> {
        self.rules.iter().find(|r| {
            r.matches.node_name.as_deref() == Some(node_name)
                && r.matches.node_kind.is_none()
                && r.matches.property.is_none()
        })
    }

    /// Evaluate against a full [`Node`], honoring all three predicates.
    /// All non-`None` predicates must match for the rule to fire.
    pub fn evaluate_node(&self, node: &Node) -> Option<&RoutingRule> {
        self.rules.iter().find(|r| {
            let m = &r.matches;
            if let Some(n) = &m.node_name {
                if n != &node.name { return false; }
            }
            if let Some(k) = &m.node_kind {
                if !node_kind_matches(k, &node.kind) { return false; }
            }
            if let Some((k, v)) = &m.property {
                match node.properties.get(k) {
                    Some(val) if val == v => {}
                    _ => return false,
                }
            }
            // At least one predicate must be set, else this would match every node.
            m.node_name.is_some() || m.node_kind.is_some() || m.property.is_some()
        })
    }

    pub fn rule_count(&self) -> usize { self.rules.len() }
}

fn node_kind_matches(spec: &str, kind: &soundworm_core::node::NodeKind) -> bool {
    use soundworm_core::node::NodeKind;
    matches!(
        (spec.to_ascii_lowercase().as_str(), kind),
        ("source",  NodeKind::Source)
        | ("sink",    NodeKind::Sink)
        | ("filter",  NodeKind::Filter)
        | ("virtual", NodeKind::Virtual)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[rules]]
name     = "spotify"
priority = 10
[rules.matches]
node_name = "spotify"
[rules.action]
Route = { target = "speakers" }
"#;

    #[test]
    fn test_load_and_evaluate() {
        let mut e = RulesEngine::default();
        e.load_toml(SAMPLE).unwrap();
        assert!(matches!(e.evaluate("spotify"), Some(Action::Route { .. })));
    }

    #[test]
    fn test_no_match() {
        let mut e = RulesEngine::default();
        e.load_toml(SAMPLE).unwrap();
        assert!(e.evaluate("unknown").is_none());
    }

    #[test]
    fn test_property_predicate() {
        use soundworm_core::node::{Node, NodeId, NodeKind};
        const T: &str = r#"
[[rules]]
name     = "by-prop"
priority = 10
[rules.matches]
property = ["application.name", "Spotify"]
[rules.action]
Route = { target = "speakers" }
"#;
        let mut e = RulesEngine::default();
        e.load_toml(T).unwrap();
        let mut n = Node {
            id: NodeId(1),
            name: "anything".into(),
            kind: NodeKind::Source,
            app_name: None,
            media_class: String::new(),
            sample_rate: 0,
            channels: 0,
            latency_ms: 0.0,
            properties: Default::default(),
        };
        assert!(e.evaluate_node(&n).is_none());
        n.properties.insert("application.name".into(), "Spotify".into());
        assert!(matches!(e.evaluate_node(&n).map(|r| &r.action), Some(Action::Route { .. })));
    }

    #[test]
    fn test_node_kind_predicate() {
        use soundworm_core::node::{Node, NodeId, NodeKind};
        const T: &str = r#"
[[rules]]
name     = "all-sinks"
priority = 1
[rules.matches]
node_kind = "Sink"
[rules.action]
Route = { target = "void" }
"#;
        let mut e = RulesEngine::default();
        e.load_toml(T).unwrap();
        let mut n = Node {
            id: NodeId(1),
            name: "x".into(),
            kind: NodeKind::Source,
            app_name: None,
            media_class: String::new(),
            sample_rate: 0,
            channels: 0,
            latency_ms: 0.0,
            properties: Default::default(),
        };
        assert!(e.evaluate_node(&n).is_none());
        n.kind = NodeKind::Sink;
        assert!(matches!(e.evaluate_node(&n).map(|r| &r.action), Some(Action::Route { .. })));
    }
}

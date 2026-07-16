use anyhow::{anyhow, Result};
use soundworm_core::node::NodeId;
use soundworm_ipc::{client::Client, default_socket_path, Op, ResponseData};

// Resolve a node argument (numeric id or exact node name) to a NodeId
// via a ListNodes round-trip.
async fn resolve_node(c: &mut Client, arg: &str) -> Result<NodeId> {
    let nodes = match c.request(Op::ListNodes).await? {
        ResponseData::Nodes { nodes } => nodes,
        _ => return Err(anyhow!("unexpected response from daemon")),
    };
    if let Ok(id) = arg.parse::<u64>() {
        if nodes.iter().any(|n| n.node.id.0 == id) {
            return Ok(NodeId(id));
        }
    }
    let mut named = nodes.iter().filter(|n| n.node.name == arg);
    match (named.next(), named.next()) {
        (Some(n), None) => Ok(n.node.id.clone()),
        (Some(_), Some(_)) => Err(anyhow!("multiple nodes named '{arg}'; use the numeric id")),
        _ => Err(anyhow!("no node matching '{arg}'")),
    }
}

pub async fn volume(args: &[String]) -> Result<()> {
    let node = args.get(2).ok_or_else(|| anyhow!("Usage: sw volume <node> <0.0..1.0>"))?;
    let level: f32 = args
        .get(3)
        .ok_or_else(|| anyhow!("Usage: sw volume <node> <0.0..1.0>"))?
        .parse()
        .map_err(|_| anyhow!("volume must be a number in 0.0..1.0"))?;

    let mut c = Client::connect(&default_socket_path()).await?;
    let node_id = resolve_node(&mut c, node).await?;
    c.request(Op::SetVolume { node: node_id.clone(), volume: level }).await?;
    println!("Set volume of node {} to {:.2}", node_id.0, level.clamp(0.0, 1.0));
    Ok(())
}

pub async fn mute(args: &[String]) -> Result<()> {
    let node = args.get(2).ok_or_else(|| anyhow!("Usage: sw mute <node> <on|off>"))?;
    let mute = match args.get(3).map(String::as_str) {
        Some("on" | "true" | "1" | "mute") => true,
        Some("off" | "false" | "0" | "unmute") => false,
        _ => return Err(anyhow!("Usage: sw mute <node> <on|off>")),
    };

    let mut c = Client::connect(&default_socket_path()).await?;
    let node_id = resolve_node(&mut c, node).await?;
    c.request(Op::SetMute { node: node_id.clone(), mute }).await?;
    println!("{} node {}", if mute { "Muted" } else { "Unmuted" }, node_id.0);
    Ok(())
}

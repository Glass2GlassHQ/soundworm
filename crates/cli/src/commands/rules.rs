use anyhow::{anyhow, Result};
use soundworm_ipc::{client::Client, default_socket_path, Op, ResponseData};

pub async fn run(args: &[String]) -> Result<()> {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
    let mut c = Client::connect(&default_socket_path()).await?;
    match sub {
        "load" => {
            let path = args
                .get(3)
                .ok_or_else(|| anyhow!("Usage: sw rules load <path>"))?;
            match c.request(Op::LoadRules { path: path.clone() }).await? {
                ResponseData::Rules { rule_count } => {
                    println!("Loaded {} rule(s) from {}", rule_count, path);
                }
                _ => return Err(anyhow!("unexpected response from daemon")),
            }
        }
        "reload" => {
            match c.request(Op::ReloadRules).await? {
                ResponseData::Rules { rule_count } => {
                    println!("Reloaded {} rule(s)", rule_count);
                }
                _ => return Err(anyhow!("unexpected response from daemon")),
            }
        }
        _ => return Err(anyhow!("Usage: sw rules <load <path> | reload>")),
    }
    Ok(())
}

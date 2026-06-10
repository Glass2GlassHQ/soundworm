use anyhow::{anyhow, Result};
use soundworm_ipc::{client::Client, default_socket_path, Op, ResponseData};

pub async fn run(args: &[String]) -> Result<()> {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
    let mut c = Client::connect(&default_socket_path()).await?;
    match sub {
        "load" => {
            let path = args
                .get(3)
                .ok_or_else(|| anyhow!("Usage: sw script load <path>"))?;
            match c.request(Op::LoadScript { path: path.clone() }).await? {
                ResponseData::Script { path } => {
                    println!("Loaded routing script {}", path);
                }
                _ => return Err(anyhow!("unexpected response from daemon")),
            }
        }
        "reload" => {
            match c.request(Op::ReloadScript).await? {
                ResponseData::Script { path } => {
                    println!("Reloaded routing script {}", path);
                }
                _ => return Err(anyhow!("unexpected response from daemon")),
            }
        }
        _ => return Err(anyhow!("Usage: sw script <load <path> | reload>")),
    }
    Ok(())
}

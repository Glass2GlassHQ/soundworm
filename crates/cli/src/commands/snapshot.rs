use anyhow::{anyhow, Result};
use soundworm_ipc::{client::Client, default_socket_path, Op, ResponseData};
use soundworm_snapshots as snapshots;

pub async fn run(args: &[String]) -> Result<()> {
    let sub  = args.get(2).map(|s| s.as_str()).unwrap_or("list");
    let name = args.get(3).map(|s| s.as_str()).unwrap_or("default");

    match sub {
        "save" => {
            let mut c = Client::connect(&default_socket_path()).await?;
            match c.request(Op::Snapshot { name: name.to_owned() }).await? {
                ResponseData::Snapshot { path } => {
                    println!("Saved snapshot '{}' → {}", name, path);
                }
                _ => return Err(anyhow!("unexpected response from daemon")),
            }
        }
        "load" => {
            let mut c = Client::connect(&default_socket_path()).await?;
            match c.request(Op::Restore { name: name.to_owned() }).await? {
                ResponseData::Restore { applied, skipped } => {
                    println!(
                        "Restored snapshot '{}': {} applied, {} skipped",
                        name, applied, skipped
                    );
                }
                _ => return Err(anyhow!("unexpected response from daemon")),
            }
        }
        "list" => {
            let names = snapshots::list().await?;
            if names.is_empty() {
                println!("No saved snapshots.");
            } else {
                for n in names { println!("  {}", n); }
            }
        }
        other => return Err(anyhow!("Unknown snapshot subcommand '{}'", other)),
    }
    Ok(())
}

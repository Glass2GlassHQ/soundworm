use anyhow::Result;
use soundworm_ipc::{client::Client, default_socket_path, Op};

pub async fn run() -> Result<()> {
    let mut c = Client::connect(&default_socket_path()).await?;
    c.request(Op::Shutdown).await?;
    println!("Daemon shutdown requested");
    Ok(())
}

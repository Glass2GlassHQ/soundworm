mod commands;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into()))
        .init();

    let raw: Vec<String> = std::env::args().collect();
    let in_process = raw.iter().any(|a| a == "--in-process");
    let args: Vec<String> = raw.into_iter().filter(|a| a != "--in-process").collect();

    match args.get(1).map(|s| s.as_str()).unwrap_or("help") {
        "list"     => commands::list::run(in_process).await,
        "link"     => commands::link::run(&args, in_process).await,
        "unlink"   => commands::link::unlink(&args, in_process).await,
        "volume"   => commands::volume::volume(&args).await,
        "mute"     => commands::volume::mute(&args).await,
        "watch"    => commands::watch::run().await,
        "snapshot" => commands::snapshot::run(&args).await,
        "rules"    => commands::rules::run(&args).await,
        "script"   => commands::script::run(&args).await,
        "shutdown" => commands::shutdown::run().await,
        "metrics"  => commands::metrics::run(&args).await,
        _ => {
            println!("soundworm (sw) — cross-platform audio router");
            println!();
            println!("USAGE:");
            println!("  sw list                     List audio nodes (via daemon)");
            println!("  sw link   <src> <sink>      Create route (via daemon)");
            println!("  sw unlink <link-id>         Remove route (via daemon)");
            println!("  sw volume <node> <0..1>     Set node volume (via daemon)");
            println!("  sw mute   <node> <on|off>   Mute/unmute node (via daemon)");
            println!("  sw watch                    Stream live events from daemon");
            println!("  sw snapshot save <name>     Save session (via daemon)");
            println!("  sw snapshot load <name>     Restore session (via daemon)");
            println!("  sw snapshot list            List sessions on disk");
            println!("  sw rules load <path>        Load TOML rules into daemon");
            println!("  sw rules reload             Re-read last loaded rules");
            println!("  sw script load <path>       Load Rhai routing script");
            println!("  sw script reload            Re-read last loaded script");
            println!("  sw shutdown                 Ask daemon to exit cleanly");
            println!("  sw metrics [--watch|--json] Latency + xrun stats (watch streams xruns)");
            println!();
            println!("FLAGS:");
            println!("  --in-process                Bypass daemon, talk to backend directly");
            println!("                              (test escape hatch — list/link/unlink only)");
            println!();
            println!("ENV:  RUST_LOG=debug sw ...   SOUNDWORM_SOCK=<path> sw ...");
            Ok(())
        }
    }
}

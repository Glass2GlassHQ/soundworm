use anyhow::{anyhow, Result};
use comfy_table::{presets::UTF8_FULL, Table};
use soundworm_ipc::{
    client::{connect_subscriber, Client},
    default_socket_path, EventFilter, MetricsPayload, Op, ResponseData,
};

pub async fn run(args: &[String]) -> Result<()> {
    let watch = args.iter().any(|a| a == "--watch");
    let json = args.iter().any(|a| a == "--json");

    if watch {
        return watch_xruns().await;
    }

    let mut c = Client::connect(&default_socket_path()).await?;
    let metrics = match c.request(Op::GetMetrics).await? {
        ResponseData::Metrics { metrics } => metrics,
        _ => return Err(anyhow!("unexpected response from daemon")),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&metrics)?);
        return Ok(());
    }

    print_table(&metrics);
    Ok(())
}

fn print_table(m: &MetricsPayload) {
    println!("xruns:  total={}", m.xrun_total);
    if !m.xrun_by_node.is_empty() {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(["Node ID", "Xruns"]);
        let mut rows: Vec<_> = m.xrun_by_node.iter().collect();
        rows.sort_by_key(|(id, _)| id.0);
        for (id, c) in rows {
            table.add_row([id.0.to_string(), c.to_string()]);
        }
        println!("{table}");
    }

    if m.nodes.is_empty() {
        println!("latency: (no samples yet)");
        return;
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(["Node ID", "count", "min", "p50", "p95", "p99", "max"]);
    for n in &m.nodes {
        table.add_row([
            n.node_id.0.to_string(),
            n.count.to_string(),
            fmt_ms(n.min_ms),
            fmt_ms(n.p50_ms),
            fmt_ms(n.p95_ms),
            fmt_ms(n.p99_ms),
            fmt_ms(n.max_ms),
        ]);
    }
    println!("latency (ms):");
    println!("{table}");
}

fn fmt_ms(v: f32) -> String { format!("{v:.2}") }

async fn watch_xruns() -> Result<()> {
    let mut events = connect_subscriber(
        &default_socket_path(),
        Some(EventFilter { kinds: Some(vec!["XrunObserved".into(), "EventsDropped".into()]) }),
    )
    .await?;
    eprintln!("Streaming xruns (ctrl-c to quit)…");
    while let Some(ev) = events.recv().await {
        use soundworm_ipc::Event;
        match ev {
            Event::XrunObserved { node_id, gap_ms } => {
                println!("xrun node={} gap={:.2}ms", node_id.0, gap_ms);
            }
            Event::EventsDropped { count } => eprintln!("⚠ dropped {count} events"),
            _ => {}
        }
    }
    Ok(())
}

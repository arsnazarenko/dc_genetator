use clap::{Parser, ValueEnum};
use samsa::prelude::TcpConnection;
use std::{time::Duration, vec};
use tokio::time;

mod dc_metrics;

#[derive(Parser)]
#[command(name = "dc-generator")]
#[command(about = "Real-time data center metrics traffic generator")]
struct Args {
    /// Output mode
    #[arg(short, long)]
    mode: Mode,

    /// Kafka topic name (required when mode is kafka)
    #[arg(long, required_if_eq("mode", "kafka"))]
    topic: Option<String>,

    /// Kafka address (required when mode is kafka)
    #[arg(long, required_if_eq("mode", "kafka"))]
    address: Option<String>,

    /// Timeout between messages in milliseconds
    #[arg(short, long, default_value_t = 500)]
    timeout: u64,

    /// Number of zones in data center
    #[arg(long, default_value_t = 4)]
    zones: usize,

    /// Number of servers per zone
    #[arg(long, default_value_t = 100)]
    servers_per_zone: usize,
}

#[derive(Clone, Debug, ValueEnum)]
enum Mode {
    Stdout,
    Kafka,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.mode {
        Mode::Stdout => {
            stdout_mode(args.timeout, args.zones, args.servers_per_zone);
        }
        Mode::Kafka => {
            let topic = args.topic.unwrap();
            let address = args.address.unwrap();
            kafka_mode(
                args.timeout,
                &topic,
                &address,
                args.zones,
                args.servers_per_zone,
            )
            .await
            .map_err(|e| std::io::Error::other(format!("Kafka client error: {}", e.to_string())))?;
        }
    }

    Ok(())
}

fn stdout_mode(timeout: u64, zones: usize, servers_per_zone: usize) {
    let gen_iterator = (0..zones)
        .into_iter()
        .map(|zone_num| {
            let zone_name = format!("zone-{}", (b'A' + zone_num as u8) as char);
            dc_metrics::ServerMetricsGenerator::new(zone_name, servers_per_zone)
        })
        .cycle();

    for mut zone_gen in gen_iterator {
        let metric = zone_gen.next().unwrap();
        println!("{}", metric.message);
        std::thread::sleep(Duration::from_millis(timeout));
    }
}

async fn kafka_mode(
    timeout: u64,
    topic: &str,
    address: &str,
    zones: usize,
    servers_per_zone: usize,
) -> samsa::prelude::Result<()> {
    let (host, port_str) = address
        .split_once(':')
        .expect("Address of kafka must be in format {host:port}");

    let host: String = host.to_string();
    let port: u16 = port_str.parse().unwrap();
    let bootstrap_addrs = vec![samsa::prelude::BrokerAddress {
        host: host.into(),
        port,
    }];
    let producer = samsa::prelude::ProducerBuilder::<TcpConnection>::new(
        bootstrap_addrs,
        vec![topic.to_string()],
    )
    .await?
    .build()
    .await;

    println!("Producer connected to kafka on address {}", address);

    let shared_producer = std::sync::Arc::new(producer);
    let shared_topic = std::sync::Arc::new(topic.to_string());

    let mut handles = Vec::with_capacity(zones);

    for zone_num in 0..zones {
        let zone_name = format!("zone-{}", (b'A' + zone_num as u8) as char);
        let producer = shared_producer.clone();
        let topic_cloned = shared_topic.clone();

        let handle = tokio::spawn(async move {
            let mut metrics_gen =
                dc_metrics::ServerMetricsGenerator::new(zone_name, servers_per_zone);
            let mut interval = time::interval(Duration::from_millis(timeout));
            loop {
                interval.tick().await;
                let metric = metrics_gen.next().unwrap();
                let message = samsa::prelude::ProduceMessage {
                    topic: topic_cloned.to_string(),
                    partition_id: 0,
                    key: Some(metric.host_id.into()),
                    value: Some(metric.message.into()),
                    headers: vec![],
                };
                producer.produce(message).await;
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        let _ = handle.await;
    }
    Ok(())
}

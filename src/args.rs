use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dc-generator")]
#[command(about = "Real-time traffic generator for Kafka")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
pub struct GenParams {
    /// Timeout between messages in milliseconds
    #[arg(short, long, default_value_t = 500)]
    pub timeout: u64,

    /// Number of zones in data center
    #[arg(short, long, default_value_t = 4)]
    pub zones: u8,

    /// Number of servers per zone
    #[arg(short, long, default_value_t = 80)]
    pub servers_per_zone: u16,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Output messages to stdout
    Stdout {
        // parameters for generator
        #[clap(flatten)]
        dc_gen_params: GenParams,
    },
    /// Send messages to Kafka
    Kafka {
        // Kafka address (required when mode is kafka)
        #[arg(long, value_parser = parse_kafka_brokers)]
        brokers: KafkaBrokers,

        // Kafka topic
        #[arg(long, default_value = "dc_metrics")]
        topic: String,

        // Number of topic partitions
        #[arg(short, long, default_value_t = 3)]
        partitions: u16,

        // Number of topic replicas
        #[arg(short, long, default_value_t = 3)]
        replicas: u8,

        // parameters for generator
        #[clap(flatten)]
        dc_gen_params: GenParams,
    },
}

pub fn parse() -> CliArgs {
    CliArgs::parse()
}

type KafkaBrokers = Vec<samsa::prelude::BrokerAddress>;

fn parse_kafka_brokers(address_str: &str) -> Result<KafkaBrokers, clap::Error> {
    let brokers = address_str
        .trim()
        .split(",")
        .map(|s| {
            let (host, port) = s.trim().split_once(":").ok_or_else(|| {
                clap::Error::raw(
                    clap::error::ErrorKind::InvalidValue,
                    "Kafka broker address must be in format: <HOST:PORT>",
                )
            })?;
            let port: u16 = port.parse().map_err(|_| {
                clap::Error::raw(clap::error::ErrorKind::InvalidValue, "Invalid port number")
            })?;
            let host = host.into();
            Ok(samsa::prelude::BrokerAddress { host, port })
        })
        .collect::<Result<Vec<_>, clap::Error>>()?;

    if brokers.is_empty() {
        Err(clap::Error::raw(
            clap::error::ErrorKind::InvalidValue,
            "Kafka brokers list must be in format: <HOST1:PORT,HOST2:PORT,...>",
        ))
    } else {
        Ok(brokers)
    }
}

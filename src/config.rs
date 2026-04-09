use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AppConfig {
    pub redis: Option<String>,
    pub mongodb: Option<String>,
    pub kafka: Option<String>,
    pub rabbitmq: Option<String>,
}

/// Vigil: Terminal-based observability tool for monitoring distributed systems locally.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// Redis connection URL or port
    #[arg(long)]
    pub redis: Option<String>,

    /// MongoDB connection URL
    #[arg(long)]
    pub mongodb: Option<String>,

    /// Kafka brokers (comma separated)
    #[arg(long)]
    pub kafka: Option<String>,

    /// RabbitMQ connection URL
    #[arg(long)]
    pub rabbitmq: Option<String>,

    /// Set a trace ID to filter logs or Gantt view
    #[arg(long)]
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub redis: String,
    pub mongodb: String,
    pub kafka: String,
    pub rabbitmq: String,
    pub trace_id: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let cli = CliArgs::parse();

        // Try reading from config.toml in current directory
        let mut toml_cfg = AppConfig::default();
        if let Ok(toml_str) = fs::read_to_string("config.toml") {
            if let Ok(parsed) = toml::from_str::<AppConfig>(&toml_str) {
                toml_cfg = parsed;
            }
        }

        Self {
            redis: cli.redis.or(toml_cfg.redis).unwrap_or_else(|| "redis://127.0.0.1:6379".to_string()),
            mongodb: cli.mongodb.or(toml_cfg.mongodb).unwrap_or_else(|| "mongodb://127.0.0.1:27017".to_string()),
            kafka: cli.kafka.or(toml_cfg.kafka).unwrap_or_else(|| "127.0.0.1:9092".to_string()),
            rabbitmq: cli.rabbitmq.or(toml_cfg.rabbitmq).unwrap_or_else(|| "amqp://127.0.0.1:5672/%2f".to_string()),
            trace_id: cli.trace_id,
        }
    }
}

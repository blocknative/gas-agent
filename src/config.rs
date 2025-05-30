use clap::{Parser, Subcommand};
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;

use crate::types::{AgentKind, Network, System};

#[derive(Parser)]
#[command(name = "Gas Agent")]
#[command(about = "Deploy agents that generate and submit gas price predictions.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the agent(s) to generate gas price predictions
    Start(Config),
    /// Generate and print a new random key pair to be used as an agent's signer key
    GenerateKeys,
}

#[derive(Parser, Clone, Debug)]
pub struct Config {
    #[arg(long, env = "SERVER_ADDRESS", default_value = "0.0.0.0:8080")]
    pub server_address: SocketAddr,

    /// A list of chain configurations to run (JSON format)
    #[arg(long, env = "CHAINS")]
    pub chains: String,

    #[arg(
        long,
        env = "COLLECTOR_ENDPOINT",
        default_value = "https://collector.gas.network"
    )]
    pub collector_endpoint: Url,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub system: System,
    pub network: Network,
    pub json_rpc_url: String,
    pub pending_block_data_source: Option<PendingBlockDataSource>,
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub kind: AgentKind,
    pub signer_key: String,
    pub prediction_trigger: PredictionTrigger,
}

#[derive(Debug, Clone, Deserialize)]
pub enum PendingBlockDataSource {
    JsonRpc {
        url: String,
        method: String,
        params: Option<Value>,
        poll_rate_ms: u64,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PredictionTrigger {
    Block,
    Poll { rate_ms: u64 },
}

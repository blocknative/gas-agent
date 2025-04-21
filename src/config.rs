use crate::types::{AgentKind, Env};
use clap::Parser;
use reqwest::Url;
use std::net::SocketAddr;

#[derive(Parser, Clone, Debug)]
pub struct Config {
    #[arg(long, env = "SERVER_ADDRESS", default_value = "0.0.0.0:8080")]
    pub server_address: SocketAddr,

    #[arg(long, env = "CHAIN_ID")]
    pub chain_id: u64,

    #[arg(long, env = "MODE")]
    pub mode: AgentKind,

    #[arg(long, env = "ENV", default_value = "local")]
    pub env: Env,

    #[arg(long, env = "COLLECTOR_ENDPOINT")]
    pub collector_endpoint: Url,

    #[arg(long, env = "SIGNER_KEY")]
    pub signer_key: String,
}

use crate::types::AgentKind;
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

    // The endpoint to publish data to, if not set, will log to stdout
    #[arg(long, env = "PUBLISH_ENDPOINT")]
    pub publish_endpoint: Option<Url>,

    // The SECP256k1 private key to sign data with, if not set, will generate a new random key and write to disk for future use
    #[arg(long, env = "SIGNER_KEY")]
    pub signer_key: Option<String>,
}

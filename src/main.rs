use agent::start_agent;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use config::Config;
use interrupts::{on_panic, on_sigterm};
use logs::init_logs;
use server::start_server_without_state;
use std::sync::Arc;
use tokio::spawn;
use tracing::{error, info};
use utils::{get_or_create_signer_key, load_chain_list};

mod agent;
mod blocks;
mod chain;
mod config;
mod constants;
mod distribution;
mod interrupts;
mod logs;
mod models;
mod publish;
mod rpc;
mod server;
mod types;
mod utils;

#[ntex::main]
async fn main() -> Result<()> {
    // Initialize tracing logger.
    init_logs();

    // Parse the configuration.
    let mut config = Config::parse();

    if config.signer_key.is_none() {
        config.signer_key = Some(get_or_create_signer_key());
    }

    info!("Loading RPC for chain_id: {}", config.chain_id);
    let chains = load_chain_list().await.context("Loading chain list")?;

    let configured_chain = chains
        .into_iter()
        .find(|chain| chain.chain_id == config.chain_id)
        .ok_or(anyhow!(
            "Chain ID: {} does not exit on chain list",
            config.chain_id
        ))?;

    let server_address = config.server_address.clone();

    // log panics
    on_panic(|panic_info| error!(error = %panic_info, "Panic detected!!"));

    info!("Loaded {} from chain list", configured_chain.name);
    let handle = Arc::new(spawn(async move {
        if let Err(e) = start_agent(configured_chain, &config).await {
            error!(
                "Failed to start agent for chain_id: {}, error: {}",
                &config.chain_id,
                e.to_string()
            );
        }
    }));

    // Create handlers for both SIGTERM and SIGINT
    let handle_for_shutdown = handle.clone();
    let shutdown_handler = on_sigterm(move || {
        let handle = handle_for_shutdown.clone();
        async move {
            handle.abort();
        }
    });

    info!("Starting server at {}", &server_address);
    let _ = start_server_without_state(&server_address, None).await;
    let _ = shutdown_handler.await;

    Ok(())
}

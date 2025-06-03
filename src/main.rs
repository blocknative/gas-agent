use agent::start_agents;
use anyhow::anyhow;
use anyhow::{Context, Result};
use clap::Parser;
use config::{ChainConfig, Cli, Commands};
use dotenv::dotenv;
use interrupts::{on_panic, on_sigterm};
use logs::init_logs;
use server::start_server_without_state;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tracing::{error, info};
use utils::generate_key_pair;

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
    dotenv().ok();
    init_logs();

    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateKeys => generate_key_pair(),
        Commands::Start(config) => {
            let chain_configs: Vec<ChainConfig> =
                serde_json::from_str(&config.chains).context("Loading Chain Configurations")?;

            if chain_configs.is_empty() {
                return Err(anyhow!("No chains configured"));
            }

            let server_address = config.server_address;

            // log panics
            on_panic(|panic_info| error!(error = %panic_info, "Panic detected!!"));

            let agents_handles = Arc::new(Mutex::new(JoinSet::new()));
            let agents_handles_clone = agents_handles.clone();

            for chain_config in chain_configs {
                let config_clone = config.clone();

                agents_handles_clone.lock().await.spawn(async move {
                    let system = chain_config.system.clone();
                    let network = chain_config.network.clone();

                    if let Err(e) = start_agents(chain_config, &config_clone).await {
                        error!(
                            "Failed to start agent for system: {}, network: {}, error: {}",
                            &system,
                            &network,
                            e.to_string()
                        );
                    }
                });
            }

            // Create handlers for both SIGTERM and SIGINT
            let shutdown_handler = on_sigterm(move || {
                let agents_for_shutdown = agents_handles.clone();

                async move {
                    agents_for_shutdown.lock().await.abort_all();
                }
            });

            info!("Starting server at {}", &server_address);
            let _ = start_server_without_state(&server_address, None).await;
            let _ = shutdown_handler.await;

            Ok(())
        }
    }
}

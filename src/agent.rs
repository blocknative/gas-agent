use crate::blocks::{block_to_block_distribution, block_to_chain_tip, ChainTip};
use crate::chain::Chain;
use crate::config::Config;
use crate::distribution::BlockDistribution;
use crate::models::apply_model;
use crate::publish::publish_agent_payload;
use crate::rpc::{get_latest_block, get_rpc_client, Block, RpcClient};
use crate::types::{AgentKind, AgentPayload, AgentPayloadKind, FeeUnit, Settlement};
use crate::utils::extract_system_network;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Url;
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{error, info};

const MAX_NUM_BLOCK_DISTRIBUTIONS: usize = 50;

pub async fn start_agent(chain: Chain, config: &Config) -> Result<()> {
    let signer: PrivateKeySigner = config
        .signer_key
        .as_ref()
        .expect("Signer key is required")
        .parse()?;

    let address = signer.address();

    let mut agent = GasAgent::new(chain, config).await?;

    info!(
        "Starting agent type: {}, Signer address: {}",
        config.mode.to_string().to_uppercase(),
        address.to_string()
    );

    agent.start().await;

    Ok(())
}

struct GasAgent {
    config: Config,
    rpc_client: RpcClient,
    chain: Chain,
    chain_tip: ChainTip,
    estimated_block_time_ms: i64,
    block_distributions: Vec<BlockDistribution>,
}

impl GasAgent {
    pub async fn new(chain: Chain, config: &Config) -> Result<Self> {
        let (rpc_client, latest_block) = init_rpc_client(&chain.rpc).await?;
        let chain_tip = block_to_chain_tip(&latest_block);
        let distribution = block_to_block_distribution(&latest_block);

        Ok(Self {
            config: config.clone(),
            rpc_client,
            chain,
            chain_tip,
            estimated_block_time_ms: 2000,
            block_distributions: vec![distribution],
        })
    }

    async fn handle_new_block(&mut self, block: Block) -> Result<()> {
        let new_chain_tip = block_to_chain_tip(&block);
        let block_gap = new_chain_tip.height - self.chain_tip.height;

        // update estimated block time
        let duration = (new_chain_tip.timestamp - self.chain_tip.timestamp).num_milliseconds();
        self.estimated_block_time_ms = duration / block_gap as i64;

        let new_distribution = block_to_block_distribution(&block);
        let actual_min = new_distribution.get(0).map(|dist| dist.gwei).unwrap_or(0.0);

        // Update chain tip
        self.chain_tip = new_chain_tip;

        let (system, network) = extract_system_network(&self.chain.name);
        let mut publish_payload: Option<AgentPayload> = None;

        match &self.config.mode {
            AgentKind::Model(model) => {
                // Add distribution
                self.block_distributions.push(new_distribution);

                let distributions_len = self.block_distributions.len();

                // Drop older distributions if reach max len
                if distributions_len > MAX_NUM_BLOCK_DISTRIBUTIONS {
                    let start_idx = distributions_len.saturating_sub(MAX_NUM_BLOCK_DISTRIBUTIONS);
                    self.block_distributions.drain(0..start_idx);
                }

                // Run payload
                let price = apply_model(&model, &self.block_distributions).await?;

                publish_payload = Some(AgentPayload {
                    from_block: self.chain_tip.height + 1,
                    settlement: Settlement::Fast,
                    timestamp: Utc::now(),
                    unit: FeeUnit::Gwei,
                    system,
                    network,
                    price,
                    kind: AgentPayloadKind::Estimate,
                });
            }
            AgentKind::Node => {
                let node_price = self
                    .rpc_client
                    .get_node_gas_price_estimate()
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Failed to get node gas price payload");
                        e
                    })
                    .ok();

                if let Some(node_price) = node_price {
                    publish_payload = Some(AgentPayload {
                        from_block: self.chain_tip.height + 1,
                        settlement: Settlement::Fast,
                        timestamp: Utc::now(),
                        unit: FeeUnit::Gwei,
                        system,
                        network,
                        price: node_price,
                        kind: AgentPayloadKind::Estimate,
                    });
                }
            }
            AgentKind::Target => {
                publish_payload = Some(AgentPayload {
                    from_block: self.chain_tip.height,
                    settlement: Settlement::Immediate,
                    timestamp: Utc::now(),
                    unit: FeeUnit::Gwei,
                    system,
                    network,
                    price: actual_min,
                    kind: AgentPayloadKind::Target,
                });
            }
        }

        if let Some(payload) = publish_payload {
            if let Some(publish_endpoint) = &self.config.publish_endpoint {
                publish_agent_payload(
                    publish_endpoint.as_str(),
                    &self.config.signer_key.as_ref().unwrap(),
                    &payload,
                )
                .await?;
            } else {
                info!("Agent payload: {:?}", payload);
            }
        }

        Ok(())
    }

    pub async fn start(&mut self) {
        loop {
            // wait estimated block time plus client_latency
            let wait = Duration::from_millis(self.estimated_block_time_ms as u64 + 200);

            tokio::time::sleep(wait).await;

            match get_latest_block(&self.rpc_client).await {
                Ok(block) => {
                    if block.number > self.chain_tip.height {
                        if let Err(e) = self.handle_new_block(block).await {
                            error!(error = %e, "Failed to handle new block");
                        }
                    } else {
                        // No new block updated yet, wait 250ms until our block time becomes more accurate
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to get latest block");
                }
            }
        }
    }
}

/// Takes a list of rpc hosts and will loop through,
/// find the first RPC that succesfully returns the chain tip
pub async fn init_rpc_client(rpc_hosts: &[String]) -> Result<(RpcClient, Block)> {
    // Filter out WS and RPC's that require an API key
    let mut rpc_hosts: VecDeque<&String> = rpc_hosts
        .iter()
        .filter(|rpc| !rpc.starts_with("ws") && !rpc.contains("${"))
        .collect();

    while !rpc_hosts.is_empty() {
        let rpc = rpc_hosts.pop_front();

        if let Some(rpc) = rpc {
            let rpc_url = Url::parse(&rpc).ok();
            info!("Testing RPC: {} by getting node latest block", &rpc);

            if let Some(rpc_url) = rpc_url {
                let client = get_rpc_client(rpc_url);

                match get_latest_block(&client).await {
                    Ok(block) => {
                        info!(
                            "Successfully connected to RPC: {} and fetched latest block: {}",
                            &rpc, &block.number
                        );

                        return std::result::Result::Ok((client, block));
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            "Failed to get latest block from rpc: {}",
                            &rpc,
                        );
                    }
                }
            } else {
                error!("RPC is an invalid URL: {}", &rpc);
            }
        }
    }

    Err(anyhow!(
        "Tried all available RPC's and could not successfully get the latest block"
    ))
}

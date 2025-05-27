use crate::blocks::{block_to_block_distribution, calc_base_fee};
use crate::config::{AgentConfig, ChainConfig, Config, PendingBlockDataSource, PredictionTrigger};
use crate::distribution::BlockDistribution;
use crate::models::apply_model;
use crate::publish::publish_agent_payload;
use crate::rpc::{get_latest_block, get_rpc_client, Block, BlockHeader, RpcClient};
use crate::types::{
    AgentKind, AgentPayload, AgentPayloadKind, FeeUnit, Settlement, SystemNetworkKey,
};
use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Url;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};

const MAX_NUM_BLOCK_DISTRIBUTIONS: usize = 50;

pub async fn start_agents(chain_config: ChainConfig, config: &Config) -> Result<()> {
    let agents = GasAgent::new(chain_config, config).await?;
    agents.run().await
}

#[derive(Clone)]
struct GasAgent {
    chain_config: ChainConfig,
    config: Config,
    rpc_client: RpcClient,
    chain_tip: Arc<RwLock<BlockHeader>>,
    estimated_block_time_ms: Arc<RwLock<i64>>,
    block_distributions: Arc<RwLock<Vec<BlockDistribution>>>,
    pending_block_distribution: Arc<RwLock<Option<BlockDistribution>>>,
}

impl GasAgent {
    pub async fn new(chain_config: ChainConfig, config: &Config) -> Result<Self> {
        let (rpc_client, rpc_chain_id, latest_block) =
            init_rpc_client(&chain_config.json_rpc_url).await?;

        let distribution =
            block_to_block_distribution(&latest_block.transactions, &latest_block.base_fee_per_gas);

        let system_network =
            SystemNetworkKey::new(chain_config.system.clone(), chain_config.network.clone());

        if system_network.to_chain_id() != rpc_chain_id {
            panic!(
                "Configured chain: {} {} does not match RPC chain_id: {}",
                &chain_config.system, &chain_config.network, rpc_chain_id
            );
        }

        Ok(Self {
            chain_config: chain_config.clone(),
            config: config.clone(),
            rpc_client,
            chain_tip: Arc::new(RwLock::new(latest_block.into())),
            estimated_block_time_ms: Arc::new(RwLock::new(2000)),
            block_distributions: Arc::new(RwLock::new(vec![distribution])),
            pending_block_distribution: Arc::new(RwLock::new(None)),
        })
    }

    async fn create_prediction(&self, agent: &AgentConfig) -> Result<()> {
        let block_distributions = {
            let guard = self.block_distributions.read().await;
            guard.clone()
        };

        let last_distribution = block_distributions.last();

        let actual_min = last_distribution
            .map(|dist| dist.get(0).map(|dist| dist.gwei))
            .flatten()
            .unwrap_or(0.0);

        match &agent.kind {
            AgentKind::Model(model) => {
                let pending_block_distribution = {
                    let guard = self.pending_block_distribution.read().await;
                    guard.clone()
                };

                let (price, settlement) =
                    apply_model(&model, &block_distributions, pending_block_distribution).await?;

                let chain_tip = self.chain_tip.read().await.clone();
                let payload = AgentPayload {
                    from_block: chain_tip.number + 1,
                    settlement,
                    timestamp: Utc::now(),
                    unit: FeeUnit::Gwei,
                    system: self.chain_config.system.clone(),
                    network: self.chain_config.network.clone(),
                    price,
                    kind: AgentPayloadKind::Estimate,
                };

                publish_agent_payload(
                    self.config.collector_endpoint.as_str(),
                    &agent.signer_key,
                    &payload,
                )
                .await?;
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
                    let chain_tip = self.chain_tip.read().await.clone();
                    let payload = AgentPayload {
                        from_block: chain_tip.number + 1,
                        settlement: Settlement::Fast,
                        timestamp: Utc::now(),
                        unit: FeeUnit::Gwei,
                        system: self.chain_config.system.clone(),
                        network: self.chain_config.network.clone(),
                        price: node_price,
                        kind: AgentPayloadKind::Estimate,
                    };

                    publish_agent_payload(
                        self.config.collector_endpoint.as_str(),
                        &agent.signer_key,
                        &payload,
                    )
                    .await?;
                }
            }
            AgentKind::Target => {
                let chain_tip = self.chain_tip.read().await.clone();
                let payload = AgentPayload {
                    from_block: chain_tip.number,
                    settlement: Settlement::Immediate,
                    timestamp: Utc::now(),
                    unit: FeeUnit::Gwei,
                    system: self.chain_config.system.clone(),
                    network: self.chain_config.network.clone(),
                    price: actual_min,
                    kind: AgentPayloadKind::Target,
                };

                publish_agent_payload(
                    self.config.collector_endpoint.as_str(),
                    &agent.signer_key,
                    &payload,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_new_block(&self, block: Block) -> Result<()> {
        let new_chain_tip = BlockHeader::from(block.clone());
        let current_chain_tip = { self.chain_tip.read().await.clone() };
        let block_gap = new_chain_tip.number - current_chain_tip.number;

        // update estimated block time
        let duration = (new_chain_tip.timestamp - current_chain_tip.timestamp).num_milliseconds();
        *self.estimated_block_time_ms.write().await = duration / block_gap as i64;

        let new_distribution =
            block_to_block_distribution(&block.transactions, &block.base_fee_per_gas);

        // Update chain tip
        *self.chain_tip.write().await = new_chain_tip.clone();

        // Update block distributions
        {
            let mut distributions = self.block_distributions.write().await;
            distributions.push(new_distribution);

            let distributions_len = distributions.len();

            // Drop older distributions if reach max len
            if distributions_len > MAX_NUM_BLOCK_DISTRIBUTIONS {
                let start_idx = distributions_len.saturating_sub(MAX_NUM_BLOCK_DISTRIBUTIONS);
                distributions.drain(0..start_idx);
            }
        }

        for agent in self.chain_config.agents.iter() {
            if matches!(&agent.prediction_trigger, &PredictionTrigger::Block) {
                self.create_prediction(agent).await?;
            }
        }

        Ok(())
    }

    pub async fn poll_blocks(&self) {
        loop {
            // wait estimated block time
            let estimated_block_time_ms = {
                let guard = self.estimated_block_time_ms.read().await;
                guard.clone()
            };

            let wait = Duration::from_millis(estimated_block_time_ms as u64);

            tokio::time::sleep(wait).await;

            let mut get_new_block = true;

            while get_new_block {
                match get_latest_block(&self.rpc_client).await {
                    Ok(block) => {
                        let current_height = { self.chain_tip.read().await.number };
                        if block.number > current_height {
                            if let Err(e) = self.handle_new_block(block).await {
                                error!(error = %e, "Failed to handle new block");
                            }

                            get_new_block = false;
                        } else {
                            // No new block updated yet, wait 1 sec until our block time becomes more accurate
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to get latest block");
                    }
                }
            }
        }
    }

    async fn poll_pending_block(self) {
        if let Some(pending_block_source) = self.chain_config.pending_block_data_source {
            match pending_block_source {
                PendingBlockDataSource::JsonRpc {
                    url,
                    method,
                    params,
                    poll_rate_ms,
                } => {
                    info!("Polling pending block from JSON-RPC: url: {}, method: {}, params: {:?}, polling rate: {}ms", url, method, params, poll_rate_ms);

                    let rpc_url = Url::parse(&url)
                        .context("Invalid block JSON rpc url")
                        .expect("Valid JSON RPC url for pending block");

                    let client = get_rpc_client(rpc_url);

                    loop {
                        match client.get_pending_block(&method, params.clone()).await {
                            Ok(transactions) => {
                                let chain_tip = { self.chain_tip.read().await.clone() };
                                let next_base_fee = calc_base_fee(&chain_tip);
                                let distribution =
                                    block_to_block_distribution(&transactions, &next_base_fee);

                                {
                                    let mut pending_block_distribution =
                                        self.pending_block_distribution.write().await;

                                    *pending_block_distribution = Some(distribution);
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to get pending block");
                            }
                        }

                        // Sleep for poll rate duration
                        tokio::time::sleep(Duration::from_millis(poll_rate_ms)).await;
                    }
                }
            }
        }
    }

    async fn poll_predictions(&self, agent: &AgentConfig, rate_ms: u64) {
        loop {
            tokio::time::sleep(Duration::from_millis(rate_ms)).await;
            if let Err(e) = self.create_prediction(agent).await {
                error!("Failed to create prediction: {}", e);
            }
        }
    }

    pub async fn run(&self) -> Result<()> {
        let pending_block_poll_agent_clone = self.clone();

        tokio::spawn(async move {
            pending_block_poll_agent_clone.poll_pending_block().await;
        });

        let block_poll_agent_clone = self.clone();

        tokio::spawn(async move {
            block_poll_agent_clone.poll_blocks().await;
        });

        for agent in self.chain_config.agents.iter() {
            if let PredictionTrigger::Poll { rate_ms } = agent.prediction_trigger {
                let trigger_poll_agent_clone = self.clone();
                let agent_clone = agent.clone();

                tokio::spawn(async move {
                    trigger_poll_agent_clone
                        .poll_predictions(&agent_clone, rate_ms)
                        .await;
                });
            }
        }

        Ok(())
    }
}

/// Takes a block source and a list of rpc hosts.
/// Will prefer the block source if available, otherwise will use the rpc hosts.
/// If using rpc_hosts, will loop through and find the first RPC that succesfully returns the chain tip
pub async fn init_rpc_client(url: &str) -> Result<(RpcClient, u64, Block)> {
    let rpc_url = Url::parse(&url).context("Invalid block JSON rpc url")?;
    let client = get_rpc_client(rpc_url);

    let chain_id = client.get_chain_id().await?;

    let block = get_latest_block(&client)
        .await
        .context("Failed to get latest block from block source rpc")?;

    Ok((client, chain_id, block))
}

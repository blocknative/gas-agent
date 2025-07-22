use crate::blocks::{block_to_block_distribution, calc_base_fee};
use crate::config::{AgentConfig, ChainConfig, Config, PendingBlockDataSource, PredictionTrigger};
use crate::distribution::BlockDistribution;
use crate::models::{apply_model, ModelError};
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
use tracing::{debug, error, info, warn};

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
    block_distributions: Arc<RwLock<Vec<BlockDistribution>>>,
    pending_block_distribution: Arc<RwLock<Option<BlockDistribution>>>,
    client: reqwest::Client,
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
            block_distributions: Arc::new(RwLock::new(vec![distribution])),
            pending_block_distribution: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
        })
    }

    async fn create_prediction(&self, agent: &AgentConfig) -> Result<()> {
        let block_distributions = {
            let guard = self.block_distributions.read().await;
            guard.clone()
        };

        let last_distribution = block_distributions.last();

        let actual_min = last_distribution
            .and_then(|dist| dist.first().map(|dist| dist.gwei))
            .unwrap_or(0.0);

        let latest_block = { self.chain_tip.read().await.number };

        match &agent.kind {
            AgentKind::Model(model) => {
                let pending_block_distribution = {
                    let guard = self.pending_block_distribution.read().await;
                    guard.clone()
                };

                let (price, settlement, from_block) = match apply_model(
                    model,
                    &block_distributions,
                    pending_block_distribution,
                    latest_block,
                )
                .await
                {
                    Ok(result) => result,
                    Err(ModelError::InsufficientData { message }) => {
                        debug!("Insufficient data for model prediction: {}", message);
                        return Ok(());
                    }
                    Err(e) => return Err(e.into()),
                };

                let payload = AgentPayload {
                    from_block,
                    settlement,
                    timestamp: Utc::now(),
                    unit: FeeUnit::Gwei,
                    system: self.chain_config.system.clone(),
                    network: self.chain_config.network.clone(),
                    price,
                    kind: AgentPayloadKind::Estimate,
                };

                publish_agent_payload(
                    &self.client,
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
                        &self.client,
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
                    &self.client,
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

        let new_distribution =
            block_to_block_distribution(&block.transactions, &block.base_fee_per_gas);

        // Update chain tip
        {
            *self.chain_tip.write().await = new_chain_tip.clone();
        }

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
                let agent_clone = agent.clone();
                let self_clone = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = self_clone.create_prediction(&agent_clone).await {
                        error!(error = %e, "Failed to create prediction");
                    }
                });
            }
        }

        Ok(())
    }

    pub async fn poll_blocks(&self) {
        // Get block time from the system network configuration
        let system_network = SystemNetworkKey::new(
            self.chain_config.system.clone(),
            self.chain_config.network.clone(),
        );

        let block_time_ms = system_network.to_block_time();

        loop {
            // Calculate wait time based on chain tip timestamp
            let chain_tip_timestamp = {
                let chain_tip = self.chain_tip.read().await;
                chain_tip.timestamp
            };

            let now = chrono::Utc::now();
            let time_since_last_block = (now - chain_tip_timestamp).num_milliseconds();

            // Wait time = block_time - time_since_last_block
            let wait_ms = if time_since_last_block < block_time_ms as i64 {
                block_time_ms as i64 - time_since_last_block
            } else {
                0 // no wait if we're past the expected time
            };

            debug!(
                "Waiting: {wait_ms}ms, Time Since Last Block: {}",
                time_since_last_block
            );

            let wait = Duration::from_millis(wait_ms as u64);
            tokio::time::sleep(wait).await;

            let mut get_new_block = true;

            while get_new_block {
                match get_latest_block(&self.rpc_client).await {
                    Ok(block) => {
                        debug!(
                            "Block for System: {}, Network: {}, Height {}",
                            &self.chain_config.system, &self.chain_config.network, block.number
                        );

                        let current_height = { self.chain_tip.read().await.number };

                        if block.number > current_height {
                            let gap = block.number - current_height;

                            if gap > 1 {
                                warn!(
                                    "Missed blocks for System: {}, Network: {}! Last block height: {}, new block height: {}, GAP: {}",
                                    &self.chain_config.system, &self.chain_config.network,
                                    current_height, block.number, gap
                                );
                            }

                            if let Err(e) = self.handle_new_block(block).await {
                                error!(error = %e, "Failed to handle new block");
                            }

                            get_new_block = false;
                        } else {
                            // No new block updated yet, wait 250ms and try again
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to get latest block");
                    }
                }
            }
        }
    }

    async fn poll_pending_block(&self, pending_block_source: PendingBlockDataSource) {
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

    async fn poll_predictions(&self, agent: &AgentConfig, rate_ms: u64) {
        loop {
            tokio::time::sleep(Duration::from_millis(rate_ms)).await;
            if let Err(e) = self.create_prediction(agent).await {
                error!("Failed to create prediction: {}", e);
            }
        }
    }

    pub async fn run(&self) -> Result<()> {
        if let Some(pending_block_source) = &self.chain_config.pending_block_data_source {
            let pending_block_poll_agent_clone = self.clone();
            let pending_block_source_clone = pending_block_source.clone();

            tokio::spawn(async move {
                pending_block_poll_agent_clone
                    .poll_pending_block(pending_block_source_clone)
                    .await;
            });
        }

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
    let rpc_url = Url::parse(url).context("Invalid block JSON rpc url")?;
    let client = get_rpc_client(rpc_url);

    let chain_id = client.get_chain_id().await?;

    let block = get_latest_block(&client)
        .await
        .context("Failed to get latest block from block source rpc")?;

    Ok((client, chain_id, block))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::Bucket;
    use crate::rpc::Transaction;
    use crate::types::{Network, System};
    use chrono::TimeZone;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn create_test_transaction(
        hash: &str,
        gas_price: Option<u128>,
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
    ) -> Transaction {
        Transaction {
            hash: hash.to_string(),
            gas_price,
            max_fee_per_gas,
            max_priority_fee_per_gas,
        }
    }

    fn create_test_block(
        number: u64,
        timestamp_secs: i64,
        transactions: Vec<Transaction>,
        base_fee_per_gas: Option<u64>,
    ) -> Block {
        Block {
            number,
            timestamp: Utc.timestamp_opt(timestamp_secs, 0).unwrap(),
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas,
            transactions,
        }
    }

    fn create_test_gas_agent() -> GasAgent {
        let chain_config = ChainConfig {
            system: System::Ethereum,
            network: Network::Mainnet,
            json_rpc_url: "http://localhost:8545".to_string(),
            pending_block_data_source: None,
            agents: vec![],
        };

        let config = Config {
            server_address: "0.0.0.0:8080".parse().unwrap(),
            chains: "[]".to_string(),
            collector_endpoint: "http://localhost:3000".parse().unwrap(),
        };

        let rpc_client = RpcClient::new("http://localhost:8545".to_string());

        let initial_block = create_test_block(
            1000,
            1700000000,
            vec![create_test_transaction(
                "0xabc",
                Some(20_000_000_000), // 20 gwei
                None,
                None,
            )],
            Some(10_000_000_000), // 10 gwei base fee
        );

        let initial_distribution = block_to_block_distribution(
            &initial_block.transactions,
            &initial_block.base_fee_per_gas,
        );

        GasAgent {
            chain_config,
            config,
            rpc_client,
            chain_tip: Arc::new(RwLock::new(initial_block.into())),
            block_distributions: Arc::new(RwLock::new(vec![initial_distribution])),
            pending_block_distribution: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
        }
    }

    #[tokio::test]
    async fn test_chain_tip_update() {
        let gas_agent = create_test_gas_agent();

        // Initial chain tip should be block 1000
        {
            let chain_tip = gas_agent.chain_tip.read().await;
            assert_eq!(chain_tip.number, 1000);
            assert_eq!(chain_tip.timestamp.timestamp(), 1700000000);
        }

        // Create a new block
        let new_block = create_test_block(
            1001,
            1700000012, // 12 seconds later
            vec![create_test_transaction(
                "0xdef",
                Some(25_000_000_000), // 25 gwei
                None,
                None,
            )],
            Some(11_000_000_000), // 11 gwei base fee
        );

        // Handle the new block
        gas_agent.handle_new_block(new_block).await.unwrap();

        // Chain tip should be updated
        {
            let chain_tip = gas_agent.chain_tip.read().await;
            assert_eq!(chain_tip.number, 1001);
            assert_eq!(chain_tip.timestamp.timestamp(), 1700000012);
            assert_eq!(chain_tip.base_fee_per_gas, Some(11_000_000_000));
        }
    }

    #[tokio::test]
    async fn test_block_distributions_update() {
        let gas_agent = create_test_gas_agent();

        // Initial distribution should have one entry
        {
            let distributions = gas_agent.block_distributions.read().await;
            assert_eq!(distributions.len(), 1);
        }

        // Add a new block with multiple transactions
        let new_block = create_test_block(
            1001,
            1700000012,
            vec![
                create_test_transaction("0x1", Some(20_000_000_000), None, None), // 20 gwei
                create_test_transaction("0x2", Some(25_000_000_000), None, None), // 25 gwei
                create_test_transaction("0x3", Some(30_000_000_000), None, None), // 30 gwei
                create_test_transaction("0x4", Some(0), None, None), // 0 gwei (should be excluded)
            ],
            Some(10_000_000_000),
        );

        gas_agent.handle_new_block(new_block).await.unwrap();

        // Check distributions updated correctly
        {
            let distributions = gas_agent.block_distributions.read().await;
            assert_eq!(distributions.len(), 2);

            // The new distribution should have 3 entries (excluding 0 gwei transaction)
            let last_dist = distributions.last().unwrap();
            assert!(last_dist.len() >= 3); // At least 3 different price buckets

            // Check that the distribution is sorted ascending
            for i in 1..last_dist.len() {
                assert!(last_dist[i].gwei >= last_dist[i - 1].gwei);
            }

            // Check that 0 gwei is not included
            assert!(last_dist.iter().all(|bucket| bucket.gwei > 0.0));
        }
    }

    #[tokio::test]
    async fn test_block_distributions_max_limit() {
        let gas_agent = create_test_gas_agent();

        // Add blocks until we exceed MAX_NUM_BLOCK_DISTRIBUTIONS
        for i in 1..=55 {
            let block = create_test_block(
                1000 + i,
                1700000000 + (i as i64 * 12),
                vec![create_test_transaction(
                    &format!("0x{:x}", i),
                    Some(20_000_000_000 + (i as u128) * 1_000_000_000),
                    None,
                    None,
                )],
                Some(10_000_000_000),
            );

            gas_agent.handle_new_block(block).await.unwrap();
        }

        // Should only keep the last MAX_NUM_BLOCK_DISTRIBUTIONS (50)
        {
            let distributions = gas_agent.block_distributions.read().await;
            assert_eq!(distributions.len(), MAX_NUM_BLOCK_DISTRIBUTIONS);
        }
    }

    #[tokio::test]
    async fn test_target_agent_payload() {
        let gas_agent = create_test_gas_agent();

        // Create a block with various gas prices
        let new_block = create_test_block(
            1001,
            1700000012,
            vec![
                create_test_transaction("0x1", Some(15_000_000_000), None, None), // 15 gwei (minimum non-zero)
                create_test_transaction("0x2", Some(20_000_000_000), None, None), // 20 gwei
                create_test_transaction("0x3", Some(25_000_000_000), None, None), // 25 gwei
                create_test_transaction("0x4", Some(0), None, None), // 0 gwei (should be excluded)
            ],
            Some(10_000_000_000),
        );

        gas_agent.handle_new_block(new_block).await.unwrap();

        // The Target agent should report the actual minimum (15 gwei) for the current block (1001)
        // We can't directly test the published payload without mocking the publish function,
        // but we can verify the distribution was created correctly
        {
            let distributions = gas_agent.block_distributions.read().await;
            let last_dist = distributions.last().unwrap();

            // The minimum should be 15 gwei (excluding 0 gas price)
            let actual_min = last_dist.first().map(|bucket| bucket.gwei).unwrap_or(0.0);
            assert_eq!(actual_min, 15.0);
        }

        // Verify chain tip is correct for Target payload
        {
            let chain_tip = gas_agent.chain_tip.read().await;
            assert_eq!(chain_tip.number, 1001); // Target reports for current block
        }
    }

    #[tokio::test]
    async fn test_model_agent_payload_from_block() {
        let gas_agent = create_test_gas_agent();

        // Create and handle a new block
        let new_block = create_test_block(
            1001,
            1700000012,
            vec![create_test_transaction(
                "0x1",
                Some(20_000_000_000),
                None,
                None,
            )],
            Some(10_000_000_000),
        );

        gas_agent.handle_new_block(new_block).await.unwrap();

        // Verify chain tip for Model payload
        {
            let chain_tip = gas_agent.chain_tip.read().await;
            assert_eq!(chain_tip.number, 1001);
            // Model agents should report from_block as chain_tip.number + 1 = 1002
        }
    }

    #[tokio::test]
    async fn test_zero_gas_price_exclusion() {
        let gas_agent = create_test_gas_agent();

        // Create a block with only zero gas price transactions
        let block_with_zeros = create_test_block(
            1001,
            1700000012,
            vec![
                create_test_transaction("0x1", Some(0), None, None),
                create_test_transaction("0x2", Some(0), None, None),
            ],
            Some(10_000_000_000),
        );

        gas_agent.handle_new_block(block_with_zeros).await.unwrap();

        // The distribution should be empty (no non-zero prices)
        {
            let distributions = gas_agent.block_distributions.read().await;
            let last_dist = distributions.last().unwrap();
            assert_eq!(last_dist.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_eip1559_transaction_handling() {
        let gas_agent = create_test_gas_agent();

        // Create a block with EIP-1559 transactions
        let new_block = create_test_block(
            1001,
            1700000012,
            vec![
                // Legacy transaction
                create_test_transaction("0x1", Some(25_000_000_000), None, None),
                // EIP-1559 transaction
                create_test_transaction(
                    "0x2",
                    None,
                    Some(30_000_000_000), // max_fee_per_gas
                    Some(2_000_000_000), // max_priority_fee_per_gas (this will be used as it's less than max_fee - base_fee)
                ),
            ],
            Some(10_000_000_000), // base fee
        );

        gas_agent.handle_new_block(new_block).await.unwrap();

        // Check that both transaction types were processed
        {
            let distributions = gas_agent.block_distributions.read().await;
            let last_dist = distributions.last().unwrap();

            // Should have entries for both transactions
            assert!(last_dist.len() >= 2);

            // The EIP-1559 transaction should result in 12 gwei (base + priority fee)
            assert!(last_dist
                .iter()
                .any(|bucket| (bucket.gwei - 12.0).abs() < 0.001));

            // The legacy transaction should result in 25 gwei
            assert!(last_dist
                .iter()
                .any(|bucket| (bucket.gwei - 25.0).abs() < 0.001));
        }
    }

    #[tokio::test]
    async fn test_multiple_block_gap_handling() {
        let gas_agent = create_test_gas_agent();

        // Initial block is 1000, jump to block 1005 (gap of 5)
        let new_block = create_test_block(
            1005,
            1700000060, // 60 seconds later
            vec![create_test_transaction(
                "0x1",
                Some(20_000_000_000),
                None,
                None,
            )],
            Some(10_000_000_000),
        );

        gas_agent.handle_new_block(new_block).await.unwrap();

        // Chain tip should jump to block 1005
        {
            let chain_tip = gas_agent.chain_tip.read().await;
            assert_eq!(chain_tip.number, 1005);
        }
    }

    #[tokio::test]
    async fn test_pending_block_distribution() {
        let gas_agent = create_test_gas_agent();

        // Initially no pending block distribution
        {
            let pending = gas_agent.pending_block_distribution.read().await;
            assert!(pending.is_none());
        }

        // Simulate setting a pending block distribution
        let pending_dist = vec![
            Bucket {
                gwei: 15.0,
                count: 5,
            },
            Bucket {
                gwei: 20.0,
                count: 10,
            },
            Bucket {
                gwei: 25.0,
                count: 3,
            },
        ];

        {
            let mut pending = gas_agent.pending_block_distribution.write().await;
            *pending = Some(pending_dist.clone());
        }

        // Verify it was set
        {
            let pending = gas_agent.pending_block_distribution.read().await;
            assert!(pending.is_some());
            let dist = pending.as_ref().unwrap();
            assert_eq!(dist.len(), 3);
            assert_eq!(dist[0].gwei, 15.0);
            assert_eq!(dist[1].gwei, 20.0);
            assert_eq!(dist[2].gwei, 25.0);
        }
    }
}

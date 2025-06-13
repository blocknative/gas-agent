/*
Percentile-Based Prediction
This approach analyzes the distribution of gas prices across recent blocks and selects a specific percentile (e.g., 75th) to ensure high inclusion probability.

How it works: This algorithm collects all gas prices from recent blocks, sorts them, and finds the price at a specific percentile (75th in this case). This is particularly effective during periods of high volatility, as it targets a price that would have included 75% of recent transactions.
*/

use crate::models::{FromBlock, Prediction};
use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};
use anyhow::{anyhow, Result};

pub fn get_prediction_percentile(
    block_distributions: &[BlockDistribution],
    latest_block: u64,
) -> Result<(Prediction, Settlement, FromBlock)> {
    if block_distributions.is_empty() {
        return Err(anyhow!(
            "Percentile model requires at least one block distribution"
        ));
    }

    let percentile = 0.75; // 75th percentile for high inclusion probability

    // Use 5 most recent blocks
    let num_blocks = 5.min(block_distributions.len());
    let blocks_to_consider = &block_distributions[block_distributions.len() - num_blocks..];

    // Collect all gas prices with their counts
    let mut all_gas_prices: Vec<(f64, u32)> = Vec::new();
    for block in blocks_to_consider {
        for bucket in block {
            all_gas_prices.push((bucket.gwei, bucket.count));
        }
    }

    // Sort by gas price
    all_gas_prices.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Calculate total number of transactions
    let total_txs: u32 = all_gas_prices.iter().map(|(_, count)| *count).sum();

    if total_txs == 0 {
        return Err(anyhow!(
            "Percentile model requires blocks with transactions"
        ));
    }

    // Find the gas price at the given percentile
    let target_count = (total_txs as f64 * percentile) as u32;
    let mut cumulative_count = 0;
    let mut percentile_price = 0.0;

    for (price, count) in all_gas_prices {
        cumulative_count += count;
        if cumulative_count >= target_count {
            percentile_price = price;
            break;
        }
    }

    Ok((round_to_9_places(percentile_price), Settlement::Fast, latest_block + 1))
}

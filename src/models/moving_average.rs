/*
Simple Weighted Moving Average (SWMA)
This approach calculates a weighted average of recent gas prices, giving more weight to more recent blocks.

How it works: This algorithm calculates the average gas price for each block, weighs them by recency, and produces a weighted average. It's simple and works well when gas prices are relatively stable.
*/

use crate::{distribution::BlockDistribution, utils::round_to_9_places};

pub fn get_prediction_swma(block_distributions: &[BlockDistribution]) -> f64 {
    // Use up to 10 most recent blocks
    let num_blocks = 10.min(block_distributions.len());
    let blocks_to_consider = &block_distributions[block_distributions.len() - num_blocks..];

    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for (i, block) in blocks_to_consider.iter().enumerate() {
        let weight = (i + 1) as f64; // Higher weights for more recent blocks

        // Calculate average gas price for this block
        let total_txs: f64 = block.iter().map(|bucket| bucket.count as f64).sum();

        let block_avg_gas_price = if total_txs > 0.0 {
            block
                .iter()
                .map(|bucket| bucket.gwei * bucket.count as f64)
                .sum::<f64>()
                / total_txs
        } else {
            0.0
        };

        weighted_sum += block_avg_gas_price * weight;
        weight_sum += weight;
    }

    let predicted_price = if weight_sum > 0.0 {
        weighted_sum / weight_sum
    } else {
        0.0
    };

    round_to_9_places(predicted_price)
}

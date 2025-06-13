use crate::models::{FromBlock, Prediction};
use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};
use anyhow::{anyhow, Result};
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};

/*
Adaptive Threshold Method
This approach identifies the minimum gas price that would have been included in each recent block and applies an adaptive premium based on price volatility.

How it works: This algorithm finds the minimum gas price included in each recent block, calculates a weighted average (prioritizing recent blocks), and then applies an adaptive premium based on price volatility. When prices are stable, it applies a small premium; when volatile, it applies a larger premium (up to 50%).
*/

pub fn get_prediction_adaptive_threshold(
    block_distributions: &[BlockDistribution],
    latest_block: u64,
) -> Result<(Prediction, Settlement, FromBlock)> {
    // Handle empty input
    if block_distributions.is_empty() {
        return Err(anyhow!(
            "AdaptiveThreshold model requires at least one block distribution"
        ));
    }

    // Use 50 most recent blocks
    let num_blocks = 50.min(block_distributions.len());
    let blocks_to_consider = &block_distributions[block_distributions.len() - num_blocks..];

    // For each block, find the minimum gas price that would have been included
    let mut min_included_prices = Vec::with_capacity(num_blocks);

    for block in blocks_to_consider {
        if block.is_empty() {
            continue;
        }

        let min_price = block
            .iter()
            .min_by(|a, b| {
                a.gwei
                    .partial_cmp(&b.gwei)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|bucket| bucket.gwei)
            .unwrap_or(0.0);

        min_included_prices.push(min_price);
    }

    if min_included_prices.is_empty() {
        return Err(anyhow!(
            "AdaptiveThreshold model requires blocks with transactions"
        ));
    }

    // Calculate weighted average, with higher weights for recent blocks
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for (i, price) in min_included_prices.iter().enumerate() {
        let weight = (i + 1) as f64; // Higher weights for recent blocks
        weighted_sum += price * weight;
        weight_sum += weight;
    }

    let base_price = weighted_sum / weight_sum;

    // Calculate adaptive premium based on price volatility
    let mean = base_price;
    let variance = min_included_prices
        .iter()
        .map(|price| (price - mean).powi(2))
        .sum::<f64>()
        / min_included_prices.len() as f64;

    let std_dev = variance.sqrt();

    // Higher volatility = higher premium (up to 50%)
    let premium_factor = 1.0 + (std_dev / base_price).min(0.5);

    let predicted_price = base_price * premium_factor;
    let predicted_price = Decimal::from_f64(predicted_price)
        .unwrap()
        .round_dp(9)
        .to_f64()
        .unwrap();

    Ok((
        round_to_9_places(predicted_price),
        Settlement::Fast,
        latest_block + 1,
    ))
}

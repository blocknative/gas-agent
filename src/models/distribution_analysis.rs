/*
Gas Price Distribution Analysis
This approach analyzes the cumulative distribution function (CDF) of gas prices to find "sweet spots" where many transactions are being included.

How it works: This algorithm analyzes how gas prices are distributed in the most recent block, constructing a cumulative distribution function. It then identifies the "sweet spot" where the rate of change in the CDF decreases significantly. This is often where many transactions are being included, representing an efficient gas price.
*/

use super::moving_average::get_prediction_swma;
use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};

pub fn get_prediction_distribution(block_distributions: &[BlockDistribution]) -> (f64, Settlement) {
    let latest_block = block_distributions.last().unwrap();

    // Focus on most recent block for distribution analysis
    if latest_block.is_empty() {
        return get_prediction_swma(block_distributions);
    }

    // Sort buckets by gas price
    let mut sorted_buckets = latest_block.clone();
    sorted_buckets.sort_by(|a, b| {
        a.gwei
            .partial_cmp(&b.gwei)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Calculate cumulative distribution function (CDF)
    let total_txs: u32 = sorted_buckets.iter().map(|bucket| bucket.count).sum();
    let mut cdf = Vec::with_capacity(sorted_buckets.len());
    let mut cumulative_count = 0;

    for bucket in &sorted_buckets {
        cumulative_count += bucket.count;
        let cumulative_percent = cumulative_count as f64 / total_txs as f64;
        cdf.push((bucket.gwei, cumulative_percent));
    }

    // Find the "sweet spot" - where the rate of increase in the CDF slows down
    let mut sweet_spot = sorted_buckets[0].gwei;
    let mut max_derivative_change = 0.0;

    // Need at least 3 points to calculate derivatives
    if cdf.len() >= 3 {
        for i in 1..cdf.len() - 1 {
            // Avoid division by zero
            if cdf[i].0 == cdf[i - 1].0 || cdf[i + 1].0 == cdf[i].0 {
                continue;
            }

            let prev_derivative = (cdf[i].1 - cdf[i - 1].1) / (cdf[i].0 - cdf[i - 1].0);
            let next_derivative = (cdf[i + 1].1 - cdf[i].1) / (cdf[i + 1].0 - cdf[i].0);
            let derivative_change = prev_derivative - next_derivative;

            if derivative_change > max_derivative_change {
                max_derivative_change = derivative_change;
                sweet_spot = cdf[i].0;
            }
        }
    } else {
        // Not enough points, use the median
        let mid_index = sorted_buckets.len() / 2;
        sweet_spot = sorted_buckets[mid_index].gwei;
    }

    // Apply a small premium to ensure higher probability of inclusion
    let predicted_price = sweet_spot * 1.1;

    (round_to_9_places(predicted_price), Settlement::Fast)
}

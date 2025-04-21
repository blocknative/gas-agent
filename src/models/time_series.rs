/*
Time-Series Forecasting (Linear Regression)
This approach uses simple linear regression to identify trends in gas prices and predict the next value.

How it works: This algorithm calculates the median gas price for each block, performs linear regression to identify the trend, and extrapolates to predict the next value. It's particularly useful when gas prices show a consistent trend over time (either increasing or decreasing).
*/

use crate::{distribution::BlockDistribution, utils::round_to_9_places};

use super::moving_average::get_prediction_swma;

pub fn get_prediction_time_series(block_distributions: &[BlockDistribution]) -> f64 {
    // Need more blocks for time series analysis
    let num_blocks = 20.min(block_distributions.len());
    if num_blocks < 3 {
        // Not enough data for time series, fall back to SWMA
        return get_prediction_swma(block_distributions);
    }

    let blocks_to_consider = &block_distributions[block_distributions.len() - num_blocks..];

    // Calculate the median gas price for each block
    let mut median_prices: Vec<f64> = Vec::with_capacity(num_blocks);
    for block in blocks_to_consider {
        if block.is_empty() {
            continue;
        }

        // Calculate weighted median
        let mut all_txs: Vec<(f64, u32)> = block
            .iter()
            .map(|bucket| (bucket.gwei, bucket.count))
            .collect();

        all_txs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let total_txs: u32 = all_txs.iter().map(|(_, count)| *count).sum();
        let half_txs = total_txs / 2;

        let mut cumulative_count = 0;
        let mut median_price = 0.0;

        for (price, count) in all_txs {
            cumulative_count += count;
            if cumulative_count >= half_txs {
                median_price = price;
                break;
            }
        }

        median_prices.push(median_price);
    }

    // Simple linear regression
    let n = median_prices.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = median_prices.iter().sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut denominator = 0.0;

    for (i, price) in median_prices.iter().enumerate() {
        let x_diff = i as f64 - x_mean;
        let y_diff = price - y_mean;

        numerator += x_diff * y_diff;
        denominator += x_diff * x_diff;
    }

    let slope = if denominator != 0.0 {
        numerator / denominator
    } else {
        0.0
    };
    let intercept = y_mean - slope * x_mean;

    // Predict the next value
    let next_x = n;
    let predicted_price = (intercept + slope * next_x).max(1.0);

    // Sanity check: cap at reasonable maximum
    let max_observed = median_prices
        .iter()
        .fold(0.0, |max: f64, &price| max.max(price));
    let reasonable_max = max_observed * 1.5; // Allow up to 50% increase
    let predicted_price = predicted_price.min(reasonable_max);

    round_to_9_places(predicted_price)
}

use crate::distribution::BlockDistribution;
use crate::types::ModelKind;
use adaptive_threshold::get_prediction_adaptive_threshold;
use anyhow::Result;
use distribution_analysis::get_prediction_distribution;
use last_min::get_prediction_last_min;
use moving_average::get_prediction_swma;
use percentile::get_prediction_percentile;
use time_series::get_prediction_time_series;

mod adaptive_threshold;
mod distribution_analysis;
mod last_min;
mod moving_average;
mod percentile;
mod time_series;

const MIN_PRICE: f64 = 0.00000001;

/// Will apply a model to a block distribution and return a price
pub async fn apply_model(
    model: &ModelKind,
    block_distributions: &[BlockDistribution],
) -> Result<f64> {
    // Handle empty input
    if block_distributions.is_empty() {
        return Ok(MIN_PRICE);
    }

    match model {
        ModelKind::AdaptiveThreshold => Ok(get_prediction_adaptive_threshold(block_distributions)),
        ModelKind::DistributionAnalysis => Ok(get_prediction_distribution(block_distributions)),
        ModelKind::MovingAverage => Ok(get_prediction_swma(block_distributions)),
        ModelKind::Percentile => Ok(get_prediction_percentile(block_distributions)),
        ModelKind::TimeSeries => Ok(get_prediction_time_series(block_distributions)),
        ModelKind::LastMin => Ok(get_prediction_last_min(block_distributions)),
    }
}
